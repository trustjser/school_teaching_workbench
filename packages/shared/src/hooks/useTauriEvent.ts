import { useEffect, useRef, useState } from 'react';
import { listenEvent } from '@shared/lib/events';
import type { TauriEventMap, TauriEventName } from '@shared/types/events';

/**
 * 订阅单个 Tauri 事件，返回最新 payload，并在卸载时自动卸载监听。
 */
export function useTauriEvent<K extends TauriEventName>(
  event: K,
): TauriEventMap[K] | null {
  const [payload, setPayload] = useState<TauriEventMap[K] | null>(null);

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | null = null;

    void listenEvent(event, (p) => {
      if (!disposed) setPayload(p);
    }).then((fn) => {
      if (disposed) {
        fn();
        return;
      }
      unlisten = fn;
    });

    return () => {
      disposed = true;
      if (unlisten) unlisten();
    };
  }, [event]);

  return payload;
}

/**
 * 订阅单个 Tauri 事件并执行副作用（不触发重渲染）。
 */
export function useTauriEventHandler<K extends TauriEventName>(
  event: K,
  handler: (payload: TauriEventMap[K]) => void,
): void {
  const handlerRef = useRef(handler);
  handlerRef.current = handler;

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | null = null;

    void listenEvent(event, (p) => {
      handlerRef.current(p);
    }).then((fn) => {
      if (disposed) {
        fn();
        return;
      }
      unlisten = fn;
    });

    return () => {
      disposed = true;
      if (unlisten) unlisten();
    };
  }, [event]);
}
