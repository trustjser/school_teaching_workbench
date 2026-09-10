import type { ReactNode } from 'react';

export interface CardProps {
  title?: ReactNode;
  /** 标题右侧操作区 */
  actions?: ReactNode;
  /** 副标题 / 描述 */
  description?: ReactNode;
  children: ReactNode;
  /** 内边距（大屏默认较大） */
  padded?: boolean;
  className?: string;
  /** 内容区额外类名（用于网格布局） */
  bodyClassName?: string;
}

/** 卡片容器：标题 + 操作区 + 内容插槽 */
export function Card({
  title,
  actions,
  description,
  children,
  padded = true,
  className = '',
  bodyClassName = '',
}: CardProps): JSX.Element {
  return (
    <section
      data-card
      className={[
        'card transition-[box-shadow,border-color] duration-200',
        className,
      ].join(' ')}
    >
      {(title || actions) && (
        <header className="flex items-start justify-between gap-4 border-b border-surface-border px-5 py-4">
          <div className="min-w-0">
            {title && <h2 className="truncate text-xl font-bold text-ink">{title}</h2>}
            {description && <p className="mt-1 text-sm text-ink-muted">{description}</p>}
          </div>
          {actions && <div className="flex shrink-0 items-center gap-2">{actions}</div>}
        </header>
      )}
      <div className={[padded ? 'p-5' : '', bodyClassName].join(' ')}>{children}</div>
    </section>
  );
}
