import { defineConfig, type UserConfig } from 'vite';
import react from '@vitejs/plugin-react';
import { fileURLToPath, URL } from 'node:url';

/**
 * 两个 app 共用的 Vite 配置工厂。
 *
 * 教务端与班级端使用同一套 React / Tailwind / 路径别名配置，只有入口目录、
 * dev 端口、HMR 端口、产物目录和注入的 `VITE_APP_TARGET` 不同。
 */
export interface CreateViteConfigOptions {
  /** 固定 app target，注入为 `import.meta.env.VITE_APP_TARGET` */
  appTarget: 'affairs' | 'classroom';
  /** Vite 项目根目录（相对本文件所在仓库根） */
  root: string;
  /** dev server 端口（教务端 1420 / 班级端 1430） */
  port: number;
  /** HMR 端口（教务端 1421 / 班级端 1431） */
  hmrPort: number;
  /** 产物输出目录（相对仓库根） */
  outDir: string;
}

export function createViteConfig(options: CreateViteConfigOptions): UserConfig {
  const host = process.env.TAURI_DEV_HOST;

  return defineConfig({
    root: fileURLToPath(new URL(options.root, import.meta.url)),
    plugins: [react()],
    define: {
      // 端标识在构建期注入：前端入口只读该值，运行期不可更改。
      'import.meta.env.VITE_APP_TARGET': JSON.stringify(options.appTarget),
    },
    resolve: {
      alias: {
        '@shared': fileURLToPath(new URL('./packages/shared/src', import.meta.url)),
        '@affairs': fileURLToPath(new URL('./apps/affairs/src', import.meta.url)),
        '@classroom': fileURLToPath(new URL('./apps/classroom/src', import.meta.url)),
      },
    },
    clearScreen: false,
    server: {
      port: options.port,
      strictPort: true,
      host: host || false,
      hmr: host
        ? {
            protocol: 'ws',
            host,
            port: options.hmrPort,
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
      outDir: fileURLToPath(new URL(options.outDir, import.meta.url)),
      emptyOutDir: true,
      // WebView2(Windows) 与 WebKit(macOS/Linux) 的最低目标
      target: process.env.TAURI_ENV_PLATFORM === 'windows' ? 'chrome105' : 'safari14',
      minify: process.env.TAURI_ENV_DEBUG ? false : 'esbuild',
      sourcemap: Boolean(process.env.TAURI_ENV_DEBUG),
      chunkSizeWarningLimit: 1500,
    },
  });
}
