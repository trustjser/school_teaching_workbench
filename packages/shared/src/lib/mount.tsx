import type { ReactElement } from 'react';
import { createRoot } from 'react-dom/client';
import { readStoredTheme, applyThemeClass, persistTheme } from '@shared/lib/theme';
import { useAppStore } from '@shared/store/useAppStore';
import { setAppTarget, type AppTarget } from '@shared/app-target';

/**
 * 两个 app 入口共用的挂载流程：
 *  1. 注入固定 app target（shared 层据此推导只读运行模式）；
 *  2. 首帧前应用主题，避免闪白 / 闪黑；
 *  3. 订阅 store.theme 变化，切换时即时落地 DOM 并持久化；
 *  4. 挂载根组件（不使用 StrictMode，避免 Tauri 环境下 effect 双调用导致
 *     事件重复订阅 / 重复补发）。
 */
export function mountApp(target: AppTarget, element: ReactElement): void {
  setAppTarget(target);

  const initialTheme = readStoredTheme();
  applyThemeClass(initialTheme, false);

  useAppStore.subscribe((state, prev) => {
    if (state.theme !== prev.theme) {
      applyThemeClass(state.theme);
      persistTheme(state.theme);
    }
  });

  const container = document.getElementById('root');
  if (!container) {
    throw new Error('Root container #root not found');
  }

  createRoot(container).render(element);
}
