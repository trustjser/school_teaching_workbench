import { useEffect } from 'react';
import { useQueueStore } from '@/store/useQueueStore';
import { useDeviceStore } from '@/store/useDeviceStore';
import { AUTO_SYNC_INTERVAL_MS, DEVICE_REFRESH_INTERVAL_MS } from '@/constants/app';
import { TAURI_EVENTS } from '@/types/events';
import { useTauriEventHandler } from './useTauriEvent';

/**
 * 同步驱动：
 *  - 监听 sync://progress 与 device 事件，队列非空时自动触发补发；
 *  - 定时兜底 flush（默认 15s）；
 *  - 定时刷新设备列表并重算在线状态。
 */
export function useAutoSync(enabled = true): void {
  // 队列变化 → 自动补发（由 Rust 侧 SyncWorker 主导，此处仅兜底唤醒）
  useTauriEventHandler(TAURI_EVENTS.SYNC_PROGRESS, ({ pending }) => {
    if (!enabled) return;
    if (pending > 0) {
      void useQueueStore.getState().flush();
    }
  });

  useEffect(() => {
    if (!enabled) return;

    const flushTimer = window.setInterval(() => {
      const queue = useQueueStore.getState();
      if (queue.pendingCount() > 0 && !queue.flushing) {
        void queue.flush();
      }
    }, AUTO_SYNC_INTERVAL_MS);

    const deviceTimer = window.setInterval(() => {
      useDeviceStore.getState().recomputeStatuses();
      void useDeviceStore.getState().load();
    }, DEVICE_REFRESH_INTERVAL_MS);

    // 立即执行一次，避免刚进入应用时状态陈旧
    useDeviceStore.getState().recomputeStatuses();

    return () => {
      window.clearInterval(flushTimer);
      window.clearInterval(deviceTimer);
    };
  }, [enabled]);
}
