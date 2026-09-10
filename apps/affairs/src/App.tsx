import { RouterProvider } from 'react-router-dom';
import { router } from './router';
import { ToastHost } from '@shared/components/ui/Toast';

/**
 * 教务端根组件。
 * 路由表（含启动引导门禁）由 ./router 维护；ToastHost 挂在 RouterProvider 之外，
 * 作为全局浮层，读取 useAppStore.toasts。
 */
export function App(): JSX.Element {
  return (
    <>
      <RouterProvider router={router} />
      <ToastHost />
    </>
  );
}
