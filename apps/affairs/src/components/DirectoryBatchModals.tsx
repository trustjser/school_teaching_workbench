import { useEffect, useMemo, useState } from 'react';
import { Copy, ListPlus } from 'lucide-react';
import { Modal } from '@shared/components/ui/Modal';
import { Button } from '@shared/components/ui/Button';
import { Input } from '@shared/components/ui/Input';
import { SearchableSelect } from '@shared/components/ui/SearchableSelect';
import {
  classList,
  directoryBatchCreate,
  type DirectoryBatchCreateReport,
  type DirectoryBatchClassInput,
  type DirectoryBatchGradeInput,
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
