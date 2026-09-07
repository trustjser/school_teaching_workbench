import { create } from 'zustand';
import type { PendingQueueItem, SyncLogEntry } from '@/types/models';
import type { FlushReport } from '@/types/api';
import { syncFlush, syncLogList, syncQueueList, syncRetry } from '@/lib/db';
import { useAppStore } from './useAppStore';

interface QueueState {
  items: PendingQueueItem[];
  logs: SyncLogEntry[];
  loading: boolean;
  /** 正在补发 */
  flushing: boolean;
  /** 补发动画触发时间戳（用于 SyncIndicator 播放动画） */
  lastFlushAt: number | null;
  lastError: string | null;
  /** 最近一次补发结果 */
  lastReport: FlushReport | null;

  load: () => Promise<void>;
  loadLogs: (limit?: number) => Promise<void>;
  flush: () => Promise<FlushReport | null>;
  retry: (id: string) => Promise<void>;
  /** 事件驱动：更新待发数量 */
  setProgress: (pending: number, sending: number, lastError?: string | null) => void;
  pendingCount: () => number;
  deadCount: () => number;
}

export const useQueueStore = create<QueueState>((set, get) => ({
  items: [],
  logs: [],
  loading: false,
  flushing: false,
  lastFlushAt: null,
  lastError: null,
  lastReport: null,

  load: async () => {
    const app = useAppStore.getState();
    set({ loading: true });
    try {
      const items = await syncQueueList();
      set({ items });
    } catch (err) {
      app.toastError(err, '加载待发队列失败');
    } finally {
      set({ loading: false });
    }
  },

  loadLogs: async (limit = 100) => {
    const app = useAppStore.getState();
    try {
      const logs = await syncLogList(limit);
      set({ logs });
    } catch (err) {
      app.toastError(err, '加载同步日志失败');
    }
  },

  flush: async () => {
    const app = useAppStore.getState();
    if (get().flushing) return null;
    set({ flushing: true, lastFlushAt: Date.now() });
    try {
      const report = await syncFlush();
      set({ lastReport: report });
      const items = await syncQueueList();
      set({ items });
      if (report.sent > 0) {
        app.pushToast({
          kind: 'success',
          title: `已补发 ${report.sent} 条`,
          description: report.failed > 0 ? `${report.failed} 条稍后重试` : '数据已同步到对端',
        });
      } else if (report.failed > 0) {
        app.pushToast({
          kind: 'warning',
          title: '暂无可发送条目',
          description: '对端可能离线，恢复网络后会自动补发',
        });
      } else {
        app.pushToast({ kind: 'info', title: '没有待发送数据' });
      }
      return report;
    } catch (err) {
      app.toastError(err, '补发失败');
      return null;
    } finally {
      set({ flushing: false });
    }
  },

  retry: async (id) => {
    const app = useAppStore.getState();
    try {
      await syncRetry(id);
      app.pushToast({ kind: 'success', title: '已重新加入待发队列' });
      await get().load();
    } catch (err) {
      app.toastError(err, '重试失败');
    }
  },

  setProgress: (pending, sending, lastError) => {
    set({ lastError: lastError ?? null });
    // 数量变化时静默刷新列表（避免频繁拉取，仅在归零或有错误时刷新）
    const currentPending = get().items.filter(
      (i) => i.status === 'pending' || i.status === 'sending',
    ).length;
    if (currentPending !== pending + sending) {
      void get().load();
    }
  },

  pendingCount: () =>
    get().items.filter((i) => i.status === 'pending' || i.status === 'sending').length,
  deadCount: () => get().items.filter((i) => i.status === 'dead').length,
}));
