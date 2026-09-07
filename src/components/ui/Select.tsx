import type { SelectHTMLAttributes } from 'react';
import { ChevronDown } from 'lucide-react';

export interface SelectOption {
  value: string;
  label: string;
  disabled?: boolean;
}

export interface SelectProps extends Omit<SelectHTMLAttributes<HTMLSelectElement>, 'children'> {
  label?: string;
  options: SelectOption[];
  /** 占位提示（value 为空字符串时展示） */
  placeholder?: string;
  error?: string;
  hint?: string;
}

/** 下拉选择：大字号选项，触控友好 */
export function Select({
  label,
  options,
  placeholder,
  error,
  hint,
  className = '',
  id,
  ...rest
}: SelectProps): JSX.Element {
  const inputId = id ?? `select-${label ?? Math.random().toString(36).slice(2, 8)}`;
  return (
    <div className="w-full">
      {label && (
        <label htmlFor={inputId} className="mb-1.5 block text-base font-semibold text-ink">
          {label}
        </label>
      )}
      <div className="relative">
        <select
          id={inputId}
          className={[
            'w-full min-h-touch appearance-none rounded-lg border bg-white px-4 pr-11',
            'text-base text-ink transition-colors',
            'focus-visible:outline-none focus-visible:ring-4 focus-visible:ring-brand-400',
            error ? 'border-red-600' : 'border-surface-border hover:border-slate-400',
            className,
          ].join(' ')}
          {...rest}
        >
          {placeholder && (
            <option value="" disabled>
              {placeholder}
            </option>
          )}
          {options.map((opt) => (
            <option key={opt.value} value={opt.value} disabled={opt.disabled}>
              {opt.label}
            </option>
          ))}
        </select>
        <ChevronDown
          className="pointer-events-none absolute right-3 top-1/2 h-5 w-5 -translate-y-1/2 text-ink-muted"
          aria-hidden
        />
      </div>
      {error ? (
        <p className="mt-1 text-sm font-medium text-red-700">{error}</p>
      ) : hint ? (
        <p className="mt-1 text-sm text-ink-muted">{hint}</p>
      ) : null}
    </div>
  );
}
