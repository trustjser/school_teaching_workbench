import { useCallback, useEffect, useRef } from 'react';

/**
 * 防抖回调。
 * @param fn 被防抖的函数
 * @param delay 延迟毫秒数
 * @returns [debouncedFn, cancel]
 */
export function useDebouncedCallback<T extends (...args: never[]) => void>(
  fn: T,
  delay = 300,
): [(...args: Parameters<T>) => void, () => void] {
  const timerRef = useRef<number | null>(null);
  const fnRef = useRef(fn);
  fnRef.current = fn;

  const cancel = useCallback(() => {
    if (timerRef.current !== null) {
      window.clearTimeout(timerRef.current);
      timerRef.current = null;
    }
  }, []);

  const debounced = useCallback(
    (...args: Parameters<T>) => {
      cancel();
      timerRef.current = window.setTimeout(() => {
        timerRef.current = null;
        fnRef.current(...args);
      }, delay);
    },
    [cancel, delay],
  );

  useEffect(() => cancel, [cancel]);

  return [debounced, cancel];
}
