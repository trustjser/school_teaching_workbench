import type { ReactNode } from 'react';
import { Navigate, Outlet, type RouteObject } from 'react-router-dom';
import { useBootstrap } from '@shared/hooks/useBootstrap';
import { useAppStore } from '@shared/store/useAppStore';
import { AppShell } from './AppShell';
import type { NavItem } from './SideNav';
import { Spinner } from '@shared/components/ui/Spinner';
import { Card } from '@shared/components/ui/Card';
import { Button } from '@shared/components/ui/Button';
import type { AppTarget } from '@shared/app-target';

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

export interface AppRootProps {
  /** 固定 app target */
  appTarget: AppTarget;
  /** 端专属导航项 */
  navItems: NavItem[];
  /** 端专属首次配置向导（phase === 'need-setup' 时渲染） */
  setup: ReactNode;
}

/**
 * 根路由组件：启动引导门禁。
 *
 * 依据 `useAppStore.phase` 决定渲染加载屏 / 引导屏 / 主框架。
 * 主框架内通过 `<Outlet />` 渲染当前子路由页面；未匹配的路径统一回落到首页。
 */
export function AppRoot({ appTarget, navItems, setup }: AppRootProps): JSX.Element {
  useBootstrap();
  const phase = useAppStore((s) => s.phase);

  if (phase === 'idle' || phase === 'loading') return <FullScreenLoading />;
  if (phase === 'need-setup') return <>{setup}</>;
  if (phase === 'error') return <ErrorScreen />;

  return (
    <AppShell appTarget={appTarget} navItems={navItems}>
      <Outlet />
    </AppShell>
  );
}

/** 构造各端共用的路由骨架，避免两个入口重复写引导/回落逻辑 */
export function createAppRoutes(options: {
  appTarget: AppTarget;
  navItems: NavItem[];
  setup: ReactNode;
  routes: RouteObject[];
}): RouteObject[] {
  const root: RouteObject = {
    path: '/',
    element: <AppRoot appTarget={options.appTarget} navItems={options.navItems} setup={options.setup} />,
    children: [
      ...options.routes,
      // 未匹配路径（含手动输入另一端旧路由）统一回落到本端首页，不加载另一端页面。
      { path: '*', element: <Navigate to="/" replace /> },
    ],
  };
  return [root];
}
