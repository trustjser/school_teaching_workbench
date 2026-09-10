import { Contrast, Moon, Sun } from 'lucide-react';
import type { LucideIcon } from 'lucide-react';
import type { ThemeName } from '@shared/types/enums';
import { useAppStore } from '@shared/store/useAppStore';

interface Option {
  value: ThemeName;
  icon: LucideIcon;
  label: string;
}

const OPTIONS: Option[] = [
  { value: 'light', icon: Sun, label: '浅色' },
  { value: 'dark', icon: Moon, label: '深色' },
  { value: 'high-contrast', icon: Contrast, label: '高对比' },
];

/**
 * 动画主题切换器：三段式分段控件，滑动指示块 + 图标弹入。
 * 写入 useAppStore.theme（经订阅落地到 <html> 并持久化）。
 */
export function ThemeSwitcher({ size = 'md' }: { size?: 'sm' | 'md' }): JSX.Element {
  const theme = useAppStore((s) => s.theme);
  const setTheme = useAppStore((s) => s.setTheme);
  const activeIndex = Math.max(
    0,
    OPTIONS.findIndex((o) => o.value === theme),
  );

  const pad = size === 'sm' ? 'p-0.5' : 'p-1';
  const btn = size === 'sm' ? 'px-2.5 py-1' : 'px-3 py-1.5';

  return (
    <div
      role="radiogroup"
      aria-label="主题切换"
      className={`relative inline-flex items-center rounded-full bg-surface-muted ${pad} shadow-inner`}
    >
      {/* 滑动指示块 */}
      <span
        aria-hidden
        className="absolute inset-y-1 left-1 z-0 h-[calc(100%-0.5rem)] w-[calc((100%-0.5rem)/3)] rounded-full bg-surface-raised shadow-soft transition-transform duration-300 ease-out"
        style={{ transform: `translateX(${activeIndex * 100}%)` }}
      />
      {OPTIONS.map((o, i) => {
        const active = i === activeIndex;
        const Icon = o.icon;
        return (
          <button
            key={o.value}
            type="button"
            role="radio"
            aria-checked={active}
            aria-label={o.label}
            title={o.label}
            onClick={() => void setTheme(o.value)}
            className={[
              'relative z-10 inline-flex items-center gap-1.5 rounded-full font-semibold transition-colors duration-200',
              btn,
              active ? 'text-brand-600' : 'text-ink-muted hover:text-ink-soft',
            ].join(' ')}
          >
            <Icon
              key={active ? 'on' : 'off'}
              className={[
                size === 'sm' ? 'h-4 w-4' : 'h-[1.05rem] w-[1.05rem]',
                active ? 'animate-icon-pop' : '',
              ].join(' ')}
              aria-hidden
            />
            <span className={size === 'sm' ? 'hidden' : 'hidden sm:inline text-sm'}>{o.label}</span>
          </button>
        );
      })}
    </div>
  );
}
