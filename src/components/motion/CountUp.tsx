import { useEffect, useRef, useState } from 'react';
import { useInView } from './Reveal';

interface CountUpProps {
  /** 目标数值 */
  value: number;
  /** 小数位 */
  decimals?: number;
  /** 动画时长（毫秒） */
  duration?: number;
  /** 前缀（如 ¥、%） */
  prefix?: string;
  /** 后缀（如 %、人） */
  suffix?: string;
  className?: string;
}

const easeOutCubic = (t: number): number => 1 - Math.pow(1 - t, 3);

const prefersReducedMotion = (): boolean =>
  typeof window !== 'undefined' && typeof window.matchMedia === 'function'
    ? window.matchMedia('(prefers-reduced-motion: reduce)').matches
    : false;

/**
 * 数字滚动计数：进入视口后从上一值缓动到目标值。
 * 尊重「减少动效」系统偏好，直接显示终值。
 */
export function CountUp({
  value,
  decimals = 0,
  duration = 900,
  prefix = '',
  suffix = '',
  className = '',
}: CountUpProps): JSX.Element {
  const { ref, inView } = useInView<HTMLSpanElement>();
  const [display, setDisplay] = useState(0);
  const fromRef = useRef(0);
  const rafRef = useRef<number | null>(null);

  useEffect(() => {
    if (!inView) return;
    if (prefersReducedMotion()) {
      fromRef.current = value;
      setDisplay(value);
      return;
    }
    const from = fromRef.current;
    const to = value;
    const start = performance.now();
    const tick = (now: number): void => {
      const t = Math.min(1, (now - start) / duration);
      setDisplay(from + (to - from) * easeOutCubic(t));
      if (t < 1) {
        rafRef.current = requestAnimationFrame(tick);
      } else {
        fromRef.current = to;
      }
    };
    rafRef.current = requestAnimationFrame(tick);
    return () => {
      if (rafRef.current != null) cancelAnimationFrame(rafRef.current);
    };
  }, [value, inView, duration]);

  const formatted = display.toLocaleString('zh-CN', {
    minimumFractionDigits: decimals,
    maximumFractionDigits: decimals,
  });

  return (
    <span ref={ref} className={['tabular-nums', className].join(' ')}>
      {prefix}
      {formatted}
      {suffix}
    </span>
  );
}
