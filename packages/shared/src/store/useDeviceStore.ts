import { create } from 'zustand';
import type { Device } from '@shared/types/models';
import { deviceForget, deviceList, deviceRefresh } from '@shared/lib/db';
import { useAppStore } from './useAppStore';
import { OFFLINE_TTL_SEC } from '@shared/constants/app';

interface DeviceState {
  devices: Device[];
  loading: boolean;
  /** 最近一次刷新时间 */
  lastRefreshedAt: number | null;

  load: () => Promise<void>;
  refresh: () => Promise<void>;
  forget: (deviceId: string) => Promise<void>;

  /** mDNS 发现：新增或更新节点 */
  upsertDevice: (device: Device) => void;
  /** 心跳：更新延迟与在线状态 */
  applyHeartbeat: (deviceId: string, latencyMs: number, ts: number) => void;
  /** 丢失/离线标记 */
  markOffline: (deviceId: string) => void;
  /** 移除节点 */
  removeDevice: (deviceId: string) => void;
  /** 依据离线阈值重算在线状态（心跳缺失时兜底） */
  recomputeStatuses: () => void;

  self: () => Device | null;
  onlineCount: () => number;
  masterDevices: () => Device[];
}

export const useDeviceStore = create<DeviceState>((set, get) => ({
  devices: [],
  loading: false,
  lastRefreshedAt: null,

  load: async () => {
    const app = useAppStore.getState();
    set({ loading: true });
    try {
      const list = await deviceList();
      set({ devices: list, lastRefreshedAt: Date.now() });
      app.setOnline(get().onlineCount() > 0);
    } catch (err) {
      app.toastError(err, '加载节点失败');
    } finally {
      set({ loading: false });
    }
  },

  refresh: async () => {
    const app = useAppStore.getState();
    try {
      const list = await deviceRefresh();
      set({ devices: list, lastRefreshedAt: Date.now() });
      app.setOnline(get().onlineCount() > 0);
    } catch (err) {
      app.toastError(err, '刷新节点失败');
    }
  },

  forget: async (deviceId) => {
    const app = useAppStore.getState();
    const snapshot = get().devices;
    set((s) => ({
      devices: s.devices.map((d) =>
        d.deviceId === deviceId ? { ...d, status: 'blocked' as const } : d,
      ),
    }));
    try {
      await deviceForget(deviceId);
      app.pushToast({ kind: 'success', title: '已忽略该节点' });
    } catch (err) {
      set({ devices: snapshot });
      app.toastError(err, '操作失败');
    }
  },

  upsertDevice: (device) => {
    set((s) => {
      const idx = s.devices.findIndex((d) => d.deviceId === device.deviceId);
      if (idx >= 0) {
        const next = [...s.devices];
        next[idx] = { ...next[idx], ...device };
        return { devices: next };
      }
      return { devices: [...s.devices, device] };
    });
    useAppStore.getState().setOnline(get().onlineCount() > 0);
  },

  applyHeartbeat: (deviceId, latencyMs, ts) => {
    set((s) => ({
      devices: s.devices.map((d) =>
        d.deviceId === deviceId
          ? {
              ...d,
              status: 'online' as const,
              lastHeartbeatAt: ts,
              lastSeenAt: ts,
              lastLatencyMs: latencyMs,
              missCount: 0,
            }
          : d,
      ),
    }));
  },

  markOffline: (deviceId) => {
    set((s) => ({
      devices: s.devices.map((d) =>
        d.deviceId === deviceId ? { ...d, status: 'offline' as const } : d,
      ),
    }));
  },

  removeDevice: (deviceId) => {
    set((s) => ({ devices: s.devices.filter((d) => d.deviceId !== deviceId) }));
  },

  recomputeStatuses: () => {
    const now = Date.now();
    set((s) => ({
      devices: s.devices.map((d) => {
        if (d.isSelf) return d;
        if (d.status === 'blocked') return d;
        const last = d.lastSeenAt ?? d.lastHeartbeatAt ?? 0;
        if (last && now - last > OFFLINE_TTL_SEC * 1000) {
          return { ...d, status: 'offline' as const };
        }
        return d;
      }),
    }));
  },

  self: () => get().devices.find((d) => d.isSelf) ?? null,
  onlineCount: () => get().devices.filter((d) => d.status === 'online' && !d.isSelf).length,
  masterDevices: () => get().devices.filter((d) => d.deviceRole === 'master'),
}));
