import { createHashRouter, RouteObject } from 'react-router-dom';
import { createAppRoutes } from '@shared/components/layout/AppRoot';
import { SettingsPanel } from '@shared/components/settings/SettingsPanel';
import { SetupWizard } from './setup/SetupWizard';
import { APP_TARGET } from './app-target';
import { AFFAIRS_NAV_ITEMS } from './nav';

import { MasterHome } from './pages/MasterHome';
import { DeviceMonitor } from './pages/DeviceMonitor';
import { AttendanceBoard } from './pages/AttendanceBoard';
import { GradeClassManage } from './pages/GradeClassManage';
import { BroadcastCenter } from './pages/BroadcastCenter';
import { TaskDashboard } from './pages/TaskDashboard';
import { TaskDetail } from './pages/TaskDetail';
import { Analytics } from './pages/Analytics';

/**
 * 教务端路由表（HashRouter，适配 Tauri 单页场景）。
 *
 * 只注册教务端页面；班级端路径（/students、/checkin、/inbox…）在此不存在，
 * 手动修改 hash 也会被回落到本端首页。
 */
const affairsRoutes: RouteObject[] = [
  { index: true, element: <MasterHome /> },
  { path: 'devices', element: <DeviceMonitor /> },
  { path: 'attendance', element: <AttendanceBoard /> },
  { path: 'directory', element: <GradeClassManage /> },
  { path: 'broadcast', element: <BroadcastCenter /> },
  { path: 'tasks', element: <TaskDashboard /> },
  { path: 'tasks/:taskId', element: <TaskDetail /> },
  { path: 'analytics', element: <Analytics /> },
  { path: 'settings', element: <SettingsPanel appTarget={APP_TARGET} /> },
];

export const router = createHashRouter(
  createAppRoutes({
    appTarget: APP_TARGET,
    navItems: AFFAIRS_NAV_ITEMS,
    setup: <SetupWizard />,
    routes: affairsRoutes,
  }),
);
