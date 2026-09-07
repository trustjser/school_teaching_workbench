import type { ReactNode } from 'react';

export type BadgeTone =
  | 'neutral'
  | 'brand'
  | 'success'
  | 'warning'
  | 'danger'
  | 'info'
  | 'violet'
  | 'orange';

export interface BadgeProps {
  children: ReactNode;
  tone?: BadgeTone;
  /** 前置 emoji（色盲友好：颜色 + 图标/文字 双编码） */
  emoji?: string;
  size?: 'sm' | 'md' | 'lg';
  className?: string;
}

const TONE_CLASS: Record<BadgeTone, string> = {
  neutral: 'bg-slate-100 text-slate-800 border-slate-400',
  brand: 'bg-brand-50 text-brand-800 border-brand-500',
  success: 'bg-green-50 text-green-800 border-green-600',
  warning: 'bg-amber-50 text-amber-800 border-amber-600',
  danger: 'bg-red-50 text-red-800 border-red-600',
  info: 'bg-cyan-50 text-cyan-800 border-cyan-600',
  violet: 'bg-violet-50 text-violet-800 border-violet-600',
  orange: 'bg-orange-50 text-orange-800 border-orange-600',
};

const SIZE_CLASS: Record<'sm' | 'md' | 'lg', string> = {
  sm: 'text-xs px-2 py-0.5',
  md: 'text-sm px-2.5 py-1',
  lg: 'text-base px-3 py-1.5',
};

/** 状态徽标：高对比描边 + emoji 双编码 */
export function Badge({
  children,
  tone = 'neutral',
  emoji,
  size = 'md',
  className = '',
}: BadgeProps): JSX.Element {
  return (
    <span
      className={[
        'inline-flex items-center gap-1.5 rounded-full border font-semibold whitespace-nowrap',
        TONE_CLASS[tone],
        SIZE_CLASS[size],
        className,
      ].join(' ')}
    >
      {emoji && (
        <span aria-hidden className="text-sm leading-none">
          {emoji}
        </span>
      )}
      {children}
    </span>
  );
}
