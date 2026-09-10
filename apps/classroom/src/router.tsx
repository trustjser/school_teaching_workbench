import { createHashRouter, RouteObject } from 'react-router-dom';
import { createAppRoutes } from '@shared/components/layout/AppRoot';
import { SettingsPanel } from '@shared/components/settings/SettingsPanel';
import { SetupWizard } from './setup/SetupWizard';
import { APP_TARGET } from './app-target';
import { CLASSROOM_NAV_ITEMS } from './nav';

import { ClientHome } from './pages/ClientHome';
import { StudentRoster } from './pages/StudentRoster';
import { CheckinPage } from './pages/CheckinPage';
import { TaskManage } from './pages/TaskManage';
import { TaskMatrix } from './pages/TaskMatrix';
import { InboxPage } from './pages/InboxPage';

/**
 * 班级端路由表（HashRouter，适配 Tauri 单页场景）。
 *
 * 只注册班级端页面；教务端路径（/devices、/attendance、/broadcast…）在此不存在，
 * 手动修改 hash 也会被回落到本端首页。
 */
const classroomRoutes: RouteObject[] = [
  { index: true, element: <ClientHome /> },
  { path: 'students', element: <StudentRoster /> },
  { path: 'checkin', element: <CheckinPage /> },
  { path: 'tasks', element: <TaskManage /> },
  { path: 'matrix', element: <TaskMatrix /> },
  { path: 'inbox', element: <InboxPage /> },
  { path: 'settings', element: <SettingsPanel appTarget={APP_TARGET} /> },
];

export const router = createHashRouter(
  createAppRoutes({
    appTarget: APP_TARGET,
    navItems: CLASSROOM_NAV_ITEMS,
    setup: <SetupWizard />,
    routes: classroomRoutes,
  }),
);
