import { useEffect, useState } from 'react';
import { Pencil, Plus, Trash2, Users } from 'lucide-react';
import { Card } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';
import { Input } from '@/components/ui/Input';
import { Modal } from '@/components/ui/Modal';
import { Textarea } from '@/components/ui/Textarea';
import { ConfirmDialog } from '@/components/ui/ConfirmDialog';
import { EmptyState } from '@/components/ui/EmptyState';
import { StudentTable } from '@/components/student/StudentTable';
import { StudentEditDrawer } from '@/components/student/StudentEditDrawer';
import { StudentImportDialog } from '@/components/student/StudentImportDialog';
import { useDirectoryStore } from '@/store/useDirectoryStore';
import { useStudentStore } from '@/store/useStudentStore';
import type { Class, Student } from '@/types/models';

interface GradeDraft {
  id?: string;
  gradeName: string;
  gradeNo: string;
  sortOrder: string;
  remark: string;
}

interface ClassDraft {
  id?: string;
  className: string;
  classNo: string;
  headTeacher: string;
  sortOrder: string;
  remark: string;
}

const EMPTY_GRADE: GradeDraft = { gradeName: '', gradeNo: '', sortOrder: '0', remark: '' };
const EMPTY_CLASS: ClassDraft = { className: '', classNo: '', headTeacher: '', sortOrder: '0', remark: '' };

/**
 * 年级 / 班级目录管理（教务处端）。
 *
 * 教务端在此统一维护「年级 → 班级」结构，并为每个班级维护学生名单；
 * 班级端通过绑定的 classId 消费对应班级的名册。目录变更经离线队列同步到班级端。
 */
