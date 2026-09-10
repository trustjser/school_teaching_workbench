import type { SelectHTMLAttributes } from 'react';
import { ChevronDown } from 'lucide-react';

export interface SelectOption {
  value: string;
  label: string;
  disabled?: boolean;
  /**
   * 次要说明（例如任务备注）。在选项里以小字截断显示，
   * 完整内容通过 `title` 悬浮提示查看（下拉面板是 overflow 容器，
   * 绝对定位的气泡会被裁剪，因此这里用原生 title 而不是 Tooltip）。
   */
  description?: string;
  /** 右侧补充信息（例如创建时间），不参与截断 */
  meta?: string;
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
            'min-w-0 w-full min-h-touch appearance-none rounded-lg border bg-surface-raised px-4 pr-11',
            'text-base text-ink transition-colors',
            'focus-visible:outline-none focus-visible:ring-4 focus-visible:ring-brand-400',
            error ? 'border-red-600' : 'border-surface-border hover:border-surface-border',
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
        <p className="mt-1 text-sm font-medium text-red-500">{error}</p>
      ) : hint ? (
        <p className="mt-1 text-sm text-ink-muted">{hint}</p>
      ) : null}
    </div>
  );
}
