import { useEffect, useState } from 'react';
import { CalendarPlus, Pencil, Plus, Trash2, Users, Monitor } from 'lucide-react';
import { Card } from '@shared/components/ui/Card';
import { Button } from '@shared/components/ui/Button';
import { Input } from '@shared/components/ui/Input';
import { Modal } from '@shared/components/ui/Modal';
import { SearchableSelect } from '@shared/components/ui/SearchableSelect';
import { Textarea } from '@shared/components/ui/Textarea';
import { ConfirmDialog } from '@shared/components/ui/ConfirmDialog';
import { EmptyState } from '@shared/components/ui/EmptyState';
import { StudentTable } from '@shared/components/student/StudentTable';
import { StudentEditDrawer } from '@shared/components/student/StudentEditDrawer';
import { StudentImportDialog } from '@shared/components/student/StudentImportDialog';
import { useDirectoryStore } from '@shared/store/useDirectoryStore';
import { useStudentStore } from '@shared/store/useStudentStore';
import { useDeviceStore } from '@shared/store/useDeviceStore';
import { useAppStore } from '@shared/store/useAppStore';
import { useTauriEventHandler } from '@shared/hooks/useTauriEvent';
import { TAURI_EVENTS } from '@shared/types/events';
import { classroomAssign, classroomAssignments, classroomDelete, classroomList, classroomUpsert } from '@shared/lib/db';
import type { Class, Classroom, ClassroomAssignment, SchoolYear, Student } from '@shared/types/models';

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
  /** 归入的学年（年隔离维度） */
  schoolYearId?: string | null;
}

interface SchoolYearDraft {
  id?: string;
  schoolYearName: string;
  schoolYearNo: string;
  startDate: string;
  endDate: string;
  sortOrder: string;
  remark: string;
}

interface ClassroomDraft {
  id?: string;
  roomName: string;
  remark: string;
}

const EMPTY_GRADE: GradeDraft = { gradeName: '', gradeNo: '', sortOrder: '0', remark: '' };
const EMPTY_CLASS: ClassDraft = {
  className: '',
  classNo: '',
  headTeacher: '',
  sortOrder: '0',
  remark: '',
  schoolYearId: null,
};
const EMPTY_YEAR: SchoolYearDraft = {
  schoolYearName: '',
  schoolYearNo: '',
  startDate: '',
  endDate: '',
  sortOrder: '0',
  remark: '',
};

/**
 * 年级 / 班级目录管理（教务处端）。
 *
 * 教务端在此统一维护「学年 → 年级 → 班级」结构，并为每个班级维护学生名单；
 * 班级端通过绑定的 schoolYearId + classId 消费对应学年班级的名册。目录变更经
 * 离线队列同步到班级端。学年是时间维度：同一教室每学年的班级人员、班主任不同，
 * 但旧数据保留（班级真正身份 = (school_year_id, grade_id, class_no)）。
 */
