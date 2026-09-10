import { Loader2 } from 'lucide-react';

export interface SpinnerProps {
  /** 尺寸（px） */
  size?: number;
  label?: string;
  className?: string;
}

/** 加载指示 */
export function Spinner({ size = 28, label = '加载中', className = '' }: SpinnerProps): JSX.Element {
  return (
    <span role="status" aria-live="polite" className={['inline-flex items-center gap-2', className].join(' ')}>
      <Loader2 className="animate-spin text-brand-600" width={size} height={size} aria-hidden />
      <span className="text-base text-ink-muted">{label}</span>
    </span>
  );
}

/** 全区域加载遮罩 */
export function LoadingOverlay({ label = '加载中…' }: { label?: string }): JSX.Element {
  return (
    <div className="flex h-full min-h-[200px] w-full items-center justify-center bg-surface-raised/70">
      <Spinner label={label} size={32} />
    </div>
  );
}
