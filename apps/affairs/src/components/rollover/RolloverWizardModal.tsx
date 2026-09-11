import { useCallback, useEffect, useMemo, useState } from 'react';
import { Modal } from '@shared/components/ui/Modal';
import { Button } from '@shared/components/ui/Button';
import { Select } from '@shared/components/ui/Select';
import { Input } from '@shared/components/ui/Input';
import { buildStudentTemplate, parseStudentFile, STUDENT_TEMPLATE_HEADERS } from '@shared/lib/excel';
import { exportSheetsToXlsx } from '@shared/lib/exporter';
import {
  classList,
  rolloverFromExcel,
  schoolYearList,
  type RolloverBindingChoice,
  type RolloverExcelReport,
  type StudentImportRowPayload,
} from '@shared/lib/db';
import type { SchoolYear } from '@shared/types/models';
import { cn } from '@shared/lib/cn';

export interface RolloverWizardModalProps {
  open: boolean;
  mode: 'rollover' | 'init';
  onClose: () => void;
  onDone: () => void;
}

const STEPS = ['学年', '名册', '教室绑定', '执行'] as const;

function suggestYearName(source: string | undefined): string {
  if (!source) return '';
  const m = source.match(/(\d{4})\s*[-–~]\s*(\d{4})/);
  if (m) return `${Number(m[1]) + 1}-${Number(m[2]) + 1}学年`;
  return `${new Date().getFullYear()}-${new Date().getFullYear() + 1}学年`;
}

