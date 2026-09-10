import type { ReactNode } from 'react';

/** 基础骨架块：复用 .shimmer 微光动画 */
export function Skeleton({ className = '' }: { className?: string }): JSX.Element {
  return <div className={['shimmer rounded-md', className].join(' ')} aria-hidden />;
}

/** 多行文本骨架 */
export function SkeletonText({ lines = 3, className = '' }: { lines?: number; className?: string }): JSX.Element {
  return (
    <div className={['space-y-2', className].join(' ')} aria-hidden>
      {Array.from({ length: lines }).map((_, i) => (
        <Skeleton
          key={i}
          className={['h-3', i === lines - 1 ? 'w-2/3' : 'w-full'].join(' ')}
        />
      ))}
    </div>
  );
}

/** 卡片骨架（与 StatCard 同形，便于列表占位） */
export function SkeletonCard({ className = '' }: { className?: string }): JSX.Element {
  return (
    <div className={['card p-5', className].join(' ')} aria-hidden>
      <Skeleton className="h-11 w-11 rounded-xl" />
      <Skeleton className="mt-3 h-4 w-24" />
      <Skeleton className="mt-2 h-9 w-28" />
      <Skeleton className="mt-2 h-3 w-20" />
    </div>
  );
}

/** 统计卡骨架（SkeletonCard 的别名，语义更清晰） */
export const SkeletonStatCard = SkeletonCard;

/** 表格行骨架 */
export function SkeletonRows({ rows = 5, className = '' }: { rows?: number; className?: string }): JSX.Element {
  return (
    <div className={['space-y-2', className].join(' ')} aria-hidden>
      {Array.from({ length: rows }).map((_, i) => (
        <Skeleton key={i} className="h-12 w-full rounded-lg" />
      ))}
    </div>
  );
}

/** 通用占位提示（无障碍） */
export function SkeletonBlock({ children, className = '' }: { children?: ReactNode; className?: string }): JSX.Element {
  return (
    <div className={['space-y-3', className].join(' ')} role="status" aria-busy="true">
      {children ?? <span className="sr-only">加载中…</span>}
    </div>
  );
}
