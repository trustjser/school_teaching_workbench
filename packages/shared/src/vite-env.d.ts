/// <reference types="vite/client" />

// 纯 CSS（Tailwind 入口）侧效导入：vite/client 对 `*.css` 的声明在不同小版本间
// 偶有差异，这里显式兜底，确保 `import '@shared/styles/index.css'` 始终通过 tsc。
declare module '*.css';

/**
 * 构建期注入的端标识（见 vite.shared.ts 的 `define`）。
 *
 * 教务端入口产物恒为 `affairs`，班级端恒为 `classroom`；各入口会用它校验自己
 * 的常量，一旦产物与入口 target 不一致立即抛错，避免把两端产物打包串了。
 */
interface ImportMetaEnv {
  readonly VITE_APP_TARGET?: string;
}

interface ImportMeta {
  readonly env: ImportMetaEnv;
}