export function GradeClassManage(): JSX.Element {
  const {
    grades,
    classes,
    schoolYears,
    gradesLoading,
    classesLoading,
    selectedGradeId,
    selectedClassId,
    selectedSchoolYearId,
    loadGrades,
    loadClasses,
    loadSchoolYears,
    selectGrade,
    selectClass,
    selectSchoolYear,
    upsertGrade,
    removeGrade,
    upsertClass,
    removeClass,
    upsertSchoolYear,
    removeSchoolYear,
  } = useDirectoryStore();

  const loadStudents = useStudentStore((s) => s.load);
  const pushToast = useAppStore((s) => s.pushToast);
  const devices = useDeviceStore((s) => s.devices);
  const loadDevices = useDeviceStore((s) => s.load);
  const [classrooms, setClassrooms] = useState<Classroom[]>([]);
  const [assignments, setAssignments] = useState<ClassroomAssignment[]>([]);
  const [roomName, setRoomName] = useState('');

  const [gradeDraft, setGradeDraft] = useState<GradeDraft | null>(null);
  const [classDraft, setClassDraft] = useState<ClassDraft | null>(null);
  const [yearDraft, setYearDraft] = useState<SchoolYearDraft | null>(null);
  const [classroomDraft, setClassroomDraft] = useState<ClassroomDraft | null>(null);
  const [pendingUnclaim, setPendingUnclaim] = useState<Classroom | null>(null);
  const [pendingDelete, setPendingDelete] = useState<
    { kind: 'grade' | 'class' | 'year' | 'classroom'; id: string; name: string } | null
  >(null);

  const [editTarget, setEditTarget] = useState<Student | null>(null);
  const [importOpen, setImportOpen] = useState(false);

  useEffect(() => {
    void loadGrades();
    void loadSchoolYears();
    void loadDevices();
  }, [loadGrades, loadSchoolYears]);

  useEffect(() => {
    void classroomList().then(setClassrooms).catch(() => undefined);
    void classroomAssignments(selectedSchoolYearId).then(setAssignments).catch(() => undefined);
  }, [selectedSchoolYearId]);

  const refreshClassrooms = async (): Promise<void> => {
    const [rooms, current] = await Promise.all([
      classroomList(),
      classroomAssignments(selectedSchoolYearId),
    ]);
    setClassrooms(rooms);
    setAssignments(current);
  };

  useTauriEventHandler(TAURI_EVENTS.CLASSROOM_CHANGED, () => {
    void refreshClassrooms().catch(() => undefined);
  });

  const addClassroom = async (): Promise<void> => {
    if (!roomName.trim()) return;
    await classroomUpsert({ roomName: roomName.trim(), deviceId: null, remark: null });
    setRoomName('');
    await refreshClassrooms();
  };

  const saveClassroom = async (): Promise<void> => {
    if (!classroomDraft?.roomName.trim()) return;
    const current = classrooms.find((room) => room.id === classroomDraft.id);
    await classroomUpsert({
      id: classroomDraft.id,
      roomName: classroomDraft.roomName.trim(),
      deviceId: current?.deviceId ?? null,
      remark: classroomDraft.remark.trim() || null,
    });
    setClassroomDraft(null);
    await refreshClassrooms();
  };

  const unclaimClassroom = async (): Promise<void> => {
    if (!pendingUnclaim) return;
    await classroomUpsert({
      id: pendingUnclaim.id,
      roomName: pendingUnclaim.roomName,
      deviceId: null,
      remark: pendingUnclaim.remark ?? null,
    });
    setPendingUnclaim(null);
    await refreshClassrooms();
  };

  const bindSelectedClass = async (room: Classroom): Promise<void> => {
    if (!selectedClass) return;
    // 新建/切换学年时状态可能尚未完成联动，优先使用页面选中的学年，
    // 否则回退到班级自身的学年，避免“绑定当前班级”被无故禁用。
    const schoolYearId = selectedSchoolYearId ?? selectedClass.schoolYearId;
    if (!schoolYearId) return;
    await classroomAssign(room.id, schoolYearId, selectedClass.id);
    await refreshClassrooms();
  };

  useEffect(() => {
    if (selectedGradeId) void loadClasses(selectedGradeId);
  }, [loadClasses, selectedGradeId]);

  const selectedGrade = grades.find((g) => g.id === selectedGradeId) ?? null;
  const selectedClass = classes.find((c) => c.id === selectedClassId) ?? null;
  const selectedYear = schoolYears.find((y) => y.id === selectedSchoolYearId) ?? null;

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
  }, [selectedClassId, selectedClass?.className]);

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
      schoolYearId: klass?.schoolYearId ?? selectedSchoolYearId ?? null,
    });
  };

  const saveClass = async (): Promise<void> => {
    if (!classDraft || !selectedGradeId) return;
    if (!classDraft.className.trim()) return;
    if (!classDraft.id && !classDraft.schoolYearId) {
      pushToast({ kind: 'warning', title: '请选择所属学年', description: '新建班级必须归入一个学年' });
      return;
    }
    await upsertClass({
      id: classDraft.id,
      gradeId: selectedGradeId,
      gradeNo: selectedGrade?.gradeNo || null,
      gradeName: selectedGrade?.gradeName || null,
      schoolYearId: classDraft.schoolYearId ?? null,
      className: classDraft.className.trim(),
      classNo: classDraft.classNo.trim() || null,
      headTeacher: classDraft.headTeacher.trim() || null,
      sortOrder: Number.parseInt(classDraft.sortOrder, 10) || 0,
      remark: classDraft.remark.trim() || null,
    });
    setClassDraft(null);
  };

  const openYearEdit = (year?: SchoolYear): void => {
    setYearDraft({
      id: year?.id,
      schoolYearName: year?.schoolYearName ?? '',
      schoolYearNo: year?.schoolYearNo ?? '',
      startDate: year?.startDate ?? '',
      endDate: year?.endDate ?? '',
      sortOrder: year ? String(year.sortOrder) : '0',
      remark: year?.remark ?? '',
    });
  };

  const saveYear = async (): Promise<void> => {
    if (!yearDraft) return;
    if (!yearDraft.schoolYearName.trim()) return;
    await upsertSchoolYear({
      id: yearDraft.id,
      schoolYearName: yearDraft.schoolYearName.trim(),
      schoolYearNo: yearDraft.schoolYearNo.trim(),
      startDate: yearDraft.startDate.trim() || null,
      endDate: yearDraft.endDate.trim() || null,
      sortOrder: Number.parseInt(yearDraft.sortOrder, 10) || 0,
      remark: yearDraft.remark.trim() || null,
    });
    setYearDraft(null);
  };

  const confirmDelete = async (): Promise<void> => {
    if (!pendingDelete) return;
    if (pendingDelete.kind === 'grade') await removeGrade(pendingDelete.id);
    else if (pendingDelete.kind === 'class') await removeClass(pendingDelete.id);
    else if (pendingDelete.kind === 'year') await removeSchoolYear(pendingDelete.id);
    else {
      await classroomDelete(pendingDelete.id);
      await refreshClassrooms();
    }
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

      {/* 学年维度选择器 */}
      <div className="flex flex-wrap items-center gap-3 rounded-xl border border-surface-border bg-surface-muted p-3">
        <span className="text-sm font-semibold text-ink">学年</span>
        <div className="min-w-[12rem] flex-1">
          <SearchableSelect
            label=""
            options={schoolYears.map((y) => ({ value: y.id, label: y.schoolYearName }))}
            value={selectedSchoolYearId ?? ''}
            onChange={(value) => selectSchoolYear(value || null)}
            placeholder="— 全部学年 —"
          />
        </div>
        <Button
          variant="secondary"
          size="md"
          icon={<CalendarPlus className="h-4 w-4" />}
          onClick={() => openYearEdit()}
          disabled={!!yearDraft}
        >
          新增学年
        </Button>
        {selectedYear && (
          <Button
            variant="ghost"
            size="md"
            icon={<Pencil className="h-4 w-4" />}
            onClick={() => openYearEdit(selectedYear)}
          >
            编辑
          </Button>
        )}
        {selectedYear && (
          <Button
            variant="ghost"
            size="md"
            icon={<Trash2 className="h-4 w-4" />}
            onClick={() =>
              setPendingDelete({ kind: 'year', id: selectedYear.id, name: selectedYear.schoolYearName })
            }
          >
            删除
          </Button>
        )}
      </div>

      <Card className="p-4">
        <div className="mb-3 flex flex-wrap items-center justify-between gap-3">
          <div>
            <p className="text-xl font-bold text-ink">教室（物理位置）与设备</p>
            <p className="text-sm text-ink-muted">教室长期保留，设备可以更换；按当前学年绑定服务班级，换学年无需重装设备。</p>
          </div>
          <div className="flex gap-2">
            <Input label="" aria-label="教室名称" placeholder="新增教室，如 301" value={roomName} onChange={(e) => setRoomName(e.target.value)} />
            <Button size="md" onClick={() => void addClassroom()} disabled={!roomName.trim()}>新增教室</Button>
          </div>
        </div>
        {classrooms.length === 0 ? <p className="text-sm text-ink-muted">还没有教室记录。</p> : (
          <div className="grid gap-2 md:grid-cols-2 lg:grid-cols-3">
            {classrooms.map((room) => {
              const assignment = assignments.find((a) => a.classroomId === room.id);
              const device = devices.find((d) => d.deviceId === room.deviceId);
              const assignedClass = classes.find((c) => c.id === assignment?.classId);
              return <div key={room.id} className="rounded-lg border border-surface-border p-3">
                <div className="flex items-center justify-between gap-2"><div className="flex items-center gap-2"><Monitor className="h-4 w-4 text-brand-600" /><span className="font-semibold text-ink">{room.roomName}</span></div><div className="flex gap-1"><Button variant="ghost" icon={<Pencil className="h-4 w-4" />} onClick={() => setClassroomDraft({ id: room.id, roomName: room.roomName, remark: room.remark ?? '' })}>编辑</Button><Button variant="ghost" icon={<Trash2 className="h-4 w-4" />} onClick={() => setPendingDelete({ kind: 'classroom', id: room.id, name: room.roomName })}>删除</Button></div></div>
                <div className="mt-2 rounded-lg bg-surface-muted px-3 py-2">
                  <p className="text-sm font-medium text-ink">设备认领状态</p>
                  <p className="mt-1 text-sm text-ink-muted">
                    {room.deviceId
                      ? `${device?.deviceName ?? `设备-${room.deviceId.slice(0, 8)}`} · ${device?.status === 'online' ? '在线' : '离线'}`
                      : '尚未被班级端认领'}
                  </p>
                </div>
                <p className="text-sm text-ink-muted">当前班级：{assignedClass?.className ?? '未分配'}</p>
                <div className="mt-2 flex flex-wrap gap-2">
                  <Button size="md" variant="secondary" disabled={!selectedClass || !(selectedSchoolYearId ?? selectedClass.schoolYearId)} onClick={() => void bindSelectedClass(room)}>绑定当前班级</Button>
                  {room.deviceId && <Button size="md" variant="ghost" onClick={() => setPendingUnclaim(room)}>解除设备认领</Button>}
                </div>
              </div>;
            })}
          </div>
        )}
      </Card>

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
                    {selectedYear && (
                      <span className="ml-2 text-base font-normal text-ink-muted">
                        （{selectedYear.schoolYearName}）
                      </span>
                    )}
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
            <Button onClick={saveClass} disabled={!classDraft?.className.trim() || (!classDraft?.id && !classDraft?.schoolYearId)}>
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
            <SearchableSelect
              label="所属学年（必填）"
              options={schoolYears.map((y) => ({ value: y.id, label: y.schoolYearName }))}
              value={classDraft.schoolYearId ?? ''}
              onChange={(value) => setClassDraft({ ...classDraft, schoolYearId: value || null })}
              placeholder="— 请选择学年 —"
            />
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

      {/* 学年编辑弹窗 */}
      <Modal
        open={yearDraft !== null}
        onClose={() => setYearDraft(null)}
        title={yearDraft?.id ? '编辑学年' : '新增学年'}
        widthClass="max-w-lg"
        footer={
          <>
            <Button variant="secondary" onClick={() => setYearDraft(null)}>
              取消
            </Button>
            <Button onClick={saveYear} disabled={!yearDraft?.schoolYearName.trim()}>
              保存
            </Button>
          </>
        }
      >
        {yearDraft && (
          <div className="space-y-4">
            <Input
              label="学年名称"
              value={yearDraft.schoolYearName}
              onChange={(e) => setYearDraft({ ...yearDraft, schoolYearName: e.target.value })}
              placeholder="如：2027届"
            />
            <div className="grid min-w-0 grid-cols-1 gap-4 sm:grid-cols-3">
              <Input
                label="届号"
                value={yearDraft.schoolYearNo}
                onChange={(e) => setYearDraft({ ...yearDraft, schoolYearNo: e.target.value })}
                placeholder="如：2027"
              />
              <Input
                label="开学日期"
                value={yearDraft.startDate}
                onChange={(e) => setYearDraft({ ...yearDraft, startDate: e.target.value })}
                placeholder="YYYY-MM-DD"
              />
              <Input
                label="结束日期"
                value={yearDraft.endDate}
                onChange={(e) => setYearDraft({ ...yearDraft, endDate: e.target.value })}
                placeholder="YYYY-MM-DD"
              />
            </div>
            <Input
              label="排序"
              value={yearDraft.sortOrder}
              onChange={(e) => setYearDraft({ ...yearDraft, sortOrder: e.target.value })}
              inputMode="numeric"
            />
            <Textarea
              label="备注"
              value={yearDraft.remark}
              onChange={(e) => setYearDraft({ ...yearDraft, remark: e.target.value })}
              rows={3}
            />
          </div>
        )}
      </Modal>

      <Modal
        open={classroomDraft !== null}
        onClose={() => setClassroomDraft(null)}
        title="编辑教室"
        widthClass="max-w-lg"
        footer={<><Button variant="secondary" onClick={() => setClassroomDraft(null)}>取消</Button><Button onClick={() => void saveClassroom()} disabled={!classroomDraft?.roomName.trim()}>保存</Button></>}
      >
        {classroomDraft && <div className="space-y-4"><Input label="教室名称" value={classroomDraft.roomName} onChange={(e) => setClassroomDraft({ ...classroomDraft, roomName: e.target.value })} /><Textarea label="备注" value={classroomDraft.remark} onChange={(e) => setClassroomDraft({ ...classroomDraft, remark: e.target.value })} rows={3} /></div>}
      </Modal>

      {/* 删除确认 */}
      <ConfirmDialog
        open={pendingDelete !== null}
        danger
        title={
          pendingDelete?.kind === 'grade'
            ? '删除年级'
            : pendingDelete?.kind === 'class'
              ? '删除班级'
              : pendingDelete?.kind === 'year'
                ? '删除学年'
                : '删除教室'
        }
        message={`确定删除「${pendingDelete?.name}」吗？`}
        detail={
          pendingDelete?.kind === 'grade'
            ? '其下班级的年级关联将置空，但班级与学生记录保留。'
            : pendingDelete?.kind === 'class'
              ? '该班级的学生名单保留，仅解除与班级的关联。'
              : pendingDelete?.kind === 'year'
                ? '该学年的班级 school_year_id 保留，历史数据不丢失；仅解除「当前学年」语义。'
                : '教室及其学年班级绑定会被移出目录，设备不会被删除。'
        }
        confirmText="删除"
        onCancel={() => setPendingDelete(null)}
        onConfirm={confirmDelete}
      />

      <ConfirmDialog
        open={pendingUnclaim !== null}
        title="解除设备认领"
        message={`确定解除「${pendingUnclaim?.roomName ?? ''}」当前设备认领吗？`}
        detail="解除后，新的班级端可以重新认领此教室；原设备不会被删除。"
        confirmText="解除认领"
        onCancel={() => setPendingUnclaim(null)}
        onConfirm={() => void unclaimClassroom()}
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
