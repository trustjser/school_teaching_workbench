import { NavLink } from 'react-router-dom';
import { GraduationCap, Settings } from 'lucide-react';
import {
  ClipboardList,
  Inbox,
  LayoutGrid,
  Radio,
  Users,
  BarChart3,
  Monitor,
  CalendarCheck,
  BookMarked,
} from 'lucide-react';
import type { LucideIcon } from 'lucide-react';
import type { AppMode } from '@/types/enums';
import { useBroadcastStore } from '@/store/useBroadcastStore';
import { useQueueStore } from '@/store/useQueueStore';

export interface NavItem {
  to: string;
  label: string;
  icon: LucideIcon;
  /** 徽标计数 */
  badge?: number;
}

const CLIENT_NAV: NavItem[] = [
  { to: '/client', label: '班级首页', icon: LayoutGrid },
  { to: '/client/checkin', label: '快捷考勤', icon: CalendarCheck },
  { to: '/client/students', label: '学生名册', icon: Users },
  { to: '/client/tasks', label: '任务中心', icon: ClipboardList },
  { to: '/client/inbox', label: '通知资料', icon: Inbox },
];

const MASTER_NAV: NavItem[] = [
  { to: '/master', label: '全校总览', icon: LayoutGrid },
  { to: '/master/devices', label: '节点监控', icon: Monitor },
  { to: '/master/attendance', label: '考勤大屏', icon: CalendarCheck },
  { to: '/master/directory', label: '年级班级管理', icon: BookMarked },
  { to: '/master/broadcast', label: '任务下发', icon: Radio },
  { to: '/master/analytics', label: '统计导出', icon: BarChart3 },
];

export interface SideNavProps {
  mode: AppMode;
  onNavigate?: () => void;
}

/** 侧边导航：按 appMode 渲染不同菜单，含品牌头与动画激活态 */
export function SideNav({ mode, onNavigate }: SideNavProps): JSX.Element {
  const unread = useBroadcastStore((s) => s.unreadCount);
  const pending = useQueueStore((s) => s.pendingCount());

  const items: NavItem[] = (mode === 'master' ? MASTER_NAV : CLIENT_NAV).map((item) => {
    if (item.to.endsWith('/inbox')) return { ...item, badge: unread };
    if (item.to === '/client') return { ...item, badge: pending };
    return item;
  });

  return (
    <nav className="flex h-full flex-col gap-1 overflow-y-auto p-3" aria-label="主导航">
      {/* 品牌头 */}
      <div className="mb-3 flex items-center gap-3 px-2 py-3">
        <div className="flex h-11 w-11 shrink-0 items-center justify-center rounded-xl bg-brand-gradient text-white shadow-glow">
          <GraduationCap className="h-6 w-6" aria-hidden />
        </div>
        <div className="min-w-0">
          <p className="truncate text-base font-bold leading-tight text-ink">教务工作台</p>
          <p className="truncate text-2xs text-ink-muted">
            {mode === 'master' ? '教务处协同端' : '班级协同端'}
          </p>
        </div>
      </div>

      {items.map((item) => (
        <NavLink
          key={item.to}
          to={item.to}
          end={item.to === '/client' || item.to === '/master'}
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
            className={[
              'h-6 w-6 shrink-0 transition-transform',

              'group-hover:scale-110',
            ].join(' ')}
            aria-hidden
          />
          <span className="flex-1 truncate text-base">{item.label}</span>
          {item.badge !== undefined && item.badge > 0 && (
            <span
              className={[
                'rounded-full px-2 py-0.5 text-xs font-bold',
                'bg-red-600 text-white',
                'animate-pulse-ring',
              ].join(' ')}
            >
              {item.badge > 99 ? '99+' : item.badge}
            </span>
          )}
        </NavLink>
      ))}

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
