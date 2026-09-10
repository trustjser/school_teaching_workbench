import { create } from 'zustand';
import type { BroadcastReceipt, BroadcastTask, SendReport } from '@shared/types/broadcast';
import type { CustomTask } from '@shared/types/models';
import type { Page } from '@shared/types/api';
import { appModeForTarget, getAppTarget } from '@shared/app-target';
import {
  broadcastAccept,
  broadcastCancel,
  broadcastClose,
  broadcastCreate,
  broadcastList,
  broadcastPage,
  broadcastReceipts,
  broadcastSend,
} from '@shared/lib/db';
import { useAppStore } from './useAppStore';
import { useTaskStore } from './useTaskStore';

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
  outboxPage: Page<BroadcastTask> | null;

  loadOutbox: () => Promise<void>;
  loadOutboxPage: (page: number, pageSize: number, keyword?: string | null, status?: string | null) => Promise<void>;
  loadInbox: () => Promise<void>;
  loadReceipts: (broadcastTaskId: string) => Promise<void>;  select: (id: string | null) => void;
  create: (task: Partial<BroadcastTask> & { title: string; payload: string }) => Promise<BroadcastTask>;
  /** `targetDeviceIds` 需为已展开的设备 ID 数组（见 resolveTargetDeviceIds） */
  send: (id: string, targetDeviceIds: string[]) => Promise<SendReport>;
  /** 取消（撤回）尚未送达的下发 */
  cancelOutbox: (id: string) => Promise<void>;
  /** 关闭已下发的任务 */
  closeOutbox: (id: string) => Promise<void>;
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
  outboxPage: null,

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

  loadOutboxPage: async (page, pageSize, keyword, status) => {
    const app = useAppStore.getState();
    set({ loading: true });
    try {
      const result = await broadcastPage('out', page, pageSize, keyword, status);
      set({ outboxPage: result, outbox: result.items });
    } catch (err) {
      app.toastError(err, '加载下发任务失败');
    } finally {
      set({ loading: false });
    }
  },

  /**
   * 取消（撤回）尚未送达的下发：后端会撤回队列里待投递的条目。
   *
   * 已送达时后端返回 `ERR_MODE`，这里原样 toast，由后端负责引导改用「关闭」。
   */
  cancelOutbox: async (id) => {
    const app = useAppStore.getState();
    try {
      const saved = await broadcastCancel(id);
      set((s) => ({
        outbox: s.outbox.map((t) => (t.id === id ? saved : t)),
        outboxPage: s.outboxPage
          ? { ...s.outboxPage, items: s.outboxPage.items.map((t) => (t.id === id ? saved : t)) }
          : s.outboxPage,
      }));
      app.pushToast({ kind: 'success', title: '已取消下发', description: '尚未投递的目标已撤回' });
    } catch (err) {
      app.toastError(err, '取消下发失败');
      throw err;
    }
  },

  /** 关闭已下发的任务（教务端宣布结束）。 */
  closeOutbox: async (id) => {
    const app = useAppStore.getState();
    try {
      const saved = await broadcastClose(id);
      set((s) => ({
        outbox: s.outbox.map((t) => (t.id === id ? saved : t)),
        outboxPage: s.outboxPage
          ? { ...s.outboxPage, items: s.outboxPage.items.map((t) => (t.id === id ? saved : t)) }
          : s.outboxPage,
      }));
      app.pushToast({ kind: 'success', title: '已关闭下发' });
    } catch (err) {
      app.toastError(err, '关闭下发失败');
      throw err;
    }
  },

  loadInbox: async () => {    const app = useAppStore.getState();
    set({ loading: true });
    try {
      const list = await broadcastList('in');
      set({ inbox: list });
      // 接收链路异常或旧版本已入库但未生成待办时，刷新收件箱做一次幂等补偿。
      // 后端按 broadcast_task_id 去重，因此不会重复创建任务。班级端入口
      // 通过 app target 即可判定，无需读取运行期 settings。
      if (appModeForTarget(getAppTarget()) === 'client') {
        await Promise.allSettled(list.map((task) => broadcastAccept(task.id)));
        await useTaskStore.getState().loadTasks();
      }
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

  send: async (id, targetDeviceIds) => {
    const app = useAppStore.getState();
    try {
      const report = await broadcastSend(id, targetDeviceIds);
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
