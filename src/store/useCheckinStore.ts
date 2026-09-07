import { create } from 'zustand';
import type { CheckinPeriod, CheckinState } from '@/types/enums';
import type { CheckinRecord, CheckinRow, DailySummary, Student } from '@/types/models';
import { checkinBatchMark, checkinList, checkinMark } from '@/lib/db';
import { nextCheckinState } from '@/constants/status';
import { toDateKey } from '@/lib/format';
import { useAppStore } from './useAppStore';
import { useStudentStore } from './useStudentStore';

/**
 * 反向考勤核心状态。
 *
 * 设计要点：
 *  1. 「本地无记录即视为出勤（present）」——加载时不写库，仅在渲染层用虚拟记录表达；
 *  2. 一旦点击即落库（checkin_mark），并写入 records 缓存；
 *  3. 点击采用乐观更新：先改变本地状态，invoke 失败则回滚到点击前的快照并 Toast；
 *  4. 状态循环顺序：present → leave → absent → present（late 为显式第四态）。
 */

export interface CheckinSummary {
  total: number;
  present: number;
  leave: number;
  absent: number;
  late: number;
  marked: number;
  attendanceRate: number;
}

interface CheckinState_ {
  date: string;
  period: CheckinPeriod;
  periodLabel: string | null;
  /** studentId -> 记录（仅保存"已落库"的记录） */
  records: Record<string, CheckinRecord>;
  loading: boolean;
  saving: Record<string, boolean>;
  summaryCache: DailySummary | null;
  /** 最近一次操作失败的提示（用于 UI 高亮） */
  lastError: string | null;

  setDate: (date: string) => void;
  setPeriod: (period: CheckinPeriod, label?: string | null) => void;
  load: (students: Student[], date?: string, period?: CheckinPeriod) => Promise<void>;
  /** 循环切换状态（点击卡片主入口） */
  cycle: (student: Student) => Promise<void>;
  /** 显式设置状态（late 等） */
  setState: (student: Student, state: CheckinState) => Promise<void>;
  /** 批量设置（一键全勤 / 异常名单复核） */
  batchSetState: (students: Student[], state: CheckinState) => Promise<void>;
  /** 取有效状态（无记录 → present） */
  effectiveState: (studentId: string) => CheckinState;
  /** 构建渲染行 */
  buildRows: (students: Student[]) => CheckinRow[];
  /** 计算汇总 */
  summary: (students: Student[]) => CheckinSummary;
  reset: () => void;
}

