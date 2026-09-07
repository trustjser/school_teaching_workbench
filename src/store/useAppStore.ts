import { create } from 'zustand';
import type { AppMode, ThemeName } from '@/types/enums';
import type { AppRuntimeSettings } from '@/types/models';
import { DEFAULT_SETTINGS, LS_KEYS } from '@/constants/app';
import { settingsSet, settingsSwitchMode, toRuntimeSettings } from '@/lib/db';
import type { AppSetting } from '@/types/models';
import { getErrorMessage } from '@/constants/errorCodes';

/** Toast 类型 */
export type ToastKind = 'success' | 'error' | 'warning' | 'info' | 'pending';

export interface ToastItem {
  id: string;
  kind: ToastKind;
  title: string;
  description?: string;
  duration: number;
}

/** 引导阶段 */
export type BootstrapPhase = 'idle' | 'loading' | 'ready' | 'need-setup' | 'error';

interface AppState {
  /** 引导阶段 */
  phase: BootstrapPhase;
  /** 引导错误信息 */
  error: string | null;
  /** 运行期设置（由 app_settings 折叠） */
  settings: AppRuntimeSettings;
  /** 原始设置行（设置页展示用） */
  rawSettings: AppSetting[];
  /** UI 缩放 */
  uiScale: number;
  /** 主题 */
  theme: ThemeName;
  /** 网络在线（指局域网内是否发现对端，非外网） */
  online: boolean;
  /** 全局 loading 计数 */
  loadingCount: number;
  /** Toast 队列 */
  toasts: ToastItem[];

  setPhase: (phase: BootstrapPhase) => void;
  setError: (error: string | null) => void;
  applySettings: (raw: AppSetting[]) => void;
  setSettings: (patch: Partial<AppRuntimeSettings>) => void;
  switchMode: (mode: AppMode) => Promise<void>;
  setUiScale: (scale: number) => Promise<void>;
  setTheme: (theme: ThemeName) => Promise<void>;
  setOnline: (online: boolean) => void;
  startLoading: () => void;
  endLoading: () => void;

  pushToast: (toast: Omit<ToastItem, 'id' | 'duration'> & { duration?: number }) => string;
  dismissToast: (id: string) => void;
  clearToasts: () => void;
  /** 便捷：错误 toast（自动查错误码表） */
  toastError: (err: unknown, fallbackTitle?: string) => void;
}

const DEFAULT_RUNTIME: AppRuntimeSettings = {
  appMode: 'client',
  firstRunDone: false,
  deviceId: '',
  deviceName: '',
  grade: null,
  className: null,
  schoolName: null,
  apiPort: DEFAULT_SETTINGS.apiPort,
  mdnsServiceType: DEFAULT_SETTINGS.mdnsServiceType,
  uiScale: DEFAULT_SETTINGS.uiScale,
  theme: 'light',
  hmacTsWindowSec: DEFAULT_SETTINGS.hmacTsWindowSec,
  heartbeatInterval: DEFAULT_SETTINGS.heartbeatInterval,
  offlineTtlSec: DEFAULT_SETTINGS.offlineTtlSec,
  queueMaxAttempts: DEFAULT_SETTINGS.queueMaxAttempts,
};

let toastSeq = 0;

function readNumberFromStorage(key: string, fallback: number): number {
  try {
    const raw = window.localStorage.getItem(key);
    if (!raw) return fallback;
    const v = Number.parseFloat(raw);
    return Number.isFinite(v) ? v : fallback;
  } catch {
    return fallback;
  }
}

export const useAppStore = create<AppState>((set, get) => ({
  phase: 'idle',
  error: null,
  settings: { ...DEFAULT_RUNTIME, uiScale: readNumberFromStorage(LS_KEYS.uiScale, DEFAULT_SETTINGS.uiScale) },
  rawSettings: [],
  uiScale: readNumberFromStorage(LS_KEYS.uiScale, DEFAULT_SETTINGS.uiScale),
  theme: 'light',
  online: false,
  loadingCount: 0,
  toasts: [],

  setPhase: (phase) => set({ phase }),
  setError: (error) => set({ error }),

  applySettings: (raw) => {
    const runtime = toRuntimeSettings(raw);
    const scale = runtime.uiScale || DEFAULT_SETTINGS.uiScale;
    set({
      rawSettings: raw,
      settings: runtime,
      uiScale: scale,
      theme: runtime.theme,
    });
    try {
      window.localStorage.setItem(LS_KEYS.uiScale, String(scale));
    } catch {
      /* 忽略存储失败 */
    }
  },

  setSettings: (patch) =>
    set((state) => ({ settings: { ...state.settings, ...patch } })),

  switchMode: async (mode) => {
    try {
      await settingsSwitchMode(mode);
      set((state) => ({ settings: { ...state.settings, appMode: mode } }));
      get().pushToast({
        kind: 'success',
        title: mode === 'master' ? '已切换为教务处端' : '已切换为班级端',
      });
    } catch (err) {
      get().toastError(err, '切换模式失败');
      throw err;
    }
  },

  setUiScale: async (scale) => {
    set({ uiScale: scale, settings: { ...get().settings, uiScale: scale } });
    try {
      window.localStorage.setItem(LS_KEYS.uiScale, String(scale));
    } catch {
      /* 忽略 */
    }
    try {
      await settingsSet('ui_scale', String(scale), 'number');
    } catch {
      /* 本地已生效，落库失败不影响使用 */
    }
  },

  setTheme: async (theme) => {
    set({ theme });
    try {
      await settingsSet('theme', theme, 'string');
    } catch {
      /* 忽略 */
    }
  },

  setOnline: (online) => set({ online }),

  startLoading: () => set((s) => ({ loadingCount: s.loadingCount + 1 })),
  endLoading: () => set((s) => ({ loadingCount: Math.max(0, s.loadingCount - 1) })),

  pushToast: (toast) => {
    toastSeq += 1;
    const id = `toast-${Date.now()}-${toastSeq}`;
    const item: ToastItem = {
      id,
      kind: toast.kind,
      title: toast.title,
      description: toast.description,
      duration: toast.duration ?? 2600,
    };
    set((s) => ({ toasts: [...s.toasts, item] }));
    return id;
  },

  dismissToast: (id) => set((s) => ({ toasts: s.toasts.filter((t) => t.id !== id) })),
  clearToasts: () => set({ toasts: [] }),

  toastError: (err, fallbackTitle) => {
    const e = err as { code?: string; message?: string };
    const code = typeof e?.code === 'string' ? e.code : undefined;
    const message = getErrorMessage(code, e?.message);
    get().pushToast({
      kind: 'error',
      title: fallbackTitle ?? '操作失败',
      description: message,
      duration: 6000,
    });
  },
}));

/** 便捷选择器 */
export const selectAppMode = (s: AppState): AppMode => s.settings.appMode;
export const selectIsSetupDone = (s: AppState): boolean => s.settings.firstRunDone;
export const selectClassName = (s: AppState): string => s.settings.className ?? '';
export const selectDeviceName = (s: AppState): string =>
  s.settings.deviceName || s.settings.className || '本机';
