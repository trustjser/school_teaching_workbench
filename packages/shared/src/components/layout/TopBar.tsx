import { CalendarDays, Search } from 'lucide-react';
import { ModeBadge } from './ModeBadge';
import { SyncIndicator } from './SyncIndicator';
import { Input } from '@/components/ui/Input';
import { ThemeSwitcher } from '@/components/motion/ThemeSwitcher';
import { useAppStore } from '@/store/useAppStore';
import { formatDateCN, toDateKey } from '@/lib/format';

export interface TopBarProps {
  /** 全局搜索值 */
  search?: string;
  onSearchChange?: (value: string) => void;
  /** 是否显示搜索框 */
  showSearch?: boolean;
}

/** 顶栏：模式徽标 + 班级/学校名 + 日期 + 同步指示器 + 搜索 + 主题切换 */
export function TopBar({ search = '', onSearchChange, showSearch = false }: TopBarProps): JSX.Element {
  const settings = useAppStore((s) => s.settings);
  const isMaster = settings.appMode === 'master';
  const orgName = isMaster
    ? settings.schoolName || '未命名学校'
    : `${settings.grade ?? ''}${settings.className ?? '未设置班级'}`.trim() || '未设置班级';

  return (
    <header className="glass sticky top-0 z-50 flex min-h-[4.5rem] flex-wrap items-center gap-4 border-b border-surface-border px-6 py-3">
      <ModeBadge mode={settings.appMode} size="md" />

      <div className="min-w-0">
        <p className="truncate text-xl font-bold text-ink">{orgName}</p>
        <p className="truncate text-sm text-ink-muted">
          {settings.deviceName || '本机'} · {isMaster ? '教务处端' : '班级端'}
        </p>
      </div>

      {showSearch && onSearchChange && (
        <div className="ml-2 w-full max-w-sm">
          <Input
            value={search}
            onChange={(e) => onSearchChange(e.target.value)}
            placeholder="搜索姓名 / 学号"
            prefix={<Search className="h-5 w-5" aria-hidden />}
            aria-label="搜索"
          />
        </div>
      )}

      <div className="ml-auto flex items-center gap-3 sm:gap-4">
        <span className="hidden items-center gap-2 text-base font-semibold text-ink-soft md:inline-flex">
          <CalendarDays className="h-6 w-6" aria-hidden />
          {formatDateCN(toDateKey(Date.now()))}
        </span>
        <SyncIndicator />
        <ThemeSwitcher />
      </div>
    </header>
  );
}
