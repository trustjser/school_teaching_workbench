import type { ReactNode } from 'react';

export interface EmptyStateProps {
  title: string;
  description?: string;
  /** 引导操作按钮 */
  action?: ReactNode;
  /** 次要操作 */
  secondaryAction?: ReactNode;
  /** 自定义插画（默认使用 public/empty-illustration.svg） */
  illustration?: ReactNode;
  compact?: boolean;
}

/** 空状态：插画 + 文案 + 引导按钮 */
export function EmptyState({
  title,
  description,
  action,
  secondaryAction,
  illustration,
  compact = false,
}: EmptyStateProps): JSX.Element {
  return (
    <div
      className={[
        'flex flex-col items-center justify-center text-center',
        compact ? 'gap-3 py-8' : 'gap-4 py-16',
      ].join(' ')}
    >
      {illustration ?? (
        <img
          src="/empty-illustration.svg"
          alt=""
          width={compact ? 160 : 240}
          height={compact ? 107 : 160}
          className="opacity-90"
        />
      )}
      <div>
        <p className={['font-bold text-ink', compact ? 'text-lg' : 'text-2xl'].join(' ')}>
          {title}
        </p>
        {description && <p className="mt-2 max-w-lg text-base text-ink-muted">{description}</p>}
      </div>
      {(action || secondaryAction) && (
        <div className="mt-2 flex flex-wrap items-center justify-center gap-3">
          {action}
          {secondaryAction}
        </div>
      )}
    </div>
  );
}
