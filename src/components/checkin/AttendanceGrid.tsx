import { useEffect, useMemo } from 'react';
import { useStudentStore } from '@/store/useStudentStore';
import { useCheckinStore } from '@/store/useCheckinStore';
import {
  CHECKIN_STATUS_META,
  CHECKIN_STATES_ALL,
  PERIOD_OPTIONS,
} from '@/constants/status';
import { toDateKey } from '@/lib/format';
import type { CheckinState } from '@/types/enums';
import { Select } from '@/components/ui/Select';
import { Input } from '@/components/ui/Input';
import { Button } from '@/components/ui/Button';

export interface AttendanceGridProps {
  /** 紧凑模式：用于首页嵌入式展示 */
  compact?: boolean;
}

/**
 * 反向考勤网格（班级端核心交互）。
 *   - 本地无记录即视为出勤（present）；
 *   - 点击卡片循环：present → leave → absent → present（late 为显式第四态，由长按/详情设置）；
 *   - 数据源：useStudentStore.roster + useCheckinStore.records，标记走乐观更新 + 落库。
 */
export function AttendanceGrid({ compact = false }: AttendanceGridProps): JSX.Element {
  const students = useStudentStore((s) => s.students);
  const records = useCheckinStore((s) => s.records);
  const loading = useCheckinStore((s) => s.loading);
  const date = useCheckinStore((s) => s.date);
  const period = useCheckinStore((s) => s.period);
  const setDate = useCheckinStore((s) => s.setDate);
  const setPeriod = useCheckinStore((s) => s.setPeriod);
  const load = useCheckinStore((s) => s.load);
  const cycle = useCheckinStore((s) => s.cycle);
  const batchSetState = useCheckinStore((s) => s.batchSetState);

  const roster = useMemo(
    () => students.filter((s) => s.status !== 'transferred'),
    [students],
  );

  useEffect(() => {
    void load(roster, date, period);
  }, [load, roster, date, period]);

  const rows = useMemo(
    () =>
      roster.map((student) => ({
        student,
        effectiveState: (records[student.id]?.state ?? 'present') as CheckinState,
      })),
    [roster, records],
  );

  const summary = useMemo(() => {
    const acc: Record<CheckinState, number> = { present: 0, leave: 0, absent: 0, late: 0 };
    rows.forEach((r) => {
      acc[r.effectiveState] += 1;
    });
    const total = roster.length;
    const attended = acc.present + acc.late;
    return {
      acc,
      total,
      attended,
      rate: total > 0 ? Math.round((attended / total) * 1000) / 10 : 0,
    };
  }, [rows, roster.length]);

  return (
    <div className="space-y-4">
      <div className="flex flex-wrap items-end gap-3">
        <Input
          type="date"
          label="日期"
          value={date}
          onChange={(e) => setDate(e.target.value || toDateKey(Date.now()))}
        />
        <Select
          label="时段"
          options={PERIOD_OPTIONS}
          value={period}
          onChange={(e) => setPeriod(e.target.value as typeof period)}
        />
        <Button variant="secondary" onClick={() => void batchSetState(roster, 'present')}>
          一键全勤
        </Button>
      </div>

      <div className="flex flex-wrap items-center gap-x-4 gap-y-1">
        <span className="text-base font-semibold text-ink-soft">
          出勤率 <span className="text-brand-700">{summary.rate}%</span>
        </span>
        {CHECKIN_STATES_ALL.map((st) => (
          <span key={st} className="inline-flex items-center gap-1 text-sm">
            <span
              className="inline-block h-3 w-3 rounded-full"
              style={{ backgroundColor: CHECKIN_STATUS_META[st].color }}
            />
            {CHECKIN_STATUS_META[st].label} {summary.acc[st]}
          </span>
        ))}
      </div>

      {loading && roster.length > 0 && <p className="text-sm text-ink-muted">加载考勤中…</p>}

      {roster.length === 0 ? (
        <p className="py-8 text-center text-ink-muted">暂无学生名册，请先在「学生名册」中导入。</p>
      ) : (
        <div
          className={
            compact
              ? 'grid grid-cols-3 gap-2 board:grid-cols-6'
              : 'grid grid-cols-2 gap-3 sm:grid-cols-3 board:grid-cols-6 wall:grid-cols-8'
          }
        >
          {rows.map(({ student, effectiveState }) => {
            const meta = CHECKIN_STATUS_META[effectiveState];
            return (
              <button
                key={student.id}
                type="button"
                onClick={() => void cycle(student)}
                className="flex min-h-touch flex-col items-center justify-center gap-1 rounded-lg border-2 bg-surface-raised p-3 text-center transition-colors hover:brightness-95"
                style={{ borderColor: meta.color }}
                title="点击切换：出勤 → 请假 → 缺勤 → 出勤"
              >
                <span className="text-2xl leading-none" style={{ color: meta.color }}>
                  {meta.emoji}
                </span>
                <span className="truncate text-base font-semibold text-ink">{student.name}</span>
                <span className="text-sm" style={{ color: meta.color }}>
                  {meta.label}
                </span>
              </button>
            );
          })}
        </div>
      )}
    </div>
  );
}
