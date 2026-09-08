import type { ButtonHTMLAttributes, ReactNode } from 'react';
import { Loader2 } from 'lucide-react';
import { cn } from '../../lib/cn';

export type ButtonVariant = 'primary' | 'secondary' | 'ghost' | 'danger' | 'success' | 'warning';
export type ButtonSize = 'md' | 'lg' | 'xl';

export interface ButtonProps extends Omit<ButtonHTMLAttributes<HTMLButtonElement>, 'children'> {
  children: ReactNode;
  variant?: ButtonVariant;
  size?: ButtonSize;
  loading?: boolean;
  /** 左侧图标 */
  icon?: ReactNode;
  /** 块级（占满父容器宽度） */
  block?: boolean;
}

const VARIANT_CLASS: Record<ButtonVariant, string> = {
  primary: 'bg-brand-600 text-white hover:bg-brand-700 active:bg-brand-800 border border-brand-700',
  secondary: 'bg-surface-raised text-ink border border-surface-border hover:bg-surface-muted active:bg-surface-muted',
  ghost: 'bg-transparent text-ink-soft hover:bg-surface-muted active:bg-surface-muted border border-transparent',
  danger: 'bg-red-600 text-white hover:bg-red-700 active:bg-red-800 border border-red-700',
  success: 'bg-green-600 text-white hover:bg-green-700 active:bg-green-800 border border-green-700',
  warning: 'bg-amber-500 text-white hover:bg-amber-600 active:bg-amber-700 border border-amber-600',
};

const SIZE_CLASS: Record<ButtonSize, string> = {
  md: 'min-h-touch px-4 text-sm',
  lg: 'min-h-touch px-5 text-base',
  xl: 'min-h-touch-lg px-7 text-lg',
};

/**
 * 大屏按钮：最小高度 44px（xl 档 52px），高对比，焦点环明显。
 */
export function Button({
  children,
  variant = 'primary',
  size = 'lg',
  loading = false,
  icon,
  block = false,
  className = '',
  disabled,
  type = 'button',
  ...rest
}: ButtonProps): JSX.Element {
  return (
    <button
      type={type}
      disabled={disabled || loading}
      className={cn(
        'inline-flex items-center justify-center gap-2 whitespace-nowrap rounded-lg font-semibold select-none',
        'transition-[transform,background-color,box-shadow,color]',
        'hover:shadow-soft active:scale-[0.97]',
        'focus-visible:outline-none focus-visible:ring-4 focus-visible:ring-brand-400',
        'disabled:opacity-50 disabled:cursor-not-allowed disabled:active:scale-100 disabled:hover:shadow-none',
        SIZE_CLASS[size],
        VARIANT_CLASS[variant],
        block ? 'w-full' : '',
        className,
      )}
      {...rest}
    >
      {loading ? <Loader2 className="h-5 w-5 animate-spin" aria-hidden /> : icon}
      {children}
    </button>
  );
}
