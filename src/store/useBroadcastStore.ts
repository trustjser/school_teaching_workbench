import { create } from 'zustand';
import type { BroadcastReceipt, BroadcastTask, SendReport, TargetSelector } from '@/types/broadcast';
import type { CustomTask } from '@/types/models';
import {
  broadcastAccept,
  broadcastCreate,
  broadcastList,
  broadcastReceipts,
  broadcastSend,
} from '@/lib/db';
import { useAppStore } from './useAppStore';

interface BroadcastState {
  /** 教务处端：已下发任务 */
  outbox: BroadcastTask[];
  /** 班级端：收件箱 */
  inbox: BroadcastTask[];
  loading: boolean;
  /** 当前选中的广播任务 ID */
  selectedId: string | null;
  receipts: Record<string, BroadcastReceipt[]>;
  /** 未读计数（收件箱红点） */
  unreadCount: number;

  loadOutbox: () => Promise<void>;
  loadInbox: () => Promise<void>;
  loadReceipts: (broadcastTaskId: string) => Promise<void>;
  select: (id: string | null) => void;
  create: (task: Partial<BroadcastTask> & { title: string; payload: string }) => Promise<BroadcastTask>;
  send: (id: string, targets: TargetSelector) => Promise<SendReport>;
  accept: (broadcastTaskId: string) => Promise<CustomTask | null>;
  /** 收到新下发任务（事件驱动） */
  pushIncoming: (task: BroadcastTask) => void;
  /** 回执到达（事件驱动） */
  upsertReceipt: (receipt: BroadcastReceipt) => void;
  markAllRead: () => void;
}

export const useBroadcastStore = create<BroadcastState>((set, get) => ({
  outbox: [],
  inbox: [],
  loading: false,
  selectedId: null,
  receipts: {},
  unreadCount: 0,

  loadOutbox: async () => {
    const app = useAppStore.getState();
    set({ loading: true });
    try {
      const list = await broadcastList('out');
      set({ outbox: list });
    } catch (err) {
      app.toastError(err, '加载下发任务失败');
    } finally {
      set({ loading: false });
    }
  },

  loadInbox: async () => {
    const app = useAppStore.getState();
    set({ loading: true });
    try {
      const list = await broadcastList('in');
      set({ inbox: list });
    } catch (err) {
      app.toastError(err, '加载收件箱失败');
    } finally {
      set({ loading: false });
    }
  },

  loadReceipts: async (broadcastTaskId) => {
    const app = useAppStore.getState();
    try {
      const list = await broadcastReceipts(broadcastTaskId);
      set((s) => ({ receipts: { ...s.receipts, [broadcastTaskId]: list } }));
    } catch (err) {
      app.toastError(err, '加载回执失败');
    }
  },

  select: (id) => set({ selectedId: id }),

  create: async (task) => {
    const app = useAppStore.getState();
    try {
      const saved = await broadcastCreate(task);
      set((s) => ({ outbox: [saved, ...s.outbox], selectedId: saved.id }));
      app.pushToast({ kind: 'success', title: '任务已创建（草稿）' });
      return saved;
    } catch (err) {
      app.toastError(err, '创建任务失败');
      throw err;
    }
  },

  send: async (id, targets) => {
    const app = useAppStore.getState();
    try {
      const report = await broadcastSend(id, targets);
      set((s) => ({
        outbox: s.outbox.map((t) =>
          t.id === id
            ? { ...t, status: 'sending', expectCount: report.expectCount, sentAt: Date.now() }
            : t,
        ),
      }));
      app.pushToast({
        kind: 'success',
        title: '已加入下发队列',
        description: `目标 ${report.expectCount} 台设备，网络不可达时会在恢复后自动补发`,
      });
      return report;
    } catch (err) {
      app.toastError(err, '下发失败');
      throw err;
    }
  },

  accept: async (broadcastTaskId) => {
    const app = useAppStore.getState();
    try {
      const task = await broadcastAccept(broadcastTaskId);
      set((s) => ({
        inbox: s.inbox.map((t) => (t.id === broadcastTaskId ? t : t)),
        unreadCount: Math.max(0, s.unreadCount - 1),
      }));
      app.pushToast({
        kind: 'success',
        title: '已生成班级待办',
        description: task.title,
      });
      return task;
    } catch (err) {
      app.toastError(err, '生成待办失败');
      return null;
    }
  },

  pushIncoming: (task) => {
    set((s) => {
      if (s.inbox.some((t) => t.id === task.id)) return s;
      return { inbox: [task, ...s.inbox], unreadCount: s.unreadCount + 1 };
    });
  },

  upsertReceipt: (receipt) => {
    set((s) => {
      const list = s.receipts[receipt.broadcastTaskId] ?? [];
      const idx = list.findIndex((r) => r.id === receipt.id);
      const next = idx >= 0 ? list.map((r, i) => (i === idx ? receipt : r)) : [...list, receipt];
      return { receipts: { ...s.receipts, [receipt.broadcastTaskId]: next } };
    });
  },

  markAllRead: () => set({ unreadCount: 0 }),
}));

/** 汇总回执数量 */
export function summarizeReceipts(list: BroadcastReceipt[]): {
  total: number;
  received: number;
  accepted: number;
  rejected: number;
  done: number;
} {
  const acc = { total: list.length, received: 0, accepted: 0, rejected: 0, done: 0 };
  list.forEach((r) => {
    if (r.status === 'received') acc.received += 1;
    else if (r.status === 'accepted') acc.accepted += 1;
    else if (r.status === 'rejected') acc.rejected += 1;
    else if (r.status === 'done') acc.done += 1;
  });
  return acc;
}
