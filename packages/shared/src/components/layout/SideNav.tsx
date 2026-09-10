import { NavLink } from 'react-router-dom';
import { GraduationCap, Settings } from 'lucide-react';
import type { LucideIcon } from 'lucide-react';
import { APP_TARGET_LABEL, APP_TARGET_SUBTITLE, type AppTarget } from '@shared/app-target';
import { useBroadcastStore } from '@shared/store/useBroadcastStore';
import { useQueueStore } from '@shared/store/useQueueStore';

/** 徽标来源：由共享层按语义解析，避免各端重复写 store 读取逻辑 */
export type NavBadge = 'inbox-unread' | 'queue-pending' | number;

export interface NavItem {
  to: string;
  label: string;
  icon: LucideIcon;
  /** 徽标计数来源 */
  badge?: NavBadge;
}

export interface SideNavProps {
  /** 固定 app target：只用于品牌头文案，不参与路由选择 */
  appTarget: AppTarget;
  /** 由各端入口显式传入的导航项 */
  navItems: NavItem[];
  onNavigate?: () => void;
}

/** 侧边导航：只渲染传入的导航项，不再根据运行模式分支 */
export function SideNav({ appTarget, navItems, onNavigate }: SideNavProps): JSX.Element {
  const unread = useBroadcastStore((s) => s.unreadCount);
  const pending = useQueueStore((s) => s.pendingCount());

  const resolveBadge = (badge: NavBadge | undefined): number | undefined => {
    if (badge === undefined) return undefined;
    if (typeof badge === 'number') return badge;
    if (badge === 'inbox-unread') return unread;
    return pending;
  };

  return (
    <nav className="flex h-full flex-col gap-1 overflow-y-auto p-3" aria-label="主导航">
      {/* 品牌头 */}
      <div className="mb-3 flex items-center gap-3 px-2 py-3">
        <div className="flex h-11 w-11 shrink-0 items-center justify-center rounded-xl bg-brand-gradient text-white shadow-glow">
          <GraduationCap className="h-6 w-6" aria-hidden />
        </div>
        <div className="min-w-0">
          <p className="truncate text-base font-bold leading-tight text-ink">
            {APP_TARGET_LABEL[appTarget]}
          </p>
          <p className="truncate text-2xs text-ink-muted">{APP_TARGET_SUBTITLE[appTarget]}</p>
        </div>
      </div>

      {navItems.map((item) => {
        const badge = resolveBadge(item.badge);
        return (
          <NavLink
            key={item.to}
            to={item.to}
            end={item.to === '/'}
            onClick={onNavigate}
            className={({ isActive }) =>
              [
                'group relative flex min-h-touch items-center gap-3 rounded-xl px-4 font-semibold',
                'transition-[transform,background-color,color,box-shadow] duration-200',
                'active:scale-[0.97] focus-visible:outline-none focus-visible:ring-4 focus-visible:ring-brand-400',
                isActive
                  ? 'bg-brand-gradient text-white shadow-glow'
                  : 'text-ink-soft hover:bg-surface-muted hover:text-ink',
              ].join(' ')
            }
          >
            <item.icon
              className={['h-6 w-6 shrink-0 transition-transform', 'group-hover:scale-110'].join(' ')}
              aria-hidden
            />
            <span className="flex-1 truncate text-base">{item.label}</span>
            {badge !== undefined && badge > 0 && (
              <span
                className={[
                  'rounded-full px-2 py-0.5 text-xs font-bold',
                  'bg-red-600 text-white',
                  'animate-pulse-ring',
                ].join(' ')}
              >
                {badge > 99 ? '99+' : badge}
              </span>
            )}
          </NavLink>
        );
      })}

      <div className="mt-auto pt-3">
        <NavLink
          to="/settings"
          onClick={onNavigate}
          className={({ isActive }) =>
            [
              'group relative flex min-h-touch items-center gap-3 rounded-xl px-4 font-semibold',
              'transition-[transform,background-color,color,box-shadow] duration-200',
              'active:scale-[0.97] focus-visible:outline-none focus-visible:ring-4 focus-visible:ring-brand-400',
              isActive
                ? 'bg-brand-gradient text-white shadow-glow'
                : 'text-ink-soft hover:bg-surface-muted hover:text-ink',
            ].join(' ')
          }
        >
          <Settings className="h-6 w-6 shrink-0 transition-transform group-hover:scale-110" aria-hidden />
          <span className="text-base">设置</span>
        </NavLink>
      </div>
    </nav>
  );
}
