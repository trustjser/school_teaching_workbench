import { useEffect, useRef } from 'react';
import { listenEvents } from '@/lib/events';
import { settingsGetAll } from '@/lib/db';
import { useAppStore } from '@/store/useAppStore';
import { useDeviceStore } from '@/store/useDeviceStore';
import { useQueueStore } from '@/store/useQueueStore';
import { useStudentStore } from '@/store/useStudentStore';
import { useBroadcastStore } from '@/store/useBroadcastStore';
import { useTaskStore } from '@/store/useTaskStore';
import { TAURI_EVENTS } from '@/types/events';
import { reloadCheckin } from '@/store/useCheckinStore';

/**
 * 启动引导：
 *  1. 读取 app_settings 判定 first_run_done；
 *  2. 装配运行模式与身份；
 *  3. 订阅全局 Tauri 事件（设备 / 同步 / 考勤 / 任务 / 广播 / 模式切换 / 导入）；
 *  4. 预热名册、设备、队列数据。
 */
export function useBootstrap(): void {
  const startedRef = useRef(false);

  useEffect(() => {
    if (startedRef.current) return;
    startedRef.current = true;

    let disposed = false;
    let unlisten: (() => void) | null = null;

    const run = async (): Promise<void> => {
      const app = useAppStore.getState();
      app.setPhase('loading');
      try {
        const settings = await settingsGetAll();
        if (disposed) return;
        app.applySettings(settings);

        // 注意：applySettings 内部调用 set() 会生成「新的」state 对象，而上面捕获的
        // `app` 仍是启动前的旧引用，其 settings 还是默认值（firstRunDone=false）。
        // 必须重新 getState() 读取最新 settings，否则 firstRunDone 永远取默认 false，
        // 表现为「每次启动都进入首次运行配置」。
        if (!useAppStore.getState().settings.firstRunDone) {
          app.setPhase('need-setup');
          return;
        }

        app.setPhase('ready');

        // 预热数据（失败不影响进入主页，页面内各自有重试入口）
        void useDeviceStore.getState().load();
        void useQueueStore.getState().load();
        void useStudentStore.getState().load();
        void useBroadcastStore.getState().loadInbox();
      } catch (err) {
        if (disposed) return;
        app.setPhase('error');
        app.setError((err as Error)?.message ?? '启动失败');
        app.toastError(err, '启动失败');
      }
    };

    const subscribe = async (): Promise<void> => {
      const fn = await listenEvents({
        // 设备发现
        [TAURI_EVENTS.DEVICE_FOUND]: (device) => {
          useDeviceStore.getState().upsertDevice(device);
        },
        [TAURI_EVENTS.DEVICE_LOST]: ({ deviceId }) => {
          useDeviceStore.getState().markOffline(deviceId);
        },
        [TAURI_EVENTS.DEVICE_HEARTBEAT]: ({ deviceId, latencyMs, ts }) => {
          useDeviceStore.getState().applyHeartbeat(deviceId, latencyMs, ts);
        },
        [TAURI_EVENTS.DEVICE_OFFLINE]: ({ deviceId }) => {
          useDeviceStore.getState().markOffline(deviceId);
        },
        // 同步
        [TAURI_EVENTS.SYNC_PROGRESS]: ({ pending, sending, lastError }) => {
          useQueueStore.getState().setProgress(pending, sending, lastError ?? null);
        },
        [TAURI_EVENTS.SYNC_ERROR]: ({ queueId, code }) => {
          useAppStore.getState().pushToast({
            kind: 'error',
            title: '同步条目已进入死信',
            description: `队列 ${queueId.slice(0, 8)}… 错误码 ${code}`,
            duration: 6000,
          });
          void useQueueStore.getState().load();
        },
        // 业务变更
        [TAURI_EVENTS.CHECKIN_UPDATED]: () => {
          void reloadCheckin();
        },
        [TAURI_EVENTS.TASK_UPDATED]: ({ taskId }) => {
          const app = useAppStore.getState();
          if (app.settings.appMode === 'master') {
            // 教务端收到班级端的任务定义、节点或记录后，
            // 统计页可能正处于打开状态，需要立即重新查询。
            const taskStore = useTaskStore.getState();
            void taskStore.loadCompletionStats();
            if (taskStore.currentTaskId) void taskStore.loadProgress(taskStore.currentTaskId);
          }
          // 任务矩阵页自行订阅刷新；此处仅保证事件不会被丢弃。
          void taskId;
        },
        [TAURI_EVENTS.BROADCAST_RECEIVED]: (task) => {
          useBroadcastStore.getState().pushIncoming(task);
          void useBroadcastStore.getState().accept(task.id).then(() => {
            void useTaskStore.getState().loadTasks();
          });
          useAppStore.getState().pushToast({
            kind: 'info',
            title: '收到新任务',
            description: `${task.title}${task.sentAt ? ` · 下发于 ${new Date(task.sentAt).toLocaleString()}` : ''}`,
            duration: 5000,
          });
        },
        [TAURI_EVENTS.BROADCAST_RECEIPT]: (receipt) => {
          useBroadcastStore.getState().upsertReceipt(receipt);
        },
        [TAURI_EVENTS.DATA_IMPORTED]: (report) => {
          useAppStore.getState().pushToast({
            kind: report.failedRows > 0 ? 'warning' : 'success',
            title: `导入完成：成功 ${report.successRows} 行`,
            description: report.failedRows > 0 ? `失败 ${report.failedRows} 行` : undefined,
          });
          void useStudentStore.getState().load();
        },
        [TAURI_EVENTS.MODE_CHANGED]: ({ mode }) => {
          const app = useAppStore.getState();
          app.setSettings({ appMode: mode });
          app.pushToast({
            kind: 'info',
            title: mode === 'master' ? '已切换为教务处端' : '已切换为班级端',
          });
          void useDeviceStore.getState().load();
        },
        [TAURI_EVENTS.PACKAGE_PROGRESS]: ({ phase, current, total }) => {
          void phase;
          void current;
          void total;
          // 由 SchPackageDialog 自行订阅；此处仅保证事件通道连通
        },
      });
      if (disposed) {
        fn();
        return;
      }
      unlisten = fn;
    };

    void run();
    void subscribe();

    return () => {
      disposed = true;
      if (unlisten) unlisten();
    };
  }, []);
}