export const useCheckinStore = create<CheckinState_>((set, get) => ({
  date: toDateKey(Date.now()),
  period: 'am',
  periodLabel: null,
  records: {},
  loading: false,
  saving: {},
  summaryCache: null,
  lastError: null,

  setDate: (date) => set({ date }),
  setPeriod: (period, label) => set({ period, periodLabel: label ?? null }),

  load: async (students, date, period) => {
    const app = useAppStore.getState();
    const targetDate = date ?? get().date;
    const targetPeriod = period ?? get().period;
    set({ loading: true, lastError: null });
    try {
      const list = await checkinList(targetDate, targetPeriod);
      const map: Record<string, CheckinRecord> = {};
      list.forEach((r) => {
        map[r.studentId] = r;
      });
      set({ date: targetDate, period: targetPeriod, records: map });
    } catch (err) {
      app.toastError(err, '加载考勤失败');
      set({ lastError: '加载考勤失败' });
      // 降级：保留已有缓存，界面仍可离线标记
      set({ date: targetDate, period: targetPeriod });
    } finally {
      set({ loading: false });
    }
  },

  cycle: async (student) => {
    const current = get().effectiveState(student.id);
    const next = nextCheckinState(current);
    await get().setState(student, next);
  },

  setState: async (student, nextState) => {
    const app = useAppStore.getState();
    const { date, period, periodLabel } = get();
    const prevRecord = get().records[student.id] ?? null;
    const prevState = prevRecord?.state ?? 'present';

    if (prevState === nextState && prevRecord) return;

    // ---- ① 乐观更新：立即构造虚拟记录并写入缓存 ----
    const now = Date.now();
    const optimistic: CheckinRecord = prevRecord
      ? { ...prevRecord, state: nextState, markedAt: now, updatedAt: now }
      : {
          id: `tmp-${student.id}-${date}-${period}`,
          studentId: student.id,
          checkinDate: date,
          period,
          periodLabel: periodLabel ?? null,
          state: nextState,
          markedBy: app.settings.deviceName || null,
          markedAt: now,
          note: null,
          source: 'local',
          createdAt: now,
          updatedAt: now,
          deletedAt: null,
          syncState: 'pending',
          dirty: true,
        };

    set((s) => ({
      records: { ...s.records, [student.id]: optimistic },
      saving: { ...s.saving, [student.id]: true },
      lastError: null,
    }));

    try {
      // ---- ② 落库 ----
      const saved = await checkinMark({
        studentId: student.id,
        date,
        period,
        state: nextState,
        note: prevRecord?.note ?? null,
      });
      // ---- ③ 用服务端返回的真实记录替换乐观记录 ----
      set((s) => ({
        records: { ...s.records, [student.id]: saved },
        saving: { ...s.saving, [student.id]: false },
      }));
      // 弱提示，不阻塞；同步由 Rust 侧异步入队推送
      app.pushToast({
        kind: 'pending',
        title: `${student.name} ${labelOf(nextState)}`,
        description: '已记录，待同步',
        duration: 1400,
      });
    } catch (err) {
      // ---- ④ 失败回滚 ----
      set((s) => {
        const nextRecords = { ...s.records };
        if (prevRecord) {
          nextRecords[student.id] = prevRecord;
        } else {
          delete nextRecords[student.id];
        }
        const nextSaving = { ...s.saving };
        delete nextSaving[student.id];
        return { records: nextRecords, saving: nextSaving, lastError: '标记失败' };
      });
      app.toastError(err, '考勤标记失败，已恢复原状态');
    }
  },

  batchSetState: async (students, nextState) => {
    const app = useAppStore.getState();
    const { date, period } = get();
    const snapshot = { ...get().records };

    const optimisticMap: Record<string, CheckinRecord> = {};
    const now = Date.now();
    students.forEach((st) => {
      const prev = get().records[st.id] ?? null;
      optimisticMap[st.id] = prev
        ? { ...prev, state: nextState, markedAt: now, updatedAt: now }
        : {
            id: `tmp-${st.id}-${date}-${period}`,
            studentId: st.id,
            checkinDate: date,
            period,
            periodLabel: get().periodLabel ?? null,
            state: nextState,
            markedBy: app.settings.deviceName || null,
            markedAt: now,
            note: null,
            source: 'local' as const,
            createdAt: now,
            updatedAt: now,
            deletedAt: null,
            syncState: 'pending' as const,
            dirty: true,
          };
    });
    set((s) => ({ records: { ...s.records, ...optimisticMap } }));

    try {
      const items = students.map((st) => ({
        studentId: st.id,
        date,
        period,
        state: nextState,
      }));
      const savedList = await checkinBatchMark(items);
      const merged = { ...get().records };
      savedList.forEach((r) => {
        merged[r.studentId] = r;
      });
      set({ records: merged });
      app.pushToast({
        kind: 'success',
        title: `已批量标记 ${savedList.length} 人为「${labelOf(nextState)}」`,
      });
    } catch (err) {
      set({ records: snapshot });
      app.toastError(err, '批量标记失败，已恢复原状态');
    }
  },

  effectiveState: (studentId) => get().records[studentId]?.state ?? 'present',

  buildRows: (students) =>
    students.map((student) => {
      const record = get().records[student.id] ?? null;
      return {
        student,
        record,
        effectiveState: record?.state ?? 'present',
      };
    }),

  summary: (students) => {
    const targets = students.filter((s) => s.status !== 'transferred');
    let present = 0;
    let leave = 0;
    let absent = 0;
    let late = 0;
    let marked = 0;
    targets.forEach((s) => {
      const rec = get().records[s.id];
      if (rec) marked += 1;
      const st = rec?.state ?? 'present';
      if (st === 'present') present += 1;
      else if (st === 'leave') leave += 1;
      else if (st === 'absent') absent += 1;
      else if (st === 'late') late += 1;
    });
    const total = targets.length;
    // 出勤率 = 出勤 / 应到（迟到计为到校，但单独统计）
    const attended = present + late;
    return {
      total,
      present,
      leave,
      absent,
      late,
      marked,
      attendanceRate: total > 0 ? Math.round((attended / total) * 1000) / 10 : 0,
    };
  },

  reset: () =>
    set({ records: {}, saving: {}, summaryCache: null, lastError: null, loading: false }),
}));

function labelOf(state: CheckinState): string {
  switch (state) {
    case 'present':
      return '出勤';
    case 'leave':
      return '请假';
    case 'absent':
      return '缺勤';
    case 'late':
      return '迟到';
    default:
      return state;
  }
}

/** 便捷：从任意组件触发刷新（读取当前名册 + 当前日期时段） */
export async function reloadCheckin(): Promise<void> {
  const students = useStudentStore.getState().checkinStudents();
  await useCheckinStore.getState().load(students);
}
