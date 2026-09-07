import type { TextareaHTMLAttributes } from 'react';
import { NOTE_MAX_LENGTH } from '@/constants/app';

export interface TextareaProps extends TextareaHTMLAttributes<HTMLTextAreaElement> {
  label?: string;
  error?: string;
  hint?: string;
  /** 显示字数统计（默认上限 500，与 NOTE_MAX_LENGTH 一致） */
  showCount?: boolean;
  maxLength?: number;
}

/** 多行输入：备注录入，带字数统计与上限提示 */
export function Textarea({
  label,
  error,
  hint,
  showCount = false,
  maxLength = NOTE_MAX_LENGTH,
  className = '',
  id,
  value,
  ...rest
}: TextareaProps): JSX.Element {
  const inputId = id ?? `textarea-${label ?? Math.random().toString(36).slice(2, 8)}`;
  const length = typeof value === 'string' ? value.length : 0;
  return (
    <div className="w-full">
      {label && (
        <label htmlFor={inputId} className="mb-1.5 block text-base font-semibold text-ink">
          {label}
        </label>
      )}
      <textarea
        id={inputId}
        maxLength={maxLength}
        value={value}
        className={[
          'w-full rounded-lg border bg-white px-4 py-3 text-base text-ink',
          'transition-colors focus-visible:outline-none focus-visible:ring-4 focus-visible:ring-brand-400',
          error ? 'border-red-600' : 'border-surface-border',
          className,
        ].join(' ')}
        {...rest}
      />
      <div className="mt-1 flex items-center justify-between gap-2">
        <span className="text-sm text-ink-muted">
          {error ? <span className="font-medium text-red-700">{error}</span> : hint}
        </span>
        {showCount && (
          <span
            className={[
              'text-sm',
              length >= maxLength ? 'font-semibold text-red-700' : 'text-ink-muted',
            ].join(' ')}
          >
            {length}/{maxLength}
          </span>
        )}
      </div>
    </div>
  );
}
