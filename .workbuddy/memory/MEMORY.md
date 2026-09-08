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

## SQLite 表重建（改 CHECK 约束）铁律（2026-09-08 踩坑，代价：pending_queue 永久丢表）
SQLite 不支持 `ALTER TABLE ... ALTER COLUMN` 改 CHECK，只能「改名旧表 → 建新表 → 拷数据 → 删旧表」。这条路上有三个坑，全部踩过：
1. **`RENAME TO` 会连带改写其他表的 `REFERENCES` 子句**。`sync_log` 有 `FOREIGN KEY (queue_id) REFERENCES pending_queue(id)`，把 `pending_queue` 改名成 `_old` 后，`sync_log` 的外键被自动改写指向 `_old`；`_old` 一 DROP，外键就悬空，之后所有 `INSERT INTO sync_log` 报 `no such table: main.pending_queue_old`。
   - 控制这个行为的开关是 **`PRAGMA foreign_keys`（不是 `legacy_alter_table`）**。官方语义：外键启用时 RENAME 会改写 REFERENCES 子句。`legacy_alter_table` 只影响**触发器体与视图定义**里的表名改写。这点我第一版方案搞错了，实测才纠正。
   - 正确做法：结构手术前 `PRAGMA foreign_keys = OFF` + `PRAGMA legacy_alter_table = ON`，做完两个都复位（`foreign_keys = ON`）。
2. **PRAGMA 是连接级设置，连接池会失效**。`DB_MAX_CONNECTIONS = 4`，在 `&DbPool` 上执行 PRAGMA、后续 ALTER 可能落到另一条连接。**整个手术例程必须 `pool.acquire()` 取单一连接，全部语句跑在 `&mut *conn` 上**。且 Rust 无 `finally`，每一处 `?` 提前返回都要保证 PRAGMA 已复位——否则连接带着 `foreign_keys = OFF` 回池复用，外键约束在应用余生静默失效。
3. **建表 DDL 的表名必须是最终真名**。原 Bug 就是 DDL 里写 `CREATE TABLE pending_queue_new`，拷数据却写 `INSERT INTO pending_queue`（已被改名走），语句失败又被 `.ok()` 吞掉，第 4 步再 DROP 旧表 → 表永久消失。
   - **禁止对结构手术类语句用 `.ok()` 吞错**；拷数据用**显式列名**，不用 `SELECT *`；例程收尾必须断言目标表存在，不存在就返回 `Err` 让启动期暴露。
   - 修复例程写成幂等自愈式（`repair_queue_schema`，每次启动都跑）：表缺失时能从 `_new` / `_old` 残留改名恢复并抢救其中数据。注意 001 的 `CREATE TABLE IF NOT EXISTS` 会兜底建空表，所以经 `run_migrations` 的测试走不到「表已丢失」分支，验证该分支必须**直调修复例程**。

## Tailwind className 冲突（已踩：Toggle 的 py-0 px-0 不生效）
- **现象**：调用方传 `className="py-0 px-0"` 想覆盖组件内部的 `px-1 py-2`，无效。
- **根因**：Tailwind 同类属性（如 py-2 / py-0）优先级由**编译后 CSS 的生成顺序**决定（py-2 排在 py-0 前），不看 className 字符串里的顺序。把调用方 className 直接拼在内部类之后，会被内部类覆盖。
- **修法**：引入 `tailwind-merge`（v2，隔离装到 node workspace），新增 `src/lib/cn.ts` 的 `cn(...)` 统一做冲突消解（后者胜出）。`Toggle.tsx`、`Button.tsx` 已改用 `cn(...)` 拼接内部类与调用方 className。`Modal.tsx` 的 `className` 实际没接到面板（拼的是 `widthClass`），属另一独立 bug，未在本轮修。
- **约定**：今后任何「内部基础类 + 外部 className 透传」的组件，一律用 `cn(内部类, className)` 拼接，不要用 `.join(' ')` 裸拼。

## 文档同步约定（已脱节过一次）
`src-tauri/migrations/001_init.sql` 头部声明「与 docs/02-ddl.sql 完全一致，任何修改必须两处同步」，但 002/003 的内容长期未同步进 docs（已于 2026-09-08 补齐 grades/classes/school_years/ux_classes_year 及 pending_queue CHECK）。**改迁移必须同步 docs/02-ddl.sql**，并保证 `sqlite3 :memory: < docs/02-ddl.sql` 整份可执行。建议后续加 CI 校验脚本比对两边规范化 DDL 文本。

