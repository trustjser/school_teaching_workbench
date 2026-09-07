export interface TabItem<T extends string> {
  value: T;
  label: string;
  /** 前置 emoji */
  emoji?: string;
  /** 右侧计数徽标 */
  count?: number;
}

export interface TabsProps<T extends string> {
  items: TabItem<T>[];
  value: T;
  onChange: (value: T) => void;
  className?: string;
  size?: 'md' | 'lg';
}

/** 页签：网格视图 / 表格视图 切换。切换保持数据（由上层 store 持有）。 */
export function Tabs<T extends string>({
  items,
  value,
  onChange,
  className = '',
  size = 'lg',
}: TabsProps<T>): JSX.Element {
  return (
    <div
      role="tablist"
      className={[
        'inline-flex items-center gap-1 rounded-lg bg-slate-100 p-1',
        className,
      ].join(' ')}
    >
      {items.map((item) => {
        const active = item.value === value;
        return (
          <button
            key={item.value}
            type="button"
            role="tab"
            aria-selected={active}
            onClick={() => onChange(item.value)}
            className={[
              'inline-flex items-center gap-2 rounded-md font-semibold transition-colors',
              'focus-visible:outline-none focus-visible:ring-4 focus-visible:ring-brand-400',
              size === 'lg' ? 'min-h-touch px-5 text-base' : 'min-h-[2.25rem] px-4 text-sm',
              active
                ? 'bg-white text-brand-800 shadow-card'
                : 'text-ink-muted hover:bg-slate-200 hover:text-ink',
            ].join(' ')}
          >
            {item.emoji && <span aria-hidden>{item.emoji}</span>}
            {item.label}
            {typeof item.count === 'number' && (
              <span
                className={[
                  'rounded-full px-2 py-0.5 text-xs font-bold',
                  active ? 'bg-brand-100 text-brand-800' : 'bg-slate-200 text-ink-soft',
                ].join(' ')}
              >
                {item.count}
              </span>
            )}
          </button>
        );
      })}
    </div>
  );
}
