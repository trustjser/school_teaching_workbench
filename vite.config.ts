import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import { fileURLToPath, URL } from 'node:url';

// Tauri 2.0 官方推荐的 Vite 配置：
//  - dev server 固定 1420 端口（strictPort），避免 Tauri 窗口连错端口
//  - clearScreen=false，保证 Rust 编译错误不被 Vite 清屏覆盖
//  - TAURI_ENV_DEBUG / TAURI_ENV_PLATFORM 由 tauri cli 注入，用于条件编译
const host = process.env.TAURI_DEV_HOST;

export default defineConfig({
  plugins: [react()],
  resolve: {
    alias: {
      '@shared': fileURLToPath(new URL('./packages/shared/src', import.meta.url)),
    },
  },
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: 'ws',
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      // 忽略 Rust 侧文件，避免 Cargo 构建触发前端热更新风暴
      ignored: ['**/src-tauri/**', '**/target/**'],
    },
  },
  // 仅暴露 VITE_ / TAURI_ 前缀的环境变量给前端
  envPrefix: ['VITE_', 'TAURI_'],
  build: {
    // WebView2(Windows) 与 WebKit(macOS/Linux) 的最低目标
    target: process.env.TAURI_ENV_PLATFORM === 'windows' ? 'chrome105' : 'safari14',
    minify: process.env.TAURI_ENV_DEBUG ? false : 'esbuild',
    sourcemap: Boolean(process.env.TAURI_ENV_DEBUG),
    chunkSizeWarningLimit: 1500,
  },
});
