import type { ButtonHTMLAttributes, ReactNode } from 'react';
import {
  Award,
  Book,
  Check,
  Circle,
  Clock,
  Flag,
  Hourglass,
  Minus,
  Star,
  ThumbsUp,
  X,
} from 'lucide-react';
import type { LucideIcon } from 'lucide-react';

/** 图标名 → lucide 组件映射（状态节点图标下拉使用） */
export const ICON_MAP: Record<string, LucideIcon> = {
  circle: Circle,
  dot: Circle,
  clock: Clock,
  check: Check,
  x: X,
  minus: Minus,
  star: Star,
  flag: Flag,
  book: Book,
  award: Award,
  'thumbs-up': ThumbsUp,
  hourglass: Hourglass,
};

/** 依据图标名解析 lucide 组件，未知名称回退到 Circle */
export function resolveIcon(name: string | null | undefined): LucideIcon {
  if (!name) return Circle;
  return ICON_MAP[name] ?? Circle;
}

export type IconButtonVariant = 'ghost' | 'solid' | 'outline' | 'danger';
export type IconButtonSize = 'md' | 'lg';

export interface IconButtonProps extends Omit<ButtonHTMLAttributes<HTMLButtonElement>, 'children'> {
  /** 图标（ReactNode，通常为 lucide 组件） */
  icon: ReactNode;
  /** 无障碍名称（必填，图标按钮无文字） */
  label: string;
  variant?: IconButtonVariant;
  size?: IconButtonSize;
  /** 显示气泡提示 */
  tooltip?: string;
}

const VARIANT_CLASS: Record<IconButtonVariant, string> = {
  ghost: 'bg-transparent text-ink-soft hover:bg-surface-muted active:bg-surface-muted',
  solid: 'bg-brand-600 text-white hover:bg-brand-700 active:bg-brand-800',
  outline: 'bg-surface-raised text-ink-soft border border-surface-border hover:bg-surface-muted',
  danger: 'bg-red-600 text-white hover:bg-red-700 active:bg-red-800',
};

const SIZE_CLASS: Record<IconButtonSize, string> = {
  md: 'h-touch w-touch min-w-touch',
  lg: 'h-touch-lg w-touch-lg min-w-touch',
};

/**
 * 图标按钮：触控区 ≥44px，高对比焦点环，必填 aria-label。
 */
export function IconButton({
  icon,
  label,
  variant = 'ghost',
  size = 'md',
  tooltip,
  className = '',
  type = 'button',
  ...rest
}: IconButtonProps): JSX.Element {
  return (
    <button
      type={type}
      aria-label={label}
      title={tooltip ?? label}
      className={[
        'inline-flex items-center justify-center rounded-lg transition-colors',
        'focus-visible:outline-none focus-visible:ring-4 focus-visible:ring-brand-400',
        'disabled:opacity-50 disabled:cursor-not-allowed',
        SIZE_CLASS[size],
        VARIANT_CLASS[variant],
        className,
      ].join(' ')}
      {...rest}
    >
      {icon}
    </button>
  );
}
