import type { LucideIcon } from 'lucide-react';
import { Card } from './Card';
import { Badge } from './Badge';
import { CountUp } from '../motion/CountUp';

export type StatTone = 'brand' | 'success' | 'violet' | 'warning' | 'danger' | 'info' | 'neutral';

/** 统计卡配色：图标底 chip / 顶部强调条 / 强调数字色 */
const TONE: Record<StatTone, { chip: string; bar: string; text: string }> = {
  brand: { chip: 'bg-brand-600/12 text-brand-600', bar: 'bg-brand-600', text: 'text-brand-700' },
  success: { chip: 'bg-green-500/12 text-green-600', bar: 'bg-green-500', text: 'text-green-600' },
  violet: { chip: 'bg-violet-500/12 text-violet-600', bar: 'bg-violet-500', text: 'text-violet-600' },
  warning: { chip: 'bg-amber-500/12 text-amber-600', bar: 'bg-amber-500', text: 'text-amber-600' },
  danger: { chip: 'bg-red-500/12 text-red-600', bar: 'bg-red-500', text: 'text-red-600' },
  info: { chip: 'bg-cyan-500/12 text-cyan-600', bar: 'bg-cyan-500', text: 'text-cyan-600' },
  neutral: { chip: 'bg-surface-muted text-ink-muted', bar: 'bg-surface-border', text: 'text-ink' },
};

export interface StatCardProps {
  icon: LucideIcon;
  label: string;
  /** 数值或已格式化文本（文本不滚动） */
  value: string | number;
  tone?: StatTone;
  /** 次级说明 */
  sub?: string;
  /** 角标计数（如未读数） */
  badge?: number;
  badgeTone?: StatTone;
  /** 大数字使用色调色（语义强调） */
  accent?: boolean;
  /** 数字滚动计数（仅数值生效） */
  countUp?: boolean;
  /** 数值小数位（透传给 CountUp） */
  decimals?: number;
  prefix?: string;
  suffix?: string;
  className?: string;
}

/**
 * 统一统计卡：图标 chip + 顶部强调条 + 标签 + 大数字（默认滚动）+ 次级说明 + 角标。
 * 全站复用，保证视觉语言一致。
 */
export function StatCard({
  icon: Icon,
  label,
  value,
  tone = 'brand',
  sub,
  badge,
  badgeTone,
  accent = false,
  countUp = true,
  decimals,
  prefix,
  suffix,
  className = '',
}: StatCardProps): JSX.Element {
  const t = TONE[tone];
  const numeric = typeof value === 'number';
  return (
    <Card className={['card-interactive relative overflow-hidden', className].join(' ')}>
      <span className={['pointer-events-none absolute inset-x-0 top-0 h-1', t.bar].join(' ')} aria-hidden />
      <div className={['inline-flex h-11 w-11 items-center justify-center rounded-xl', t.chip].join(' ')}>
        <Icon className="h-6 w-6" aria-hidden />
      </div>
      <p className="mt-3 text-base text-ink-muted">{label}</p>
      <p className={['mt-1 text-4xl font-bold', accent && numeric ? t.text : 'text-ink'].join(' ')}>
        {numeric ? (
          countUp ? (
            <CountUp value={value} decimals={decimals} prefix={prefix} suffix={suffix} />
          ) : (
            `${prefix ?? ''}${value}${suffix ?? ''}`
          )
        ) : (
          value
        )}
      </p>
      {sub && <p className="mt-1 text-sm text-ink-muted">{sub}</p>}
      {badge ? (
        <Badge tone={badgeTone ?? tone} className="mt-2">
          {badge} 条未读
        </Badge>
      ) : null}
    </Card>
  );
}
