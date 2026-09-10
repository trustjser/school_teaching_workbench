import { createHashRouter, Navigate, Outlet } from 'react-router-dom';
import { useBootstrap } from '@shared/hooks/useBootstrap';
import { useAppStore } from '@shared/store/useAppStore';
import { AppShell } from '@shared/components/layout/AppShell';
import { Spinner } from '@shared/components/ui/Spinner';
import { Card } from '@shared/components/ui/Card';
import { Button } from '@shared/components/ui/Button';

import { ClientHome } from '../pages/ClientHome';
import { StudentRoster } from '../pages/StudentRoster';
import { CheckinPage } from '../pages/CheckinPage';
import { TaskManage } from '../pages/TaskManage';
import { TaskMatrix } from '../pages/TaskMatrix';
import { InboxPage } from '../pages/InboxPage';
import { SetupWizard } from '@shared/components/setup/SetupWizard';
import { MasterHome } from '../pages/MasterHome';
import { DeviceMonitor } from '../pages/DeviceMonitor';
import { AttendanceBoard } from '../pages/AttendanceBoard';
import { BroadcastCenter } from '../pages/BroadcastCenter';
import { Analytics } from '../pages/Analytics';
import { TaskDashboard } from '../pages/TaskDashboard';
import { TaskDetail } from '../pages/TaskDetail';
import { GradeClassManage } from '../pages/GradeClassManage';
import { Settings } from '../pages/Settings';

/** 启动中全屏遮罩 */
function FullScreenLoading(): JSX.Element {
  return (
    <div className="flex h-screen w-screen flex-col items-center justify-center gap-4 bg-surface-sunken">
      <Spinner size={40} label="正在初始化本地数据库…" />
    </div>
  );
}

/** 启动失败提示 */
function ErrorScreen(): JSX.Element {
  const error = useAppStore((s) => s.error);
  return (
    <div className="flex h-screen w-screen items-center justify-center bg-surface-sunken p-6">
      <Card className="max-w-lg">
        <h1 className="text-2xl font-bold text-red-700">启动失败</h1>
        <p className="mt-2 text-ink-soft">{error ?? '未知错误'}</p>
        <p className="mt-4 text-sm text-ink-muted">
          请确认通过 Tauri 应用窗口启动，而非直接用浏览器打开（前端依赖本地命令服务）。
        </p>
        <div className="mt-4">
          <Button onClick={() => window.location.reload()}>重试</Button>
        </div>
      </Card>
    </div>
  );
}

/**
 * 根路由：启动引导门禁。
 * 依据 useAppStore.phase 决定渲染加载屏 / 引导屏 / 主框架。
 * 主框架内通过 <Outlet /> 渲染当前子路由页面。
 */
function AppRoot(): JSX.Element {
  useBootstrap();
  const phase = useAppStore((s) => s.phase);

  if (phase === 'idle' || phase === 'loading') return <FullScreenLoading />;
  if (phase === 'need-setup') return <SetupWizard />;
  if (phase === 'error') return <ErrorScreen />;

  return (
    <AppShell>
      <Outlet />
    </AppShell>
  );
}

/** 默认重定向：按当前运行模式跳到对应首页 */
function RootRedirect(): JSX.Element {
  const mode = useAppStore((s) => s.settings.appMode);
  return <Navigate to={mode === 'master' ? '/master' : '/client'} replace />;
}

/**
 * 路由表（react-router-dom v6，HashRouter 形式，适配 Tauri 单页场景）。
 * 覆盖班级端与教务处端全部导航项，并包含 /settings 设置页。
 */
export const router = createHashRouter([
  {
    path: '/',
    element: <AppRoot />,
    children: [
      { index: true, element: <RootRedirect /> },
      { path: 'client', element: <ClientHome /> },
      { path: 'client/students', element: <StudentRoster /> },
      { path: 'client/checkin', element: <CheckinPage /> },
      { path: 'client/tasks', element: <TaskManage /> },
      { path: 'client/matrix', element: <TaskMatrix /> },
      { path: 'client/inbox', element: <InboxPage /> },
      { path: 'master', element: <MasterHome /> },
      { path: 'master/devices', element: <DeviceMonitor /> },
      { path: 'master/attendance', element: <AttendanceBoard /> },
      { path: 'master/directory', element: <GradeClassManage /> },
      { path: 'master/broadcast', element: <BroadcastCenter /> },
      { path: 'master/tasks', element: <TaskDashboard /> },
      { path: 'master/tasks/:taskId', element: <TaskDetail /> },
      { path: 'master/analytics', element: <Analytics /> },
      { path: 'settings', element: <Settings /> },
      { path: '*', element: <RootRedirect /> },
    ],
  },
]);
