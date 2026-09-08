export interface ToggleProps {
  checked: boolean;
  onChange: (checked: boolean) => void;
  label?: string;
  description?: string;
  disabled?: boolean;
  className?: string;
}

/** 开关：启用评分 / 备注等。触控区 ≥44px，状态同时用颜色与文字表达。 */
export function Toggle({
  checked,
  onChange,
  label,
  description,
  disabled = false,
  className = '',
}: ToggleProps): JSX.Element {
  return (
    <label
      className={[
        'flex items-center justify-between gap-4 rounded-lg px-1 py-2',
        disabled ? 'opacity-50' : 'cursor-pointer',
        className,
      ].join(' ')}
    >
      <span className="min-w-0">
        {label && <span className="block text-base font-semibold text-ink">{label}</span>}
        {description && <span className="mt-0.5 block text-sm text-ink-muted">{description}</span>}
      </span>
      <button
        type="button"
        role="switch"
        aria-checked={checked}
        aria-label={label ?? '开关'}
        disabled={disabled}
        onClick={() => onChange(!checked)}
        className={[
          'relative inline-flex h-touch w-[5rem] shrink-0 items-center rounded-full',
          'border-2 transition-colors',
          'focus-visible:outline-none focus-visible:ring-4 focus-visible:ring-brand-400',
          checked ? 'border-green-700 bg-green-600' : 'border-surface-border bg-surface-muted',
        ].join(' ')}
      >
        <span
          className={[
            'absolute h-9 w-9 rounded-full bg-surface-raised shadow transition-transform',
            checked ? 'translate-x-[2.1rem]' : 'translate-x-0.5',
          ].join(' ')}
        />
        <span
          className={[
            'pointer-events-none absolute text-xs font-bold uppercase',
            checked ? 'left-3 text-white' : 'right-3 text-ink-soft',
          ].join(' ')}
        >
          {checked ? '开' : '关'}
        </span>
      </button>
    </label>
  );
}
