import { type ReactNode } from 'react';

export interface PageTransitionProps {
  children: ReactNode;
  /** 路由标识（通常用 location.pathname）。变化时重新播放入场动画。 */
  routeKey: string;
  className?: string;
}

/**
 * 路由级入场动画：以 routeKey 作为 React key 强制重挂载，
 * 每切换路由即重放 rise-in（上浮淡入），无需第三方动画库。
 */
export function PageTransition({ children, routeKey, className = '' }: PageTransitionProps): JSX.Element {
  return (
    <div key={routeKey} className={`animate-rise-in ${className}`}>
      {children}
    </div>
  );
}
