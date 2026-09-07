import { createHashRouter, Navigate, Outlet, useNavigate } from 'react-router-dom';
import { useBootstrap } from '@/hooks/useBootstrap';
import { useAppStore } from '@/store/useAppStore';
import { useDeviceStore } from '@/store/useDeviceStore';
import { useQueueStore } from '@/store/useQueueStore';
import { useStudentStore } from '@/store/useStudentStore';
import { useBroadcastStore } from '@/store/useBroadcastStore';
import { AppShell } from '@/components/layout/AppShell';
import { Spinner } from '@/components/ui/Spinner';
import { Card } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';

import { ClientHome } from '@/pages/ClientHome';
import { StudentRoster } from '@/pages/StudentRoster';
import { CheckinPage } from '@/pages/CheckinPage';
import { TaskManage } from '@/pages/TaskManage';
import { TaskMatrix } from '@/pages/TaskMatrix';
import { InboxPage } from '@/pages/InboxPage';
import { MasterHome } from '@/pages/MasterHome';
import { DeviceMonitor } from '@/pages/DeviceMonitor';
import { AttendanceBoard } from '@/pages/AttendanceBoard';
import { BroadcastCenter } from '@/pages/BroadcastCenter';
import { Analytics } from '@/pages/Analytics';
import { Settings } from '@/pages/Settings';

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
 * 首次启动引导占位。
 * 完整的「密钥 / 班级 / 模式」配置向导不在本任务范围；此处提供「以某端预览」入口，
 * 仅写入前端 store 的 appMode，便于在无 Rust 配置数据时也能查看 UI。
 *
 * 关键：必须把 `phase` 从 `need-setup` 切到 `ready`——`AppRoot` 在
 * `need-setup` 阶段会**一直**渲染本组件、不渲染 `<Outlet />`，所以仅
 * `navigate('/client')` 是无效的（URL 变了但子路由看不到）。`setPhase('ready')`
 * 后才会渲染 `AppShell + Outlet`，子路由（ClientHome/MasterHome）才真正挂载。
 * 同时补上 `useBootstrap` 在 ready 阶段会跑的预热，让预览能看到数据。
 */
function SetupScreen(): JSX.Element {
  const setSettings = useAppStore((s) => s.setSettings);
  const setPhase = useAppStore((s) => s.setPhase);
  const navigate = useNavigate();

  const enterPreview = (mode: 'client' | 'master'): void => {
    setSettings({ appMode: mode });
    // 与 useBootstrap.ready 分支一致地预热全局 store
    void useDeviceStore.getState().load();
    void useQueueStore.getState().load();
    void useStudentStore.getState().load();
    void useBroadcastStore.getState().loadInbox();
    setPhase('ready');
    navigate(mode === 'master' ? '/master' : '/client');
  };

  return (
    <div className="flex h-screen w-screen items-center justify-center bg-surface-sunken p-6">
      <Card className="max-w-lg">
        <h1 className="text-2xl font-bold text-ink">首次启动引导</h1>
        <p className="mt-2 text-ink-soft">
          尚未完成首次运行配置（共享密钥 / 班级 / 运行模式）。请在设置中完成配置后继续使用。
        </p>
        <div className="mt-4 flex flex-wrap gap-3">
          <Button onClick={() => enterPreview('client')}>以班级端预览</Button>
          <Button variant="secondary" onClick={() => enterPreview('master')}>
            以教务处端预览
          </Button>
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
  if (phase === 'need-setup') return <SetupScreen />;
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
      { path: 'master/broadcast', element: <BroadcastCenter /> },
      { path: 'master/analytics', element: <Analytics /> },
      { path: 'settings', element: <Settings /> },
      { path: '*', element: <RootRedirect /> },
    ],
  },
]);
