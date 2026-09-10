import {
  Children,
  cloneElement,
  useEffect,
  useRef,
  useState,
  type ReactNode,
  type CSSProperties,
  type ReactElement,
} from 'react';

/**
 * 滚动揭示：元素进入视口时播放入场动画。
 * 返回 ref 与 inView，供组件自行决定动画时机。
 */
export function useInView<T extends HTMLElement = HTMLDivElement>(
  options?: IntersectionObserverInit & { once?: boolean },
): { ref: React.RefObject<T>; inView: boolean } {
  const ref = useRef<T>(null);
  const [inView, setInView] = useState(false);

  useEffect(() => {
    const el = ref.current;
    if (!el || typeof IntersectionObserver === 'undefined') {
      setInView(true);
      return;
    }
    const obs = new IntersectionObserver(
      ([entry]) => {
        if (entry.isIntersecting) {
          setInView(true);
          if (options?.once !== false) obs.disconnect();
        } else if (options?.once === false) {
          setInView(false);
        }
      },
      { threshold: 0.12, rootMargin: '0px 0px -8% 0px', ...options },
    );
    obs.observe(el);
    return () => obs.disconnect();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  return { ref, inView };
}

export interface RevealProps {
  children: ReactNode;
  /** 延迟（毫秒），用于错落编排 */
  delay?: number;
  className?: string;
  /** 是否在进入视口后才播放；false 则在挂载即播放（用于首屏区块） */
  requireInView?: boolean;
  style?: CSSProperties;
}

/**
 * 通用揭示容器：进入视口（或挂载）时上浮淡入。
 * 仅渲染 div，便于作为区块/网格项包裹。
 */
export function Reveal({
  children,
  delay = 0,
  className = '',
  requireInView = true,
  style,
}: RevealProps): JSX.Element {
  const { ref, inView } = useInView<HTMLDivElement>();
  const play = requireInView ? inView : true;
  return (
    <div
      ref={ref}
      className={[className, play ? 'animate-rise-in' : 'opacity-0'].join(' ')}
      style={{ animationDelay: `${delay}ms`, ...style }}
    >
      {children}
    </div>
  );
}

export interface StaggerProps {
  children: ReactNode;
  /** 相邻子项延迟步长（毫秒） */
  step?: number;
  /** 起始延迟（毫秒） */
  base?: number;
  /** 容器类名（通常是 grid 布局类） */
  className?: string;
}

/**
 * 错落入场：作为网格/列表容器，给直接子项依次叠加 rise-in 与递增 delay。
 * 用法：<Stagger className="grid grid-cols-3 ...">{items}</Stagger>
 */
export function Stagger({ children, step = 60, base = 0, className = '' }: StaggerProps): JSX.Element {
  let i = 0;
  return (
    <div className={className}>
      {Children.map(children, (child) => {
        if (!child || typeof child !== 'object' || !('props' in (child as object))) {
          return child;
        }
        const node = child as ReactElement<{ className?: string; style?: CSSProperties }>;
        const delay = base + i * step;
        i += 1;
        return cloneElement(node, {
          className: [node.props.className ?? '', 'animate-rise-in'].join(' '),
          style: { ...(node.props.style ?? {}), animationDelay: `${delay}ms` },
        });
      })}
    </div>
  );
}
