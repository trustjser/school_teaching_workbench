import { useEffect, useMemo, useState } from 'react';
import { ArrowRightLeft, Copy, ListPlus, Plus, School, Trash2 } from 'lucide-react';
import { Modal } from '@shared/components/ui/Modal';
import { Button } from '@shared/components/ui/Button';
import { Input } from '@shared/components/ui/Input';
import { SearchableSelect } from '@shared/components/ui/SearchableSelect';
import {
  classList,
  directoryBatchCreate,
  schoolYearRollover,
  type DirectoryBatchCreateReport,
  type DirectoryBatchClassInput,
  type DirectoryBatchGradeInput,
  type RolloverReport,
  type RolloverRequest,
} from '@shared/lib/db';
import type { Class, Grade, SchoolYear } from '@shared/types/models';

/** 命名模板展开：{年级} / {序号} / {序号2}（两位补零） */
export function expandTemplate(template: string, gradeName: string, seq: number): string {
  const padded = String(seq).padStart(2, '0');
  return template
    .replace(/\{年级\}/g, gradeName)
    .replace(/\{序号2\}/g, padded)
    .replace(/\{序号\}/g, String(seq));
}

const DEFAULT_TEMPLATE = '{年级}{序号}班';
const DEFAULT_GRADE_NAMES = ['一年级', '二年级', '三年级', '四年级', '五年级', '六年级'];

interface GradeRow {
  key: string;
  gradeName: string;
  count: string;
}

/* -------------------------------------------------------------------------- */
/* 快速建校：新建学年 + 年级 × 班数 + 命名模板                                    */
/* -------------------------------------------------------------------------- */

export interface QuickSchoolSetupModalProps {
  open: boolean;
  onClose: () => void;
  onDone: (report: DirectoryBatchCreateReport) => Promise<void> | void;
}

