import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { isTauriRuntime } from './tauri';
import type { TauriEventMap, TauriEventName } from '@/types/events';

/**
 * Tauri 事件 listen 封装。
 * 统一处理：非 Tauri 环境静默降级、错误日志、返回卸载函数。
 */

export type EventHandler<T> = (payload: T) => void;

/**
 * 订阅单个事件。
 * @returns 卸载函数（Promise）
 */
export async function listenEvent<K extends TauriEventName>(
  event: K,
  handler: EventHandler<TauriEventMap[K]>,
): Promise<UnlistenFn> {
  if (!isTauriRuntime()) {
    return () => {
      /* 浏览器环境无事件总线，空卸载 */
    };
  }
  try {
    return await listen<TauriEventMap[K]>(event, (e) => {
      handler(e.payload);
    });
  } catch (err) {
    if (import.meta.env.VITE_DEBUG === 'true') {
      // eslint-disable-next-line no-console
      console.error(`[listen:${event}]`, err);
    }
    return () => {
      /* 订阅失败返回空卸载，不影响页面渲染 */
    };
  }
}

/**
 * 批量订阅事件，返回一个统一卸载函数。
 * 常用于 useTauriEvent / useBootstrap。
 */
export async function listenEvents(
  subscriptions: Partial<{
    [K in TauriEventName]: EventHandler<TauriEventMap[K]>;
  }>,
): Promise<UnlistenFn> {
  const keys = Object.keys(subscriptions) as TauriEventName[];
  const unlisteners = await Promise.all(
    keys.map((key) => {
      const handler = subscriptions[key] as EventHandler<TauriEventMap[typeof key]> | undefined;
      if (!handler) return Promise.resolve<UnlistenFn>(() => undefined);
      return listenEvent(key, handler);
    }),
  );
  return () => {
    unlisteners.forEach((fn) => {
      try {
        fn();
      } catch {
        /* 卸载失败忽略 */
      }
    });
  };
}

/**
 * 兼容 Tauri v1 命名的别名（部分团队成员习惯 emit 调用）。
 * 注意：按 docs/03-tasks.md §4.4，前端不向 Rust 发事件；
 * 此函数仅用于 WebView 内部跨组件广播（不经过 Rust）。
 */
export function emitLocal(event: string, payload: unknown): void {
  try {
    window.dispatchEvent(new CustomEvent(event, { detail: payload }));
  } catch {
    /* 忽略 */
  }
}

/** 监听 WebView 内部自定义事件（与 emitLocal 配对） */
export function listenLocal<T>(event: string, handler: EventHandler<T>): () => void {
  const wrapper = (e: Event) => {
    handler((e as CustomEvent<T>).detail);
  };
  window.addEventListener(event, wrapper);
  return () => window.removeEventListener(event, wrapper);
}
