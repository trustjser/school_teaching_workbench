import { BarChart3, BookMarked, CalendarCheck, ClipboardList, LayoutGrid, Monitor, Radio } from 'lucide-react';
import type { NavItem } from '@shared/components/layout/SideNav';

/** 教务端导航项：只包含本端路由，班级端路径不在此注册 */
export const AFFAIRS_NAV_ITEMS: NavItem[] = [
  { to: '/', label: '全校总览', icon: LayoutGrid },
  { to: '/devices', label: '节点监控', icon: Monitor },
  { to: '/attendance', label: '考勤大屏', icon: CalendarCheck },
  { to: '/directory', label: '年级班级管理', icon: BookMarked },
  { to: '/broadcast', label: '任务下发', icon: Radio },
  { to: '/tasks', label: '任务看板', icon: ClipboardList },
  { to: '/analytics', label: '统计导出', icon: BarChart3 },
];