export function QuickSchoolSetupModal({ open, onClose, onDone }: QuickSchoolSetupModalProps): JSX.Element {
  const [yearName, setYearName] = useState(`${new Date().getFullYear() + 1}届`);
  const [yearNo, setYearNo] = useState('');
  const [rows, setRows] = useState<GradeRow[]>(
    DEFAULT_GRADE_NAMES.map((name, i) => ({ key: `g${i}`, gradeName: name, count: '4' })),
  );
  const [template, setTemplate] = useState(DEFAULT_TEMPLATE);
  const [submitting, setSubmitting] = useState(false);

  const namedRows = useMemo(() => rows.filter((r) => r.gradeName.trim() !== ''), [rows]);
  const totalCount = useMemo(
    () => namedRows.reduce((sum, r) => sum + (Number.parseInt(r.count, 10) || 0), 0),
    [namedRows],
  );

  const canSubmit = yearName.trim() !== '' && namedRows.length > 0 && !submitting;

  const submit = async (): Promise<void> => {
    if (!canSubmit) return;
    setSubmitting(true);
    try {
      const grades: DirectoryBatchGradeInput[] = namedRows.map((r, i) => ({
        key: r.key,
        gradeName: r.gradeName.trim(),
        gradeNo: String(i + 1),
        sortOrder: i + 1,
      }));
      const classes: DirectoryBatchClassInput[] = [];
      namedRows.forEach((row) => {
        const count = Number.parseInt(row.count, 10) || 0;
        for (let seq = 1; seq <= count; seq += 1) {
          classes.push({
            gradeKey: row.key,
            className: expandTemplate(template, row.gradeName.trim(), seq),
            classNo: String(seq),
            sortOrder: seq,
          });
        }
      });
      const report = await directoryBatchCreate({
        schoolYear: {
          schoolYearName: yearName.trim(),
          schoolYearNo: yearNo.trim() || null,
          startDate: null,
          endDate: null,
          sortOrder: 0,
        },
        grades,
        classes,
      });
      await onDone(report);
      onClose();
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <Modal
      open={open}
      onClose={onClose}
      title="快速建校"
      description="一次生成「学年 → 年级 → 班级」整棵目录树；同名学年 / 年级 / 班级已存在时自动跳过，可安全重复执行。"
      widthClass="max-w-2xl"
      footer={
        <>
          <Button variant="secondary" onClick={onClose}>取消</Button>
          <Button icon={<School className="h-4 w-4" />} onClick={() => void submit()} disabled={!canSubmit}>
            {submitting ? '创建中…' : `创建学年 + ${namedRows.length} 个年级 + ${totalCount} 个班级`}
          </Button>
        </>
      }
    >
      <div className="space-y-4">
        <div className="grid min-w-0 grid-cols-1 gap-4 sm:grid-cols-2">
          <Input
            label="学年名称（必填）"
            value={yearName}
            onChange={(e) => setYearName(e.target.value)}
            placeholder="如：2027届"
          />
          <Input
            label="届号（可选）"
            value={yearNo}
            onChange={(e) => setYearNo(e.target.value)}
            placeholder="如：2027"
          />
        </div>

        <div>
          <div className="mb-2 flex items-center justify-between">
            <p className="text-sm font-semibold text-ink">年级与班数</p>
            <Button
              size="md"
              variant="ghost"
              icon={<Plus className="h-4 w-4" />}
              onClick={() => setRows((prev) => [...prev, { key: `g${Date.now()}`, gradeName: '', count: '1' }])}
            >
              添加年级
            </Button>
          </div>
          <div className="space-y-2">
            {rows.map((row, index) => (
              <div key={row.key} className="flex items-end gap-2">
                <div className="min-w-0 flex-1">
                  <Input
                    label={index === 0 ? '年级名称' : ''}
                    aria-label="年级名称"
                    value={row.gradeName}
                    onChange={(e) =>
                      setRows((prev) => prev.map((r) => (r.key === row.key ? { ...r, gradeName: e.target.value } : r)))
                    }
                    placeholder="如：三年级"
                  />
                </div>
                <div className="w-24">
                  <Input
                    label={index === 0 ? '班数' : ''}
                    aria-label="班数"
                    inputMode="numeric"
                    value={row.count}
                    onChange={(e) =>
                      setRows((prev) => prev.map((r) => (r.key === row.key ? { ...r, count: e.target.value } : r)))
                    }
                  />
                </div>
                <Button
                  variant="ghost"
                  icon={<Trash2 className="h-4 w-4" />}
                  aria-label="删除该年级"
                  onClick={() => setRows((prev) => prev.filter((r) => r.key !== row.key))}
                >
                  <span className="sr-only">删除</span>
                </Button>
              </div>
            ))}
          </div>
        </div>

        <Input
          label="班级命名模板"
          value={template}
          onChange={(e) => setTemplate(e.target.value)}
          placeholder="{年级}{序号}班，可用 {序号2} 补零"
        />

        <div className="rounded-lg border border-surface-border bg-surface-muted p-3">
          <p className="text-sm font-semibold text-ink">
            预览：{yearName.trim() || '（未命名学年）'} · {namedRows.length} 个年级 · {totalCount} 个班级
          </p>
          <ul className="mt-2 max-h-36 space-y-1 overflow-y-auto text-sm text-ink-muted">
            {namedRows.map((row) => {
              const count = Number.parseInt(row.count, 10) || 0;
              const sample = count > 0
                ? [1, 2, 3]
                    .filter((seq) => seq <= count)
                    .map((seq) => expandTemplate(template, row.gradeName.trim(), seq))
                    .join('、') + (count > 3 ? ` …共 ${count} 个班` : '')
                : '（0 个班）';
              return (
                <li key={row.key}>
                  <span className="font-medium text-ink">{row.gradeName.trim()}</span>：{sample}
                </li>
              );
            })}
          </ul>
        </div>
      </div>
    </Modal>
  );
}

/* -------------------------------------------------------------------------- */
/* 克隆学年结构：把某学年的年级 / 班级骨架复制到新学年（不拷学生）                  */
/* -------------------------------------------------------------------------- */

export interface CloneYearModalProps {
  open: boolean;
  schoolYears: SchoolYear[];
  onClose: () => void;
  onDone: (report: DirectoryBatchCreateReport) => Promise<void> | void;
}

export function CloneYearModal({ open, schoolYears, onClose, onDone }: CloneYearModalProps): JSX.Element {
  const [sourceYearId, setSourceYearId] = useState<string>('');
  const [targetYearName, setTargetYearName] = useState(`${new Date().getFullYear() + 1}届`);
  const [sourceClasses, setSourceClasses] = useState<Class[]>([]);
  const [rename, setRename] = useState(false);
  const [template, setTemplate] = useState(DEFAULT_TEMPLATE);
  const [loading, setLoading] = useState(false);
  const [submitting, setSubmitting] = useState(false);

  useEffect(() => {
    if (!open || !sourceYearId) {
      setSourceClasses([]);
      return;
    }
    let cancelled = false;
    setLoading(true);
    void classList(null, sourceYearId)
      .then((list) => {
        if (!cancelled) setSourceClasses(list);
      })
      .catch(() => {
        if (!cancelled) setSourceClasses([]);
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [open, sourceYearId]);

  const groups = useMemo(() => {
    const map = new Map<string, { gradeName: string; classes: Class[] }>();
    sourceClasses.forEach((c) => {
      if (!c.gradeId) return;
      const entry = map.get(c.gradeId) ?? { gradeName: c.gradeName ?? '', classes: [] };
      entry.classes.push(c);
      map.set(c.gradeId, entry);
    });
    return [...map.entries()];
  }, [sourceClasses]);

  const totalClasses = groups.reduce((sum, [, g]) => sum + g.classes.length, 0);
  const canSubmit = sourceYearId !== '' && targetYearName.trim() !== '' && totalClasses > 0 && !submitting;

  const submit = async (): Promise<void> => {
    if (!canSubmit) return;
    setSubmitting(true);
    try {
      const grades: DirectoryBatchGradeInput[] = groups.map(([gradeId, g], i) => ({
        key: gradeId,
        gradeId,
        gradeName: g.gradeName,
        sortOrder: i + 1,
      }));
      const classes: DirectoryBatchClassInput[] = [];
      groups.forEach(([gradeId, g]) => {
        g.classes.forEach((c, index) => {
          const seq = Number.parseInt(c.classNo ?? '', 10) || index + 1;
          classes.push({
            gradeKey: gradeId,
            className: rename ? expandTemplate(template, g.gradeName, seq) : c.className,
            classNo: c.classNo ?? String(seq),
            headTeacher: c.headTeacher,
            sortOrder: c.sortOrder,
          });
        });
      });
      const report = await directoryBatchCreate({
        schoolYear: { schoolYearName: targetYearName.trim(), schoolYearNo: null, startDate: null, endDate: null, sortOrder: 0 },
        grades,
        classes,
      });
      await onDone(report);
      onClose();
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <Modal
      open={open}
      onClose={onClose}
      title="克隆学年结构"
      description="把某个学年的「年级 → 班级」骨架复制到新学年（不含学生名单），班主任一并带过去。"
      widthClass="max-w-2xl"
      footer={
        <>
          <Button variant="secondary" onClick={onClose}>取消</Button>
          <Button icon={<Copy className="h-4 w-4" />} onClick={() => void submit()} disabled={!canSubmit}>
            {submitting ? '克隆中…' : `克隆 ${groups.length} 个年级 · ${totalClasses} 个班级`}
          </Button>
        </>
      }
    >
      <div className="space-y-4">
        <SearchableSelect
          label="源学年"
          options={schoolYears.map((y) => ({ value: y.id, label: y.schoolYearName }))}
          value={sourceYearId}
          onChange={(value) => setSourceYearId(value)}
          placeholder="— 选择要克隆的学年 —"
        />
        <Input
          label="新学年名称（必填）"
          value={targetYearName}
          onChange={(e) => setTargetYearName(e.target.value)}
          placeholder="如：2028届"
        />
        <label className="flex items-center gap-2 text-sm text-ink">
          <input type="checkbox" checked={rename} onChange={(e) => setRename(e.target.checked)} />
          按模板重命名班级（年级名不变时与原名等价）
        </label>
        {rename && (
          <Input
            label="班级命名模板"
            value={template}
            onChange={(e) => setTemplate(e.target.value)}
            placeholder="{年级}{序号}班"
          />
        )}
        <div className="rounded-lg border border-surface-border bg-surface-muted p-3">
          {loading && <p className="text-sm text-ink-muted">正在加载源学年目录…</p>}
          {!loading && sourceYearId === '' && (
            <p className="text-sm text-ink-muted">先选择源学年，这里会显示将被克隆的结构。</p>
          )}
          {!loading && sourceYearId !== '' && totalClasses === 0 && (
            <p className="text-sm text-ink-muted">该学年下没有班级，无可克隆内容。</p>
          )}
          {!loading && groups.length > 0 && (
            <ul className="max-h-40 space-y-1 overflow-y-auto text-sm text-ink-muted">
              {groups.map(([gradeId, g]) => (
                <li key={gradeId}>
                  <span className="font-medium text-ink">{g.gradeName}</span>：{g.classes.length} 个班
                </li>
              ))}
            </ul>
          )}
        </div>
      </div>
    </Modal>
  );
}

/* -------------------------------------------------------------------------- */
/* 批量加班：在某年级下按模板续接班号追加 N 个班                                   */
/* -------------------------------------------------------------------------- */

export interface BatchAddClassesModalProps {
  open: boolean;
  grade: Grade | null;
  /** 目标学年 id（新建班级必填） */
  schoolYearId: string | null;
  /** 该年级在当前学年下已有的班级（用于续接班号） */
  existingClasses: Class[];
  onClose: () => void;
  onDone: (report: DirectoryBatchCreateReport) => Promise<void> | void;
}

export function BatchAddClassesModal({
  open,
  grade,
  schoolYearId,
  existingClasses,
  onClose,
  onDone,
}: BatchAddClassesModalProps): JSX.Element {
  const [count, setCount] = useState('1');
  const [startNo, setStartNo] = useState('1');
  const [template, setTemplate] = useState(DEFAULT_TEMPLATE);
  const [submitting, setSubmitting] = useState(false);

  useEffect(() => {
    if (!open) return;
    const maxNo = existingClasses.reduce((max, c) => {
      const no = Number.parseInt(c.classNo ?? '', 10);
      return Number.isFinite(no) && no > max ? no : max;
    }, 0);
    setStartNo(String(maxNo + 1));
    setTemplate(DEFAULT_TEMPLATE);
    setCount('1');
  }, [open, existingClasses]);

  const n = Number.parseInt(count, 10) || 0;
  const start = Number.parseInt(startNo, 10) || 1;
  const canSubmit = !!grade && !!schoolYearId && n > 0 && !submitting;

  const submit = async (): Promise<void> => {
    if (!canSubmit || !grade) return;
    setSubmitting(true);
    try {
      const classes: DirectoryBatchClassInput[] = [];
      for (let seq = start; seq < start + n; seq += 1) {
        classes.push({
          gradeKey: 'g',
          className: expandTemplate(template, grade.gradeName, seq),
          classNo: String(seq),
          sortOrder: seq,
        });
      }
      const report = await directoryBatchCreate({
        schoolYear: null,
        schoolYearId,
        grades: [{ key: 'g', gradeId: grade.id, gradeName: grade.gradeName }],
        classes,
      });
      await onDone(report);
      onClose();
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <Modal
      open={open}
      onClose={onClose}
      title={`批量加班${grade ? ` · ${grade.gradeName}` : ''}`}
      description="按模板续接现有班号批量追加班级；班号冲突时自动跳过已存在项。"
      widthClass="max-w-lg"
      footer={
        <>
          <Button variant="secondary" onClick={onClose}>取消</Button>
          <Button icon={<ListPlus className="h-4 w-4" />} onClick={() => void submit()} disabled={!canSubmit}>
            {submitting ? '创建中…' : `追加 ${n} 个班级`}
          </Button>
        </>
      }
    >
      <div className="space-y-4">
        {!schoolYearId && (
          <p className="rounded-lg border border-amber-400 bg-amber-50 px-3 py-2 text-sm text-amber-900">
            请先选择学年：新建班级必须归入一个学年。
          </p>
        )}
        <div className="grid min-w-0 grid-cols-1 gap-4 sm:grid-cols-2">
          <Input
            label="追加班数"
            inputMode="numeric"
            value={count}
            onChange={(e) => setCount(e.target.value)}
          />
          <Input
            label="起始班号"
            inputMode="numeric"
            value={startNo}
            onChange={(e) => setStartNo(e.target.value)}
          />
        </div>
        <Input
          label="命名模板"
          value={template}
          onChange={(e) => setTemplate(e.target.value)}
          placeholder="{年级}{序号}班"
        />
        {grade && n > 0 && (
          <div className="rounded-lg border border-surface-border bg-surface-muted p-3">
            <p className="text-sm font-semibold text-ink">预览</p>
            <p className="mt-1 text-sm text-ink-muted">
              {[...Array(Math.min(n, 5)).keys()]
                .map((i) => expandTemplate(template, grade.gradeName, start + i))
                .join('、')}
              {n > 5 ? ` …共 ${n} 个班` : ''}
            </p>
          </div>
        )}
      </div>
    </Modal>
  );
}

/* -------------------------------------------------------------------------- */
/* 换届向导：干跑预览（逐行可调留级）→ 确认单事务执行                             */
/* -------------------------------------------------------------------------- */

const ACTION_LABELS: Record<string, string> = {
  promote: '升级',
  graduate: '毕业',
  retain: '留级',
};

export interface RolloverWizardModalProps {
  open: boolean;
  schoolYears: SchoolYear[];
  grades: Grade[];
  /** 默认源学年（当前选中学年） */
  defaultSourceYearId: string | null;
  onClose: () => void;
  onDone: (report: RolloverReport) => Promise<void> | void;
}

export function RolloverWizardModal({
  open,
  schoolYears,
  grades,
  defaultSourceYearId,
  onClose,
  onDone,
}: RolloverWizardModalProps): JSX.Element {
  const [sourceYearId, setSourceYearId] = useState(defaultSourceYearId ?? '');
  const [newYearName, setNewYearName] = useState('');
  const [graduatingIds, setGraduatingIds] = useState<string[]>([]);
  const [retainedIds, setRetainedIds] = useState<Set<string>>(new Set());
  const [step, setStep] = useState<'form' | 'preview'>('form');
  const [preview, setPreview] = useState<RolloverReport | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    if (!open) return;
    setSourceYearId(defaultSourceYearId ?? '');
    setNewYearName(String(new Date().getFullYear() + 1) + '届');
    setGraduatingIds(grades.length > 0 ? [grades[grades.length - 1].id] : []);
    setRetainedIds(new Set());
    setPreview(null);
    setStep('form');
  }, [open, defaultSourceYearId, grades]);

  const canPreview = sourceYearId !== '' && newYearName.trim() !== '' && !busy;

  const buildRequest = (): RolloverRequest => ({
    sourceSchoolYearId: sourceYearId,
    newSchoolYearName: newYearName.trim(),
    newSchoolYearNo: null,
    newStartDate: null,
    newEndDate: null,
    graduatingGradeIds: graduatingIds,
    retainedStudentIds: [...retainedIds],
  });

  const runPreview = async (): Promise<void> => {
    if (!canPreview) return;
    setBusy(true);
    try {
      const report = await schoolYearRollover(buildRequest(), true);
      setPreview(report);
      setStep('preview');
    } finally {
      setBusy(false);
    }
  };

  const executeRollover = async (): Promise<void> => {
    if (busy) return;
    setBusy(true);
    try {
      const report = await schoolYearRollover(buildRequest(), false);
      await onDone(report);
      onClose();
    } finally {
      setBusy(false);
    }
  };

  const toggleRetain = (studentId: string): void => {
    setRetainedIds((prev) => {
      const next = new Set(prev);
      if (next.has(studentId)) next.delete(studentId);
      else next.add(studentId);
      return next;
    });
  };

  return (
    <Modal
      open={open}
      onClose={onClose}
      title="学年换届向导"
      description="克隆班级目录到新学年并按目标年级自动改名（一年级1班 → 二年级1班）、学生整体升一级、毕业年级原地保留、教室重绑到新班级。预览确认后单事务执行。"
      widthClass="max-w-3xl"
      footer={
        step === 'form' ? (
          <>
            <Button variant="secondary" onClick={onClose}>取消</Button>
            <Button icon={<ArrowRightLeft className="h-4 w-4" />} onClick={() => void runPreview()} disabled={!canPreview}>
              {busy ? '计算中…' : '生成换届预览'}
            </Button>
          </>
        ) : (
          <>
            <Button variant="secondary" onClick={() => setStep('form')} disabled={busy}>返回调整</Button>
            <Button onClick={() => void executeRollover()} disabled={busy}>
              {busy ? '执行中…' : '确认换届（单事务执行）'}
            </Button>
          </>
        )
      }
    >
      {step === 'form' && (
        <div className="space-y-4">
          <SearchableSelect
            label="源学年"
            options={schoolYears.map((y) => ({ value: y.id, label: y.schoolYearName }))}
            value={sourceYearId}
            onChange={(value) => setSourceYearId(value)}
            placeholder="— 选择要换届的学年 —"
          />
          <Input
            label="新学年名称（必填）"
            value={newYearName}
            onChange={(e) => setNewYearName(e.target.value)}
            placeholder="如：2028届"
          />
          <div>
            <p className="mb-2 text-sm font-semibold text-ink">毕业年级（可多选）</p>
            <div className="flex flex-wrap gap-2">
              {grades.map((g, i) => {
                const isLast = i === grades.length - 1;
                const checked = graduatingIds.includes(g.id);
                return (
                  <label
                    key={g.id}
                    className={[
                      'flex cursor-pointer items-center gap-2 rounded-lg border px-3 py-1.5 text-sm',
                      checked ? 'border-brand-500 bg-brand-50 text-ink' : 'border-surface-border text-ink-muted',
                    ].join(' ')}
                  >
                    <input
                      type="checkbox"
                      checked={checked}
                      onChange={() =>
                        setGraduatingIds((prev) =>
                          prev.includes(g.id) ? prev.filter((id) => id !== g.id) : [...prev, g.id],
                        )
                      }
                    />
                    {g.gradeName}
                    {isLast && <span className="text-ink-muted">（最高年级）</span>}
                  </label>
                );
              })}
            </div>
            <p className="mt-2 text-sm text-ink-muted">
              毕业年级的学生原地保留在原学年班级，不出现在新学年名册中。
            </p>
          </div>
        </div>
      )}

      {step === 'preview' && preview && (
        <div className="space-y-4">
          <div className="grid grid-cols-2 gap-3 md:grid-cols-5">
            <StatCard label="升级学生" value={preview.promoteCount} />
            <StatCard label="毕业学生" value={preview.graduateCount} />
            <StatCard label="留级学生" value={preview.retainCount} />
            <StatCard label="新建班级" value={preview.classesCreated} />
            <StatCard label="教室重绑" value={preview.rebindCount} />
          </div>
          {preview.warnings.length > 0 && (
            <ul className="list-disc space-y-1 rounded-lg border border-amber-400 bg-amber-50 px-6 py-3 text-sm text-amber-900">
              {preview.warnings.map((w) => (
                <li key={w}>{w}</li>
              ))}
            </ul>
          )}
          {preview.rebindPlans.length > 0 && (
            <div className="rounded-lg border border-surface-border bg-surface-muted p-3">
              <p className="text-sm font-semibold text-ink">教室重绑</p>
              <ul className="mt-1 max-h-24 space-y-1 overflow-y-auto text-sm text-ink-muted">
                {preview.rebindPlans.map((r) => (
                  <li key={r.classroomId}>
                    教室「{r.roomName}」：{r.fromClass} → {r.toClass}
                  </li>
                ))}
              </ul>
            </div>
          )}
          <div className="rounded-lg border border-surface-border p-3">
            <p className="mb-2 text-sm font-semibold text-ink">
              学生名单（{preview.studentPlans.length} 人）— 勾选「留级」可逐行调整，该生保持不动
            </p>
            <div className="max-h-64 overflow-y-auto">
              <table className="w-full text-sm">
                <thead className="sticky top-0 bg-surface-raised text-left text-ink-muted">
                  <tr>
                    <th className="px-2 py-1.5">学号</th>
                    <th className="px-2 py-1.5">姓名</th>
                    <th className="px-2 py-1.5">原班级</th>
                    <th className="px-2 py-1.5">动作</th>
                    <th className="px-2 py-1.5">去向</th>
                    <th className="px-2 py-1.5">留级</th>
                  </tr>
                </thead>
                <tbody>
                  {preview.studentPlans.map((p) => {
                    const retained = retainedIds.has(p.studentId);
                    const action = retained ? 'retain' : p.action;
                    return (
                      <tr key={p.studentId} className="border-t border-surface-border text-ink">
                        <td className="px-2 py-1.5">{p.studentNo}</td>
                        <td className="px-2 py-1.5">{p.name}</td>
                        <td className="px-2 py-1.5">{p.fromGrade} · {p.fromClass}</td>
                        <td className="px-2 py-1.5">{ACTION_LABELS[action] ?? action}</td>
                        <td className="px-2 py-1.5 text-ink-muted">
                          {action === 'promote' ? `${p.toGrade} · ${p.toClass}` : '—'}
                        </td>
                        <td className="px-2 py-1.5">
                          <input
                            type="checkbox"
                            checked={retained}
                            onChange={() => toggleRetain(p.studentId)}
                            aria-label={'留级：' + p.name}
                          />
                        </td>
                      </tr>
                    );
                  })}
                </tbody>
              </table>
            </div>
          </div>
        </div>
      )}
    </Modal>
  );
}

function StatCard({ label, value }: { label: string; value: number }): JSX.Element {
  return (
    <div className="rounded-lg border border-surface-border bg-surface-muted p-3">
      <p className="text-sm text-ink-muted">{label}</p>
      <p className="text-2xl font-bold text-ink">{value}</p>
    </div>
  );
}
