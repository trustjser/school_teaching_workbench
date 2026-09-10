import { CalendarCheck, ClipboardList, Inbox, LayoutGrid, Users } from 'lucide-react';
import type { NavItem } from '@shared/components/layout/SideNav';

/** 班级端导航项：只包含本端路由，教务端路径不在此注册 */
export const CLASSROOM_NAV_ITEMS: NavItem[] = [
  { to: '/', label: '班级首页', icon: LayoutGrid, badge: 'queue-pending' },
  { to: '/students', label: '班级名册', icon: Users },
  { to: '/checkin', label: '快捷考勤', icon: CalendarCheck },
  { to: '/tasks', label: '任务中心', icon: ClipboardList },
  { to: '/matrix', label: '任务看板', icon: LayoutGrid },
  { to: '/inbox', label: '通知资料', icon: Inbox, badge: 'inbox-unread' },
];