export function GradeClassManage(): JSX.Element {
  const {
    grades,
    classes,
    gradesLoading,
    classesLoading,
    selectedGradeId,
    selectedClassId,
    loadGrades,
    loadClasses,
    selectGrade,
    selectClass,
    upsertGrade,
    removeGrade,
    upsertClass,
    removeClass,
  } = useDirectoryStore();

  const loadStudents = useStudentStore((s) => s.load);

  const [gradeDraft, setGradeDraft] = useState<GradeDraft | null>(null);
  const [classDraft, setClassDraft] = useState<ClassDraft | null>(null);
  const [pendingDelete, setPendingDelete] = useState<
    { kind: 'grade' | 'class'; id: string; name: string } | null
  >(null);

  const [editTarget, setEditTarget] = useState<Student | null>(null);
  const [importOpen, setImportOpen] = useState(false);

  useEffect(() => {
    void loadGrades();
  }, [loadGrades]);

  useEffect(() => {
    if (selectedGradeId) void loadClasses(selectedGradeId);
  }, [loadClasses, selectedGradeId]);

  const selectedGrade = grades.find((g) => g.id === selectedGradeId) ?? null;
  const selectedClass = classes.find((c) => c.id === selectedClassId) ?? null;

  // 选中班级后联动加载该班名册
  useEffect(() => {
    if (selectedClass) {
      void loadStudents({
        classId: selectedClass.id,
        className: selectedClass.className,
        grade: selectedClass.gradeName,
      });
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [selectedClassId]);

  const openGradeEdit = (grade?: GradeDraft & { id?: string }): void => {
    setGradeDraft(grade ?? EMPTY_GRADE);
  };

  const saveGrade = async (): Promise<void> => {
    if (!gradeDraft) return;
    if (!gradeDraft.gradeName.trim()) return;
    await upsertGrade({
      id: gradeDraft.id,
      gradeName: gradeDraft.gradeName.trim(),
      gradeNo: gradeDraft.gradeNo.trim(),
      sortOrder: Number.parseInt(gradeDraft.sortOrder, 10) || 0,
      remark: gradeDraft.remark.trim() || null,
    });
    setGradeDraft(null);
  };

  const openClassEdit = (klass?: Class): void => {
    if (!selectedGradeId) return;
    setClassDraft({
      id: klass?.id,
      className: klass?.className ?? '',
      classNo: klass?.classNo ?? '',
      headTeacher: klass?.headTeacher ?? '',
      sortOrder: klass ? String(klass.sortOrder) : '0',
      remark: klass?.remark ?? '',
    });
  };

  const saveClass = async (): Promise<void> => {
    if (!classDraft || !selectedGradeId) return;
    if (!classDraft.className.trim()) return;
    await upsertClass({
      id: classDraft.id,
      gradeId: selectedGradeId,
      gradeNo: selectedGrade?.gradeNo || null,
      gradeName: selectedGrade?.gradeName || null,
      className: classDraft.className.trim(),
      classNo: classDraft.classNo.trim() || null,
      headTeacher: classDraft.headTeacher.trim() || null,
      sortOrder: Number.parseInt(classDraft.sortOrder, 10) || 0,
      remark: classDraft.remark.trim() || null,
    });
    setClassDraft(null);
  };

  const confirmDelete = async (): Promise<void> => {
    if (!pendingDelete) return;
    if (pendingDelete.kind === 'grade') await removeGrade(pendingDelete.id);
    else await removeClass(pendingDelete.id);
    setPendingDelete(null);
  };

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <h1 className="text-3xl font-bold text-ink">年级与班级管理</h1>
        <Button
          variant="secondary"
          icon={<Plus className="h-5 w-5" />}
          onClick={() => openGradeEdit()}
          disabled={!!gradeDraft}
        >
          新增年级
        </Button>
      </div>

      <div className="grid grid-cols-1 gap-4 lg:grid-cols-[20rem_1fr]">
        {/* 年级列表 */}
        <Card className="p-3">
          <p className="mb-2 px-2 text-base font-semibold text-ink">年级</p>
          {gradesLoading && <p className="px-2 py-4 text-sm text-ink-muted">加载中…</p>}
          {!gradesLoading && grades.length === 0 && (
            <p className="px-2 py-4 text-sm text-ink-muted">还没有年级，点击右上角新增。</p>
          )}
          <ul className="space-y-1">
            {grades.map((g) => (
              <li key={g.id}>
                <div
                  className={[
                    'group flex items-center gap-2 rounded-lg px-3 py-2 transition-colors',
                    g.id === selectedGradeId
                      ? 'bg-brand-50 ring-2 ring-brand-400'
                      : 'hover:bg-surface-muted',
                  ].join(' ')}
                >
                  <button
                    type="button"
                    className="flex-1 text-left"
                    onClick={() => selectGrade(g.id === selectedGradeId ? null : g.id)}
                  >
                    <span className="font-semibold text-ink">{g.gradeName}</span>
                    {g.gradeNo && <span className="ml-2 text-sm text-ink-muted">{g.gradeNo}</span>}
                  </button>
                  <button
                    type="button"
                    className="rounded p-1 text-ink-muted hover:bg-surface-raised hover:text-ink"
                    onClick={() => openGradeEdit({ id: g.id, gradeName: g.gradeName, gradeNo: g.gradeNo, sortOrder: String(g.sortOrder), remark: g.remark ?? '' })}
                    aria-label="编辑年级"
                  >
                    <Pencil className="h-4 w-4" />
                  </button>
                  <button
                    type="button"
                    className="rounded p-1 text-ink-muted hover:bg-red-50 hover:text-red-600"
                    onClick={() => setPendingDelete({ kind: 'grade', id: g.id, name: g.gradeName })}
                    aria-label="删除年级"
                  >
                    <Trash2 className="h-4 w-4" />
                  </button>
                </div>
              </li>
            ))}
          </ul>
        </Card>

        {/* 班级 + 名册 */}
        <div className="space-y-4">
          {!selectedGrade && (
            <Card className="p-6">
              <EmptyState
                title="请选择左侧年级"
                description="选定年级后即可管理其下的班级，并为每个班级维护学生名单。"
              />
            </Card>
          )}

          {selectedGrade && (
            <>
              <Card className="p-4">
                <div className="mb-3 flex items-center justify-between">
                  <p className="text-xl font-bold text-ink">
                    {selectedGrade.gradeName} · 班级
                  </p>
                  <Button
                    variant="secondary"
                    size="md"
                    icon={<Plus className="h-4 w-4" />}
                    onClick={() => openClassEdit()}
                  >
                    新增班级
                  </Button>
                </div>
                {classesLoading && <p className="text-sm text-ink-muted">加载中…</p>}
                {!classesLoading && classes.length === 0 && (
                  <p className="text-sm text-ink-muted">该年级下还没有班级。</p>
                )}
                <div className="grid grid-cols-1 gap-2 sm:grid-cols-2">
                  {classes.map((c) => (
                    <div
                      key={c.id}
                      className={[
                        'group flex items-center gap-2 rounded-lg border px-3 py-2 transition-colors',
                        c.id === selectedClassId
                          ? 'border-brand-500 bg-brand-50'
                          : 'border-surface-border hover:bg-surface-muted',
                      ].join(' ')}
                    >
                      <button
                        type="button"
                        className="flex flex-1 items-center gap-2 text-left"
                        onClick={() => selectClass(c.id === selectedClassId ? null : c.id)}
                      >
                        <Users className="h-4 w-4 text-brand-600" aria-hidden />
                        <span className="font-semibold text-ink">{c.className}</span>
                        {c.headTeacher && (
                          <span className="text-sm text-ink-muted">班主任：{c.headTeacher}</span>
                        )}
                      </button>
                      <button
                        type="button"
                        className="rounded p-1 text-ink-muted hover:bg-surface-raised hover:text-ink"
                        onClick={() => openClassEdit(c)}
                        aria-label="编辑班级"
                      >
                        <Pencil className="h-4 w-4" />
                      </button>
                      <button
                        type="button"
                        className="rounded p-1 text-ink-muted hover:bg-red-50 hover:text-red-600"
                        onClick={() => setPendingDelete({ kind: 'class', id: c.id, name: c.className })}
                        aria-label="删除班级"
                      >
                        <Trash2 className="h-4 w-4" />
                      </button>
                    </div>
                  ))}
                </div>
              </Card>

              {selectedClass && (
                <Card className="p-4">
                  <div className="mb-3 flex items-center justify-between">
                    <p className="text-xl font-bold text-ink">{selectedClass.className} · 学生名单</p>
                    <div className="flex gap-2">
                      <Button
                        size="md"
                        variant="secondary"
                        onClick={() => setImportOpen(true)}
                      >
                        导入名册
                      </Button>
                      <Button
                        size="md"
                        onClick={() => setEditTarget(null)}
                      >
                        新增学生
                      </Button>
                    </div>
                  </div>
                  <StudentTable onEdit={setEditTarget} onImport={() => setImportOpen(true)} />
                </Card>
              )}
            </>
          )}
        </div>
      </div>

      {/* 年级编辑弹窗 */}
      <Modal
        open={gradeDraft !== null}
        onClose={() => setGradeDraft(null)}
        title={gradeDraft?.id ? '编辑年级' : '新增年级'}
        widthClass="max-w-lg"
        footer={
          <>
            <Button variant="secondary" onClick={() => setGradeDraft(null)}>
              取消
            </Button>
            <Button onClick={saveGrade} disabled={!gradeDraft?.gradeName.trim()}>
              保存
            </Button>
          </>
        }
      >
        {gradeDraft && (
          <div className="space-y-4">
            <Input
              label="年级名称"
              value={gradeDraft.gradeName}
              onChange={(e) => setGradeDraft({ ...gradeDraft, gradeName: e.target.value })}
              placeholder="如：三年级"
            />
            <div className="grid min-w-0 grid-cols-1 gap-4 sm:grid-cols-2">
              <Input
                label="年级编号"
                value={gradeDraft.gradeNo}
                onChange={(e) => setGradeDraft({ ...gradeDraft, gradeNo: e.target.value })}
                placeholder="如：3 / 2023"
              />
              <Input
                label="排序"
                value={gradeDraft.sortOrder}
                onChange={(e) => setGradeDraft({ ...gradeDraft, sortOrder: e.target.value })}
                inputMode="numeric"
              />
            </div>
            <Textarea
              label="备注"
              value={gradeDraft.remark}
              onChange={(e) => setGradeDraft({ ...gradeDraft, remark: e.target.value })}
              rows={3}
            />
          </div>
        )}
      </Modal>

      {/* 班级编辑弹窗 */}
      <Modal
        open={classDraft !== null}
        onClose={() => setClassDraft(null)}
        title={classDraft?.id ? '编辑班级' : '新增班级'}
        widthClass="max-w-lg"
        footer={
          <>
            <Button variant="secondary" onClick={() => setClassDraft(null)}>
              取消
            </Button>
            <Button onClick={saveClass} disabled={!classDraft?.className.trim()}>
              保存
            </Button>
          </>
        }
      >
        {classDraft && (
          <div className="space-y-4">
            <Input
              label="班级名称"
              value={classDraft.className}
              onChange={(e) => setClassDraft({ ...classDraft, className: e.target.value })}
              placeholder="如：三年级二班"
            />
            <div className="grid min-w-0 grid-cols-1 gap-4 sm:grid-cols-2">
              <Input
                label="班号"
                value={classDraft.classNo}
                onChange={(e) => setClassDraft({ ...classDraft, classNo: e.target.value })}
                placeholder="如：2"
              />
              <Input
                label="排序"
                value={classDraft.sortOrder}
                onChange={(e) => setClassDraft({ ...classDraft, sortOrder: e.target.value })}
                inputMode="numeric"
              />
            </div>
            <Input
              label="班主任"
              value={classDraft.headTeacher}
              onChange={(e) => setClassDraft({ ...classDraft, headTeacher: e.target.value })}
              placeholder="如：张老师"
            />
            <Textarea
              label="备注"
              value={classDraft.remark}
              onChange={(e) => setClassDraft({ ...classDraft, remark: e.target.value })}
              rows={3}
            />
          </div>
        )}
      </Modal>

      {/* 删除确认 */}
      <ConfirmDialog
        open={pendingDelete !== null}
        danger
        title={pendingDelete?.kind === 'grade' ? '删除年级' : '删除班级'}
        message={`确定删除「${pendingDelete?.name}」吗？`}
        detail={
          pendingDelete?.kind === 'grade'
            ? '其下班级的年级关联将置空，但班级与学生记录保留。'
            : '该班级的学生名单保留，仅解除与班级的关联。'
        }
        confirmText="删除"
        onCancel={() => setPendingDelete(null)}
        onConfirm={confirmDelete}
      />

      {/* 班级名册：编辑 / 导入（作用域为该班级） */}
      {selectedClass && (
        <>
          <StudentImportDialog
            open={importOpen}
            classContext={{
              classId: selectedClass.id,
              className: selectedClass.className,
              grade: selectedClass.gradeName,
            }}
            onClose={() => setImportOpen(false)}
          />
          <StudentEditDrawer
            open={editTarget !== null}
            student={editTarget}
            classContext={{
              classId: selectedClass.id,
              className: selectedClass.className,
              grade: selectedClass.gradeName,
            }}
            onClose={() => setEditTarget(null)}
          />
        </>
      )}
    </div>
  );
}
