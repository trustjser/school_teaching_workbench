import { createRoot } from 'react-dom/client';
import { App } from './App';
import './index.css';

const container = document.getElementById('root');
if (!container) {
  throw new Error('Root container #root not found');
}

// 注意：不使用 React.StrictMode，避免 Tauri 环境下 effect 双调用导致事件重复订阅 / 重复补发。
createRoot(container).render(<App />);