## 布局 / 响应式约定
- 全屏遮罩或居中卡片**避免用 `w-screen`**：一旦内容超高出现垂直滚动条，`w-screen`（100vw）会包含滚动条宽度，导致水平滚动条。改用 `w-full` 或 `min-w-full`，并给卡片加 `min-w-0`。
- 表单控件（Input/Select/Textarea 包装器与本机元素）统一加 `min-w-0`，防止 `cols`/`size`/选项文本的内禀最小宽度撑开父容器。
- 两列表单一律写成 `grid-cols-1 sm:grid-cols-2`（移动端单列，≥640px 双列），避免窄窗下双列溢出。
- **固定定位组件必须用 Portal 挂到 body**：`PageTransition` 等路由入场动画使用 `transform`（且 `animation-fill-mode: both`），会导致其内部 fixed 元素的包含块被篡改，从而被 `main` 的 `overflow` 裁剪、遮罩覆盖不全、标题被挤出可视区。`Modal` 已改为 `createPortal(document.body)`，后续新增的全屏弹窗/遮罩照此办理。

## Tauri 命令契约（既有，勿改）
- 命令参数一律**扁平 camelCase**（见 `src/lib/tauri.ts` 头部注释）；整结构体命令传 `Partial<T>`，Rust 侧实体开 `serde(default)`。
- 「UI 选择器 → Rust 只认 ID 列表」展开在前端做（如 `broadcast_send` 的 targets 先展开成 deviceId[]）。

## SQLx 绑定顺序铁律（已踩：upsert 漏绑 seat_no → 编辑学生报 NOT NULL）
- **现象**：编辑学生报 `NOT NULL constraint failed: students.status`。
- **根因**：`student_repo.rs::upsert` 的 INSERT 列含 `seat_no`（第 8 列），但 `.bind()` 链漏了 `.bind(student.seat_no)`，导致 `status` 被绑进 `seat_no` 列、`status_since`（常 `None`）被绑进 `status` 列。**少一个 `.bind()` 会使其后所有绑定整体错位**，NULL 落进错误的 NOT NULL 列正是报错的精确路径；快乐路径因两字段都非空只静默错位、不报错，极具迷惑性。
- **修法**：`.bind(&student.class_id)` 后补 `.bind(student.seat_no)`。`upsert_in_tx` 一直正确（有 seat_no 绑定），仅主 `upsert` 漏绑——所以批量导入（走 tx）不暴露、单条编辑（走主 upsert）才炸。
- **约定**：改 INSERT 列时必须同步核对 `.bind()` 链，bind/column 元数逐一对齐；排查 `NOT NULL` 报错第一步先数 bind/column 是否匹配。空串 status → `None` 走 `DEFAULT 'active'` 的兜底保留（防御），但不是本次根因。

## 本机联调 教务端⇄班级端（双实例）铁律
- 两份副本必须 `tauri.conf.json` 的 `identifier` 不同，否则 Tauri 按 identifier 解析应用数据目录 → 两份共用同一 SQLite/设备身份/模式设置/`api_port`，无法互相同步。副本改 `cn.yipaike.lanworkbench`→`cn.yipaike.lanworkbench.client`（加后缀即可）。
- vite `strictPort:true` 且 tauri `devUrl=1420`：第二份 `tauri dev` 会端口冲突。副本改 `vite.config.ts` `server.port`→1421 + `tauri.conf.json` `build.devUrl`→`http://localhost:1421`。
- 同步 API 端口（`config/constants.rs` `API_PORT=5178`，`net/server.rs` 从 5178 探测 +20）**不用改**，双实例自动错开 5178/5179。
- 模式（master/client）、`completed_setup`、device_id 都持久化在 DB（`app_settings`）→ 不同 identifier 即独立数据库，两端可分别设为教务端/班级端。
- 共享密钥（SetupWizard 可选）**两端必须完全一致**，否则能发现但解密失败不同步。班级端首次配置时目录未同步，年级/班级先手动填，待目录同步后再在设置里重绑。
- mDNS（`_schworkbench._tcp.local.`，5353）靠回环组播，单机双实例通常可用；若班级端收不到目录，先查两端 DeviceMonitor 互见 + 密钥一致。当前无手动 IP:端口 添加对端兜底 UI（如需可加）。
