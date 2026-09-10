import { RouterProvider } from 'react-router-dom';
import { router } from './router';
import { ToastHost } from '@shared/components/ui/Toast';

/**
 * 应用根组件。
 * 路由表（含启动引导门禁 / 默认按模式重定向）由 ./router 统一维护。
 * ToastHost 挂在 RouterProvider 之外，作为全局浮层，读取 useAppStore.toasts。
 */
export function App(): JSX.Element {
  return (
    <>
      <RouterProvider router={router} />
      <ToastHost />
    </>
  );
}
