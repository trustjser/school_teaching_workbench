import { NavLink } from 'react-router-dom';
import {
  ClipboardList,
  Inbox,
  LayoutGrid,
  Radio,
  Settings,
  Users,
  BarChart3,
  Monitor,
  CalendarCheck,
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
  { to: '/client/tasks', label: '任务管理', icon: ClipboardList },
  { to: '/client/matrix', label: '任务矩阵', icon: ClipboardList },
  { to: '/client/inbox', label: '教务指令', icon: Inbox },
];

const MASTER_NAV: NavItem[] = [
  { to: '/master', label: '全校总览', icon: LayoutGrid },
  { to: '/master/devices', label: '节点监控', icon: Monitor },
  { to: '/master/attendance', label: '考勤大屏', icon: CalendarCheck },
  { to: '/master/broadcast', label: '任务下发', icon: Radio },
  { to: '/master/analytics', label: '统计导出', icon: BarChart3 },
];

export interface SideNavProps {
  mode: AppMode;
  onNavigate?: () => void;
}

/** 侧边导航：按 appMode 渲染不同菜单 */
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
      {items.map((item) => (
        <NavLink
          key={item.to}
          to={item.to}
          end={item.to === '/client' || item.to === '/master'}
          onClick={onNavigate}
          className={({ isActive }) =>
            [
              'flex min-h-touch items-center gap-3 rounded-lg px-4 font-semibold transition-colors',
              'focus-visible:outline-none focus-visible:ring-4 focus-visible:ring-brand-400',
              isActive
                ? 'bg-brand-600 text-white'
                : 'text-ink-soft hover:bg-slate-100 hover:text-ink',
            ].join(' ')
          }
        >
          <item.icon className="h-6 w-6 shrink-0" aria-hidden />
          <span className="flex-1 truncate text-base">{item.label}</span>
          {item.badge !== undefined && item.badge > 0 && (
            <span className="rounded-full bg-red-600 px-2 py-0.5 text-xs font-bold text-white">
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
              'flex min-h-touch items-center gap-3 rounded-lg px-4 font-semibold transition-colors',
              'focus-visible:outline-none focus-visible:ring-4 focus-visible:ring-brand-400',
              isActive
                ? 'bg-brand-600 text-white'
                : 'text-ink-soft hover:bg-slate-100 hover:text-ink',
            ].join(' ')
          }
        >
          <Settings className="h-6 w-6 shrink-0" aria-hidden />
          <span className="text-base">设置</span>
        </NavLink>
      </div>
    </nav>
  );
}
