/// <reference types="vite/client" />

// 纯 CSS（Tailwind 入口）侧效导入：vite/client 对 `*.css` 的声明在不同小版本间
// 偶有差异，这里显式兜底，确保 `import './index.css'` 始终通过 tsc。
declare module '*.css';
