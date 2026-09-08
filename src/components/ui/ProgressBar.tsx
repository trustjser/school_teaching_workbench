export interface ProgressBarProps {
  /** 当前值 */
  value: number;
  /** 最大值 */
  max?: number;
  /** 文案（显示在右侧） */
  label?: string;
  /** 高度类 */
  heightClass?: string;
  /** 颜色主题 */
  tone?: 'brand' | 'success' | 'warning' | 'danger';
  /** 显示百分比数字 */
  showPercent?: boolean;
}

const TONE_BG: Record<'brand' | 'success' | 'warning' | 'danger', string> = {
  brand: 'bg-brand-600',
  success: 'bg-green-600',
  warning: 'bg-amber-500',
  danger: 'bg-red-600',
};

/** 进度条：导入 / 导出 / 补发进度 */
export function ProgressBar({
  value,
  max = 100,
  label,
  heightClass = 'h-4',
  tone = 'brand',
  showPercent = true,
}: ProgressBarProps): JSX.Element {
  const safeMax = max > 0 ? max : 100;
  const ratio = Math.max(0, Math.min(1, value / safeMax));
  const percent = Math.round(ratio * 100);
  return (
    <div className="w-full">
      {(label || showPercent) && (
        <div className="mb-1 flex items-center justify-between text-sm font-semibold text-ink-soft">
          <span>{label}</span>
          {showPercent && <span>{percent}%</span>}
        </div>
      )}
      <div
        role="progressbar"
        aria-valuenow={value}
        aria-valuemin={0}
        aria-valuemax={safeMax}
        aria-label={label ?? '进度'}
        className={['w-full overflow-hidden rounded-full bg-surface-muted', heightClass].join(' ')}
      >
        <div
          className={['h-full rounded-full transition-[width] duration-200', TONE_BG[tone]].join(' ')}
          style={{ width: `${percent}%` }}
        />
      </div>
    </div>
  );
}
