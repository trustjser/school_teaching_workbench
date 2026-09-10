import { CalendarCheck, ClipboardList, Inbox, LayoutGrid } from 'lucide-react';
import type { NavItem } from '@shared/components/layout/SideNav';

/**
 * 班级端导航项：只包含本端路由。
 *
 * 说明：班级端不提供「班级名册」入口——名册由教务端维护并随目录同步下发，
 * 班级端只负责考勤登记与任务执行。
 */
export const CLASSROOM_NAV_ITEMS: NavItem[] = [
  { to: '/', label: '班级首页', icon: LayoutGrid, badge: 'queue-pending' },
  { to: '/checkin', label: '快捷考勤', icon: CalendarCheck },
  { to: '/tasks', label: '任务中心', icon: ClipboardList },
  { to: '/matrix', label: '任务看板', icon: LayoutGrid },
  { to: '/inbox', label: '通知资料', icon: Inbox, badge: 'inbox-unread' },
];
