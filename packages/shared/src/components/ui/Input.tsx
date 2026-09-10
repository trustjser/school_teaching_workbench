import type { InputHTMLAttributes, ReactNode } from 'react';

export interface InputProps extends Omit<InputHTMLAttributes<HTMLInputElement>, 'size' | 'prefix'> {
  label?: string;
  error?: string;
  hint?: string;
  /** 前置内容（如图标） */
  prefix?: ReactNode;
  /** 后置内容（如单位） */
  suffix?: ReactNode;
  size?: 'md' | 'lg';
}

/** 输入框：标签、校验态、前后缀，触控区 ≥44px */
export function Input({
  label,
  error,
  hint,
  prefix,
  suffix,
  size = 'lg',
  className = '',
  id,
  ...rest
}: InputProps): JSX.Element {
  const inputId = id ?? `input-${label ?? Math.random().toString(36).slice(2, 8)}`;
  return (
    <div className="min-w-0 w-full">
      {label && (
        <label htmlFor={inputId} className="mb-1.5 block text-base font-semibold text-ink">
          {label}
        </label>
      )}
      <div
        className={[
          'flex items-center gap-2 rounded-lg border bg-surface-raised px-3',
          'transition-colors focus-within:ring-4 focus-within:ring-brand-400',
          error ? 'border-red-600' : 'border-surface-border',
          size === 'lg' ? 'min-h-touch' : 'min-h-[2.25rem]',
        ].join(' ')}
      >
        {prefix && <span className="shrink-0 text-ink-muted">{prefix}</span>}
        <input
          id={inputId}
          className={[
            'min-w-0 flex-1 bg-transparent outline-none placeholder:text-ink-muted',
            size === 'lg' ? 'text-base' : 'text-sm',
            'text-ink',
            className,
          ].join(' ')}
          {...rest}
        />
        {suffix && <span className="shrink-0 text-sm text-ink-muted">{suffix}</span>}
      </div>
      {error ? (
        <p className="mt-1 text-sm font-medium text-red-500">{error}</p>
      ) : hint ? (
        <p className="mt-1 text-sm text-ink-muted">{hint}</p>
      ) : null}
    </div>
  );
}
