# 项目长期记忆：lan-workbench（局域网教务协同工作台）

## 技术栈
React 18 + TypeScript 5 + Vite 5 + Tauri v2（桌面壳）+ Tailwind v3.4 + Zustand 4。
路由 react-router-dom v6（HashRouter）。图标 lucide-react。SQL 走 @tauri-apps/plugin-sql。

## 主题与样式系统（2026-09-08 重建）
- 三套主题：light（默认）/ dark / high-contrast，由 `<html>` 上的类 `.dark` / `.high-contrast` 驱动；light 无额外类。
- 主题令牌集中在 `src/theme.css`：用 CSS 变量定义 `--surface*` / `--ink*` / `--brand*` / `--glass*`，三套主题各自覆盖。
- `tailwind.config.js` 中 `surface.*` 与 `ink.*` 颜色映射到 `rgb(var(--x) / <alpha-value>)`，改主题即全局换肤；`brand/state/node` 仍为固定高饱和色（跨主题通用）。
- 应用落地：`src/lib/theme.ts` 的 `applyThemeClass` + `main.tsx` 订阅 `useAppStore.theme`（store 已有 `setTheme`，写入 DB 并触发本订阅落地 DOM + localStorage 持久化，首帧前读取 localStorage 防闪）。
- **约定：写组件一律用语义令牌（bg-surface-raised / bg-surface-sunken / bg-surface-muted / border-surface-border / text-ink / text-ink-soft / text-ink-muted），禁止写死 `bg-white`、`slate-*`、`gray-*`、`zinc-*`、`neutral-*`。** 这样深色/高对比自动正确。
- 切换瞬态过渡：切主题时给 `<html>` 挂 `.theme-anim`（theme.css 内全局过渡 320ms 后移除）。
- 动画：关键帧/动画集中在 tailwind.config.js（rise-in / fade-in / scale-in / slide-in-up / shimmer / float / gradient-pan / icon-pop / pulse-ring / toast-in / pop-in / slide-in-right）。路由入场用 `src/components/motion/PageTransition`（按 pathname 重挂载播 rise-in）；错落用 `Stagger`/`Reveal`（同目录）。主题切换器 `ThemeSwitcher`（日/月/对比 滑动分段控件）。
- 玻璃拟态：`.glass`（顶栏/侧栏）；悬浮抬升卡片用 `.card-interactive`（Card 已带 `data-card` 供高对比加描边）。

## 环境验证坑（重要，避免重复踩）
- `npx tsc` 会拉到错误版本的 tsc，误报 `Cannot find module '@tauri-apps/api/core'`。**必须用 `./node_modules/.bin/tsc --noEmit`** 才是真结果。
- 本机 WorkBuddy 文件代理会对读取 `node_modules/@tauri-apps/api/core.js` 触发「敏感内容审批超时」，导致 `vite build` 在打包末期被拦截（与代码无关，真实 Tauri 环境正常）。
- 真实构建用 `npm run build`/`tauri build`。
- **SQLite 迁移坑（已踩）**：本项目捆绑的 SQLite 版本 < 3.35，**不支持 `ALTER TABLE ... ADD COLUMN IF NOT EXISTS`**（ALTER 无 IF NOT EXISTS 语法，仅 CREATE TABLE/INDEX 有）。`run_migrations`（src-tauri/src/db/mod.rs）对 `ADD COLUMN` 语句做幂等保护：先 `PRAGMA table_info` 查列是否存在，存在则跳过、否则执行普通 `ALTER TABLE ... ADD COLUMN`。**今后写迁移，给 students 等旧表加列一律写普通 `ALTER TABLE x ADD COLUMN y TYPE`（不要写 IF NOT EXISTS），幂等交给 Rust 侧**；`CREATE TABLE/INDEX IF NOT EXISTS` 仍可用。
- 迁移只经 `run_migrations`（init_db 调用），`tauri-plugin-sql` 在 app.rs 中**未注册** migrations，勿混淆。

## 布局 / 响应式约定
- 全屏遮罩或居中卡片**避免用 `w-screen`**：一旦内容超高出现垂直滚动条，`w-screen`（100vw）会包含滚动条宽度，导致水平滚动条。改用 `w-full` 或 `min-w-full`，并给卡片加 `min-w-0`。
- 表单控件（Input/Select/Textarea 包装器与本机元素）统一加 `min-w-0`，防止 `cols`/`size`/选项文本的内禀最小宽度撑开父容器。
- 两列表单一律写成 `grid-cols-1 sm:grid-cols-2`（移动端单列，≥640px 双列），避免窄窗下双列溢出。
- **固定定位组件必须用 Portal 挂到 body**：`PageTransition` 等路由入场动画使用 `transform`（且 `animation-fill-mode: both`），会导致其内部 fixed 元素的包含块被篡改，从而被 `main` 的 `overflow` 裁剪、遮罩覆盖不全、标题被挤出可视区。`Modal` 已改为 `createPortal(document.body)`，后续新增的全屏弹窗/遮罩照此办理。

## Tauri 命令契约（既有，勿改）
- 命令参数一律**扁平 camelCase**（见 `src/lib/tauri.ts` 头部注释）；整结构体命令传 `Partial<T>`，Rust 侧实体开 `serde(default)`。
- 「UI 选择器 → Rust 只认 ID 列表」展开在前端做（如 `broadcast_send` 的 targets 先展开成 deviceId[]）。
