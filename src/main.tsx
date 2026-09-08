import { createRoot } from 'react-dom/client';
import { App } from './App';
import './index.css';
import { readStoredTheme, applyThemeClass, persistTheme } from './lib/theme';
import { useAppStore } from './store/useAppStore';

// 首帧前应用主题，避免闪白 / 闪黑
const initialTheme = readStoredTheme();
applyThemeClass(initialTheme, false);

// 订阅 store.theme 变化：切换时即时落地 DOM 并持久化
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

// 注意：不使用 React.StrictMode，避免 Tauri 环境下 effect 双调用导致事件重复订阅 / 重复补发。
createRoot(container).render(<App />);
