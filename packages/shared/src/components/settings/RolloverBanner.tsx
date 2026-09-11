import { useEffect, useState } from 'react';
import { cn } from '@shared/lib/cn';

const KEY = 'rollover-banner-until';

export interface RolloverBannerData {
  schoolYearName: string;
  className: string;
  gradeName: string | null;
}

/** 换届自动切换横幅：显示至手动关闭或 24h；长期信息由状态栏承载（设计 §5.3）。 */
export function showRolloverBanner(data: RolloverBannerData): void {
  try {
    localStorage.setItem(KEY, JSON.stringify({ ...data, until: Date.now() + 24 * 3600_000 }));
    window.dispatchEvent(new CustomEvent('rollover-banner'));
  } catch { /* localStorage 不可用时静默 */ }
}

export function RolloverBanner(): JSX.Element | null {
  const [data, setData] = useState<RolloverBannerData | null>(null);
  useEffect(() => {
    const read = (): void => {
      try {
        const raw = localStorage.getItem(KEY);
        if (!raw) return setData(null);
        const parsed = JSON.parse(raw) as RolloverBannerData & { until: number };
        setData(parsed.until > Date.now() ? parsed : null);
        if (parsed.until <= Date.now()) localStorage.removeItem(KEY);
      } catch { setData(null); }
    };
    read();
    window.addEventListener('rollover-banner', read);
    return () => window.removeEventListener('rollover-banner', read);
  }, []);
  if (!data) return null;
  return (
    <div className={cn('flex items-center justify-between gap-2 rounded-lg border border-brand-400 bg-brand-50 px-3 py-2')}>
      <p className="text-sm text-ink">
        已切换到 {data.schoolYearName} · {data.gradeName ?? ''}{data.className}
      </p>
      <button
        className="rounded px-2 py-1 text-sm text-ink-soft hover:bg-surface-muted"
        onClick={() => { localStorage.removeItem(KEY); setData(null); }}
      >知道了</button>
    </div>
  );
}