/** 教务端换届/建校向导：Excel 上传为主，4 步流水线（设计 §3）。 */
export function RolloverWizardModal({ open, mode, onClose, onDone }: RolloverWizardModalProps): JSX.Element {
  const [step, setStep] = useState(0);
  const [sourceYearId, setSourceYearId] = useState('');
  const [years, setYears] = useState<SchoolYear[]>([]);
  const [newYearName, setNewYearName] = useState('');
  const [startDate, setStartDate] = useState('');
  const [endDate, setEndDate] = useState('');
  const [rows, setRows] = useState<StudentImportRowPayload[]>([]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [preview, setPreview] = useState<RolloverExcelReport | null>(null);
  const [choices, setChoices] = useState<RolloverBindingChoice[]>([]);
  /** 已决策的教室（含显式「暂不绑定」）：conflict 行只有决策后才不阻塞执行。 */
  const [decided, setDecided] = useState<Set<string>>(new Set());
  const [result, setResult] = useState<RolloverExcelReport | null>(null);
  const [classes, setClasses] = useState<{ id: string; label: string }[]>([]);

  useEffect(() => {
    if (!open) return;
    setStep(0); setRows([]); setPreview(null); setResult(null); setError(null); setChoices([]); setDecided(new Set());
    void (async () => {
      const list = await schoolYearList().catch(() => []);
      setYears(list);
      // rollover 模式默认源学年 = 目录中最近创建的一个
      const first = list[0];
      setSourceYearId(first?.id ?? '');
      setNewYearName(suggestYearName(first?.schoolYearName));
    })();
  }, [open]);

  // 初始全部未决策：auto 建议预填（有 id 用 id，仅名字则记 className 待执行解析），
  // conflict/none 不预填，由用户逐行仲裁。
  const applyAutoSuggestions = useCallback((p: RolloverExcelReport) => {
    setChoices(p.bindingSuggestions.map((s) => ({
      classroomId: s.classroomId,
      classId: s.matchKind === 'auto' ? (s.suggestedClassId ?? null) : null,
      className: s.matchKind === 'auto' && !s.suggestedClassId ? (s.suggestedClass ?? null) : null,
    })));
    setDecided(new Set());
  }, []);

  const doPreview = async (): Promise<void> => {
    if (mode === 'rollover' && !sourceYearId) {
      setError('请先选择源学年');
      return;
    }
    setBusy(true); setError(null);
    try {
      const p = await rolloverFromExcel({
        mode,
        sourceSchoolYearId: mode === 'rollover' ? sourceYearId : null,
        newSchoolYearName: newYearName.trim(),
        newStartDate: startDate || null,
        newEndDate: endDate || null,
        rows,
      }, true);
      setPreview(p);
      applyAutoSuggestions(p);
      // 有错误行时同样进入预览页：错误明细 + 修正重传入口都在 Step ③ 页展示
      setStep(2);
    } catch (err) {
      setError((err as Error).message || '预览失败');
    } finally { setBusy(false); }
  };

  const doExecute = async (): Promise<void> => {
    setBusy(true); setError(null);
    try {
      const r = await rolloverFromExcel({
        mode,
        sourceSchoolYearId: mode === 'rollover' ? sourceYearId : null,
        newSchoolYearName: newYearName.trim(),
        newStartDate: startDate || null,
        newEndDate: endDate || null,
        rows,
        confirmBindings: choices,
      }, false);
      setResult(r);
      setStep(3);
    } catch (err) {
      setError((err as Error).message || '执行失败');
    } finally { setBusy(false); }
  };

  const onFile = async (file: File): Promise<void> => {
    setBusy(true); setError(null);
    try {
      const parsed = await parseStudentFile(file);
      const list = parsed.parsed.map((r) => ({
        studentNo: r.studentNo,
        name: r.name,
        gender: r.gender || null,
        grade: r.grade || null,
        className: r.className || null,
        seatNo: r.seatNo,
        phone: r.phone || null,
        note: r.note || null,
      }));
      setRows(list);
    } catch (err) {
      setError((err as Error).message || '解析失败');
    } finally { setBusy(false); }
  };

  const downloadErrors = async (): Promise<void> => {
    if (!preview) return;
    await exportSheetsToXlsx('换届错误行.xlsx', [{
      name: '错误行',
      columns: ['Excel行号', '学号', '姓名', '原因'],
      rows: preview.errors.map((e) => [e.rowIndex, e.studentNo, e.name, e.reason]),
    }]);
  };

  const downloadTemplate = async (): Promise<void> => {
    await exportSheetsToXlsx('整校名册模板.xlsx', [{
      name: '名册',
      columns: [...STUDENT_TEMPLATE_HEADERS],
      // buildStudentTemplate 首行即表头，去掉避免与 columns 重复
      rows: buildStudentTemplate().slice(1),
    }]);
  };

  // 下拉决策：空值 = 显式「暂不绑定」；任一操作均记入 decided（解除 conflict 阻塞）。
  const setChoice = (classroomId: string, value: string): void => {
    setChoices((prev) => prev.map((c) => (c.classroomId === classroomId
      ? (value ? { ...c, classId: value, className: null } : { ...c, classId: null, className: null })
      : c)));
    setDecided((prev) => new Set(prev).add(classroomId));
  };
  // 一键接受全部 auto 建议：有 id 用 id，仅名字则按名提交（执行时解析）；conflict 不自动接受。
  const acceptAll = (): void => {
    if (!preview) return;
    const next = new Map<string, RolloverBindingChoice>();
    for (const s of preview.bindingSuggestions) {
      if (s.matchKind !== 'auto') continue;
      next.set(s.classroomId, s.suggestedClassId
        ? { classroomId: s.classroomId, classId: s.suggestedClassId, className: null }
        : { classroomId: s.classroomId, classId: null, className: s.suggestedClass });
    }
    setChoices((prev) => prev.map((c) => next.get(c.classroomId) ?? c));
    setDecided((prev) => new Set([...prev, ...next.keys()]));
  };

  // Step ③ 需要的可选班级下拉（新学年目录）。
  useEffect(() => {
    if (step !== 2 || !preview) return;
    void (async () => {
      const list = await classList(null, null).catch(() => []);
      setClasses(list
        .filter((c) => c.schoolYearId === (result?.newSchoolYearId ?? preview.newSchoolYearId))
        .map((c) => ({ id: c.id, label: `${c.gradeName ?? ''}${c.className}` })));
    })();
  }, [step, preview, result]);

  // conflict 行只有在用户显式决策（选定班级或「暂不绑定」）后才不阻塞执行。
  const conflictsRemain = useMemo(() => {
    if (!preview) return false;
    return preview.bindingSuggestions.some((s) => s.matchKind === 'conflict' && !decided.has(s.classroomId));
  }, [preview, decided]);

  return (
    <Modal open={open} onClose={onClose} widthClass="max-w-3xl" title={mode === 'init' ? '首次建校向导' : '新学年换届向导'}
      description={mode === 'init'
        ? '上传整校名册 Excel，自动生成年级、班级与学生名册；教室绑定可现在确认或之后在设备认领后补绑。'
        : '上传新学年整校 Excel 生成新学年数据，教室绑定批量确认后单事务执行；旧学年数据原地保留。'}>
      <ol className="mb-5 flex gap-2 text-sm">
        {STEPS.map((label, i) => (
          <li key={label} className={cn(
            'flex-1 rounded-md border px-2 py-1 text-center',
            i === step ? 'border-brand-400 bg-brand-50 text-ink font-medium' : 'border-surface-border text-ink-muted',
          )}>{i + 1}. {label}</li>
        ))}
      </ol>

      {step === 0 && (
        <div className="space-y-4">
          {mode === 'rollover' && (
            <Select
              label="源学年（换届自）"
              value={sourceYearId}
              onChange={(e) => {
                setSourceYearId(e.target.value);
                setNewYearName(suggestYearName(years.find((y) => y.id === e.target.value)?.schoolYearName));
              }}
              options={years.map((y) => ({ value: y.id, label: y.schoolYearName }))}
            />
          )}
          <Input label="新学年名称" value={newYearName} onChange={(e) => setNewYearName(e.target.value)} placeholder="2026-2027学年" />
          <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
            <Input label="开始日期（可选）" value={startDate} onChange={(e) => setStartDate(e.target.value)} placeholder="2026-09-01" />
            <Input label="结束日期（可选）" value={endDate} onChange={(e) => setEndDate(e.target.value)} placeholder="2027-07-15" />
          </div>
          <div className="flex justify-end gap-2">
            <Button variant="secondary" onClick={onClose}>取消</Button>
            <Button disabled={!newYearName.trim()} onClick={() => setStep(1)}>下一步</Button>
          </div>
        </div>
      )}

      {step === 1 && (
        <div className="space-y-4">
          <div className="flex flex-wrap items-center gap-2">
            <label className="cursor-pointer rounded-md border border-surface-border px-3 py-2 text-sm text-ink hover:bg-surface-muted">
              选择 Excel / CSV 文件
              <input type="file" accept=".xlsx,.xls,.csv" className="hidden"
                onChange={(e) => { const f = e.target.files?.[0]; if (f) void onFile(f); }} />
            </label>
            <Button variant="secondary" size="md" onClick={() => void downloadTemplate()}>下载模板</Button>
            {rows.length > 0 && <span className="text-sm text-ink-muted">已解析 {rows.length} 行</span>}
          </div>
          {error && <p className="text-sm text-red-600">{error}</p>}
          <div className="flex justify-between">
            <Button variant="secondary" onClick={() => setStep(0)}>上一步</Button>
            <Button disabled={rows.length === 0 || busy} loading={busy} onClick={() => void doPreview()}>
              {busy ? '计算中…' : '生成预览'}
            </Button>
          </div>
        </div>
      )}

      {step === 2 && preview && (
        <div className="space-y-4">
          <div className="rounded-lg bg-surface-muted p-3 text-sm text-ink space-y-1">
            <p>将新增 {preview.directory.newGrades.length} 个年级、{preview.directory.newClasses.length} 个班级；学生 {rows.length} 人。</p>
            {preview.studentsMissing > 0 && <p className="text-amber-700">{preview.studentsMissing} 名原学年学生未出现在 Excel 中（保留原样，不做删除）。</p>}
            {preview.directory.untouchedClasses.length > 0 && <p className="text-ink-muted">未涉及班级（保留）：{preview.directory.untouchedClasses.join('、')}</p>}
          </div>
          {preview.errors.length > 0 ? (
            <div className="rounded-lg border border-red-300 bg-red-50 p-3 text-sm">
              <p className="font-medium text-ink">{preview.errors.length} 行数据有误，修正后重新上传：</p>
              <ul className="mt-1 max-h-40 overflow-auto text-red-700">
                {preview.errors.slice(0, 20).map((e, i) => (
                  <li key={i}>第 {e.rowIndex} 行 {e.studentNo} {e.name}：{e.reason}</li>
                ))}
              </ul>
              <div className="mt-2 flex gap-2">
                <Button variant="secondary" size="md" onClick={() => void downloadErrors()}>下载错误报告</Button>
                <Button variant="secondary" size="md" onClick={() => setStep(1)}>重新上传</Button>
              </div>
            </div>
          ) : (
            <>
              {preview.bindingSuggestions.length > 0 && (
                <div className="space-y-2">
                  <div className="flex items-center justify-between">
                    <p className="text-sm font-medium text-ink">教室绑定（Step ③）</p>
                    <Button size="md" variant="secondary" onClick={acceptAll}>全部接受建议</Button>
                  </div>
                  <div className="max-h-56 overflow-auto rounded-lg border border-surface-border">
                    <table className="w-full text-sm">
                      <thead className="bg-surface-muted text-left text-ink-muted">
                        <tr><th className="p-2">教室</th><th className="p-2">旧绑定</th><th className="p-2">新绑定</th></tr>
                      </thead>
                      <tbody>
                        {preview.bindingSuggestions.map((s) => {
                          const chosen = choices.find((c) => c.classroomId === s.classroomId);
                          const isConflict = s.matchKind !== 'auto';
                          return (
                            <tr key={s.classroomId} className="border-t border-surface-border">
                              <td className="p-2 text-ink">{s.roomName}</td>
                              <td className="p-2 text-ink-muted">{s.oldClass ?? '—'}</td>
                              <td className="p-2">
                                <select
                                  className={cn('w-full rounded border bg-surface-raised px-2 py-1 text-ink min-w-0',
                                    isConflict && !decided.has(s.classroomId) ? 'border-amber-500' : 'border-surface-border')}
                                  value={chosen?.classId ?? ''}
                                  onChange={(e) => setChoice(s.classroomId, e.target.value)}
                                >
                                  <option value="">暂不绑定</option>
                                  {classes.map((c) => <option key={c.id} value={c.id}>{c.label}</option>)}
                                </select>
                                {s.suggestedClass && (
                                  <p className="mt-0.5 text-xs text-ink-muted">建议：{s.suggestedClass}</p>
                                )}
                              </td>
                            </tr>
                          );
                        })}
                      </tbody>
                    </table>
                  </div>
                  {conflictsRemain && <p className="text-sm text-amber-700">存在冲突或未匹配教室，请逐行仲裁（或选择「暂不绑定」）。</p>}
                </div>
              )}
              <div className="flex justify-between">
                <Button variant="secondary" onClick={() => setStep(1)}>重新上传</Button>
                <Button disabled={conflictsRemain} loading={busy} onClick={() => void doExecute()}>
                  {busy ? '执行中…' : `确认执行（单事务）`}
                </Button>
              </div>
            </>
          )}
          {error && <p className="text-sm text-red-600">{error}</p>}
        </div>
      )}

      {step === 3 && result && (
        <div className="space-y-4 text-sm text-ink">
          <div className="rounded-lg bg-surface-muted p-3 space-y-1">
            <p className="font-medium">换届完成：{result.newSchoolYearName}</p>
            <p>新增年级 {result.createdGrades.length} · 新增班级 {result.createdClasses.length} · 学生落位 {result.studentsAdded + result.studentsUpdated} 人（新增 {result.studentsAdded} / 更新 {result.studentsUpdated}）</p>
            <p className="text-ink-muted">已设为当前学年。教室端将在下次目录同步后自动切换绑定。</p>
            {mode === 'rollover' && <p className="text-ink-muted">旧学年数据已原地保留，可在学年选择器「历史学年」分组查看；建议导出快照留存。</p>}
          </div>
          <div className="flex justify-end gap-2">
            <Button variant="secondary" onClick={() => void downloadSchSnapshot(result.newSchoolYearId)}>导出 .sch 快照</Button>
            <Button onClick={() => { onDone(); onClose(); }}>完成</Button>
          </div>
        </div>
      )}
    </Modal>
  );
}

/** 收尾页一键导出：用系统保存对话框选路径后走 package_export_sch。 */
async function downloadSchSnapshot(_yearId: string): Promise<void> {
  const { save } = await import('@tauri-apps/plugin-dialog');
  const path = await save({ defaultPath: `backup-${new Date().toISOString().slice(0, 10)}.sch` });
  if (!path) return;
  const { packageExportSch } = await import('@shared/lib/db');
  await packageExportSch('school', null, path);
}
