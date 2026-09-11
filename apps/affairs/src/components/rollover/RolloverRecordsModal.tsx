import { useEffect, useState } from 'react';
import { Modal } from '@shared/components/ui/Modal';
import { rolloverExecutionsList, rolloverRebind, classList, schoolYearList, type RolloverExecution } from '@shared/lib/db';
import { useAppStore } from '@shared/store/useAppStore';

interface BindingSuggestion {
  classroomId: string;
  roomName: string;
  suggestedClass: string | null;
}

interface ExecutionSummary {
  rebind?: unknown;
  bindingSuggestions?: BindingSuggestion[];
  /** execute 实际落库的教室绑定 (classroomId, classId)。 */
  appliedBindings?: [string, string][];
}

function parseSummary(raw: string): ExecutionSummary | null {
  try {
    const parsed = JSON.parse(raw) as ExecutionSummary;
    return parsed && typeof parsed === 'object' ? parsed : null;
  } catch {
    return null;
  }
}

function modeLabel(mode: string): string {
  if (mode === 'rebind') return '绑定修正';
  if (mode === 'init') return '首次建校';
  return '学年换届';
}

/**
 * 换届执行记录：展示最近 50 条换届 / 建校 / 修正记录。
 * 对带 bindingSuggestions 的记录，可一键把教室改绑到已有班级，教室端同步后自动对齐。
 */
export function RolloverRecordsModal({ open, onClose }: { open: boolean; onClose: () => void }): JSX.Element {
  const [records, setRecords] = useState<RolloverExecution[]>([]);
  const [classes, setClasses] = useState<{ id: string; label: string }[]>([]);
  const [busyId, setBusyId] = useState<string | null>(null);
  const pushToast = useAppStore((s) => s.pushToast);

  const reload = async (): Promise<void> => {
    const [rs, cl, ys] = await Promise.all([
      rolloverExecutionsList().catch(() => []),
      classList(null, null).catch(() => []),
      schoolYearList().catch(() => []),
    ]);
    setRecords(rs);
    const yName = new Map(ys.map((y) => [y.id, y.schoolYearName]));
    setClasses(
      cl.map((c) => ({
        id: c.id,
        label: `${yName.get(c.schoolYearId ?? '') ?? ''} ${c.gradeName ?? ''}${c.className}`,
      })),
    );
  };

  useEffect(() => {
    if (open) void reload();
  }, [open]);

  const rebind = async (classroomId: string, classId: string): Promise<void> => {
    setBusyId(classroomId);
    try {
      await rolloverRebind(classroomId, classId);
      pushToast({ kind: 'success', title: '绑定已修正', description: '教室端下次同步目录后自动对齐' });
      await reload();
    } catch (err) {
      pushToast({ kind: 'error', title: '修正失败', description: (err as Error).message });
    } finally {
      setBusyId(null);
    }
  };

  return (
    <Modal
      open={open}
      onClose={onClose}
      title="换届执行记录"
      description="最近 50 条换届 / 建校 / 修正记录；修正教室绑定后教室端自动对齐。"
    >
      <div className="space-y-3">
        {records.length === 0 && <p className="text-sm text-ink-muted">暂无记录。</p>}
        {records.map((rec) => {
          const summary = parseSummary(rec.summaryJson);
          return (
            <div key={rec.id} className="rounded-lg border border-surface-border p-3">
              <p className="text-sm font-medium text-ink">
                {modeLabel(rec.mode)} · {new Date(rec.executedAt).toLocaleString()}
              </p>
              {!summary ? (
                <p className="mt-1 break-all text-sm text-ink-muted">{rec.summaryJson}</p>
              ) : summary.bindingSuggestions?.length ? (
                <table className="mt-2 w-full text-sm">
                  <tbody>
                    {summary.bindingSuggestions.map((s) => (
                      <tr key={s.classroomId} className="border-t border-surface-border">
                        <td className="py-1 pr-2 text-ink">{s.roomName}</td>
                        <td className="py-1 pr-2 text-ink-muted">{s.suggestedClass ?? '未绑定'}</td>
                        <td className="py-1">
                          <select
                            className="min-w-0 rounded border border-surface-border bg-surface-raised px-2 py-1 text-ink"
                            disabled={busyId === s.classroomId}
                            defaultValue=""
                            onChange={(e) => {
                              if (e.target.value) void rebind(s.classroomId, e.target.value);
                            }}
                          >
                            <option value="">改绑到…</option>
                            {classes.map((c) => (
                              <option key={c.id} value={c.id}>
                                {c.label.trim()}
                              </option>
                            ))}
                          </select>
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              ) : summary.rebind ? (
                <p className="mt-1 break-all text-sm text-ink-muted">{JSON.stringify(summary.rebind)}</p>
              ) : null}
              {summary?.appliedBindings && summary.appliedBindings.length > 0 && (
                <div className="mt-2">
                  <p className="text-sm font-medium text-ink">实际绑定</p>
                  <ul className="mt-1 space-y-0.5 text-sm text-ink-muted">
                    {summary.appliedBindings.map(([classroomId, classId]) => {
                      const room = summary.bindingSuggestions?.find((s) => s.classroomId === classroomId)?.roomName
                        ?? classroomId.slice(0, 8);
                      return <li key={classroomId}>{room} → {classId.slice(0, 8)}</li>;
                    })}
                  </ul>
                </div>
              )}
            </div>
          );
        })}
      </div>
    </Modal>
  );
}
