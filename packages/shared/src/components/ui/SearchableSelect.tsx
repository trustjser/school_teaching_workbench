import { useEffect, useMemo, useRef, useState, type KeyboardEvent } from 'react';
import { Check, ChevronDown, Search, X } from 'lucide-react';
import type { SelectOption } from './Select';

export interface SearchableSelectProps {
  label?: string;
  options: SelectOption[];
  value: string;
  onChange: (value: string) => void;
  placeholder?: string;
  searchPlaceholder?: string;
  error?: string;
  hint?: string;
  disabled?: boolean;
  className?: string;
}

/** 支持搜索、键盘操作和触摸点击的下拉选择。 */
export function SearchableSelect({
  label,
  options,
  value,
  onChange,
  placeholder = '请选择',
  searchPlaceholder = '输入关键词搜索',
  error,
  hint,
  disabled = false,
  className = '',
}: SearchableSelectProps): JSX.Element {
  const rootRef = useRef<HTMLDivElement>(null);
  const searchRef = useRef<HTMLInputElement>(null);
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState('');
  const [highlighted, setHighlighted] = useState(0);
  const selected = options.find((option) => option.value === value);
  const filtered = useMemo(() => {
    const normalized = query.trim().toLocaleLowerCase();
    if (!normalized) return options;
    return options.filter((option) => option.label.toLocaleLowerCase().includes(normalized));
  }, [options, query]);

  useEffect(() => {
    if (!open) return;
    const close = (event: MouseEvent): void => {
      if (!rootRef.current?.contains(event.target as Node)) setOpen(false);
    };
    document.addEventListener('mousedown', close);
    searchRef.current?.focus();
    return () => document.removeEventListener('mousedown', close);
  }, [open]);

  useEffect(() => {
    if (highlighted >= filtered.length) setHighlighted(Math.max(0, filtered.length - 1));
  }, [filtered.length, highlighted]);

  const choose = (option: SelectOption): void => {
    if (option.disabled) return;
    onChange(option.value);
    setOpen(false);
    setQuery('');
  };

  const onKeyDown = (event: KeyboardEvent<HTMLDivElement>): void => {
    if (disabled) return;
    if (!open && (event.key === 'Enter' || event.key === ' ' || event.key === 'ArrowDown')) {
      event.preventDefault();
      setOpen(true);
      return;
    }
    if (!open) return;
    if (event.key === 'ArrowDown') {
      event.preventDefault();
      setHighlighted((index) => Math.min(index + 1, Math.max(0, filtered.length - 1)));
    } else if (event.key === 'ArrowUp') {
      event.preventDefault();
      setHighlighted((index) => Math.max(index - 1, 0));
    } else if (event.key === 'Enter') {
      event.preventDefault();
      const option = filtered[highlighted];
      if (option) choose(option);
    } else if (event.key === 'Escape') {
      event.preventDefault();
      setOpen(false);
    }
  };

  return (
    <div ref={rootRef} className={['relative min-w-0 w-full', className].join(' ')} onKeyDown={onKeyDown}>
      {label && <span className="mb-1.5 block text-base font-semibold text-ink">{label}</span>}
      <button
        type="button"
        disabled={disabled}
        aria-haspopup="listbox"
        aria-expanded={open}
        onClick={() => setOpen((isOpen) => !isOpen)}
        title={
          selected
            ? [selected.label, selected.description, selected.meta].filter(Boolean).join(' · ')
            : undefined
        }
        className={[
          'flex min-h-touch w-full items-center justify-between gap-3 rounded-lg border bg-surface-raised px-4 text-left text-base text-ink',
          'focus-visible:outline-none focus-visible:ring-4 focus-visible:ring-brand-400 disabled:cursor-not-allowed disabled:opacity-60',
          error ? 'border-red-600' : 'border-surface-border hover:border-brand-400',
        ].join(' ')}
      >
        <span className={['min-w-0 flex-1 truncate', selected ? '' : 'text-ink-muted'].join(' ')}>
          {selected?.label ?? placeholder}
        </span>
        {selected?.meta && (
          <span className="shrink-0 text-sm text-ink-muted">{selected.meta}</span>
        )}
        <ChevronDown className={['h-5 w-5 shrink-0 text-ink-muted transition-transform', open ? 'rotate-180' : ''].join(' ')} aria-hidden />
      </button>
      {open && (
        <div className="absolute left-0 right-0 z-50 mt-2 overflow-hidden rounded-lg border border-surface-border bg-surface-raised shadow-pop">
          <div className="border-b border-surface-border p-2">
            <div className="flex items-center gap-2 rounded-md border border-surface-border px-3 focus-within:ring-2 focus-within:ring-brand-400">
              <Search className="h-4 w-4 shrink-0 text-ink-muted" aria-hidden />
              <input
                ref={searchRef}
                value={query}
                onChange={(event) => {
                  setQuery(event.target.value);
                  setHighlighted(0);
                }}
                placeholder={searchPlaceholder}
                className="min-h-[2.5rem] min-w-0 flex-1 bg-transparent text-base text-ink outline-none placeholder:text-ink-muted"
                aria-label={searchPlaceholder}
              />
              {query && <button type="button" className="p-1 text-ink-muted" onClick={() => setQuery('')} aria-label="清除搜索"><X className="h-4 w-4" /></button>}
            </div>
          </div>
          <div className="max-h-64 overflow-y-auto p-1" role="listbox" aria-label={label ?? '选项'}>
            {filtered.length === 0 ? (
              <p className="px-3 py-4 text-center text-sm text-ink-muted">没有匹配项</p>
            ) : (
              filtered.map((option, index) => (
                <button
                  key={option.value}
                  type="button"
                  role="option"
                  aria-selected={option.value === value}
                  disabled={option.disabled}
                  // 备注在选项里被截断，完整内容走原生 title 提示。
                  title={option.description ? `${option.label} · ${option.description}` : option.label}
                  onMouseEnter={() => setHighlighted(index)}
                  onClick={() => choose(option)}
                  className={[
                    'flex min-h-touch w-full items-center justify-between gap-3 rounded-md px-3 py-2 text-left text-base',
                    index === highlighted ? 'bg-brand-50 text-brand-800' : 'text-ink hover:bg-surface-muted',
                    option.disabled ? 'cursor-not-allowed opacity-50' : '',
                  ].join(' ')}
                >
                  <span className="min-w-0 flex-1">
                    <span className="block truncate">{option.label}</span>
                    {option.description && (
                      <span className="block truncate text-sm text-ink-muted">{option.description}</span>
                    )}
                  </span>
                  <span className="flex shrink-0 items-center gap-2">
                    {option.meta && <span className="text-sm text-ink-muted">{option.meta}</span>}
                    {option.value === value && <Check className="h-5 w-5 text-brand-600" aria-hidden />}
                  </span>
                </button>
              ))
            )}
          </div>
        </div>
      )}
      {error ? <p className="mt-1 text-sm font-medium text-red-500">{error}</p> : hint ? <p className="mt-1 text-sm text-ink-muted">{hint}</p> : null}
    </div>
  );
}
