# 教务端与班级端双 App 拆分 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将现有单一 Tauri 工作台拆为教务端和班级端两个独立 app，同时共享前端基础设施、业务代码和 Rust 后端，并支持同机双端联调。

**Architecture:** 使用 `apps/affairs` 与 `apps/classroom` 两个固定前端入口，使用 `packages/shared` 保存共享类型、UI、stores、Tauri 数据服务和同步能力。两个 Tauri 配置使用不同 identifier、产品名、图标和前端端口；Rust 根据 identifier 推导固定角色，数据库不再决定 app 角色。

**Tech Stack:** React 18、TypeScript、Vite 5、React Router 6、Zustand、Tauri 2、Rust、Axum、SQLite、npm。

**Spec:** `docs/superpowers/specs/2026-09-10-dual-app-architecture-design.md`

## Global Constraints

- 生产 identifier 必须为 `cn.yipaike.lanworkbench.affairs` 和 `cn.yipaike.lanworkbench.classroom`。
- 开发 identifier 必须为 `cn.yipaike.lanworkbench.affairs.dev` 和 `cn.yipaike.lanworkbench.classroom.dev`。
- 教务端固定映射 Rust `master`，班级端固定映射 Rust `client`。
- 旧版综合 app 的本地数据库不迁移、不读取、不删除、不转换。
- 两个前端入口必须共享同一份业务类型、UI、stores、数据库访问和同步协议实现。
- Rust 后端只维护一份；现有 Axum 端口从 5178 起自动探测，不能把业务协议绑定到开发端口。
- 前端开发端口必须为教务端 1420/HMR 1421、班级端 1430/HMR 1431。
- 生产环境不提供运行模式切换；访问另一端 hash 路由只能落到本端首页或 404。
- 每个任务完成后运行该任务列出的验证命令并单独提交。

## 文件与模块地图

### 新建文件

- `apps/affairs/index.html`：教务端 HTML 壳。
- `apps/affairs/src/main.tsx`、`App.tsx`、`router.tsx`：教务端入口、根组件和路由。
- `apps/affairs/src/setup/SetupWizard.tsx`、`pages/Settings.tsx`：教务端首次配置与设置。
- `apps/classroom/index.html`：班级端 HTML 壳。
- `apps/classroom/src/main.tsx`、`App.tsx`、`router.tsx`：班级端入口、根组件和路由。
- `apps/classroom/src/setup/SetupWizard.tsx`、`pages/Settings.tsx`：班级端首次配置与设置。
- `packages/shared/src/app-target.ts`：前端固定 app target 与 `AppMode` 映射。
- `packages/shared/src/styles/index.css`、`packages/shared/src/vite-env.d.ts`：共享样式和 Vite 类型声明。
- `src-tauri/src/config/target.rs`：Rust identifier 到固定角色的映射和单元测试。
- `src-tauri/tauri.affairs.conf.json`、`src-tauri/tauri.classroom.conf.json`：两个 Tauri 构建配置。

### 移动到共享目录

- `src/components/*` → `packages/shared/src/components/*`。
- `src/constants/*` → `packages/shared/src/constants/*`。
- `src/hooks/*` → `packages/shared/src/hooks/*`。
- `src/lib/*` → `packages/shared/src/lib/*`。
- `src/store/*` → `packages/shared/src/store/*`。
- `src/types/*` → `packages/shared/src/types/*`。
- `src/index.css`、`src/theme.css` → `packages/shared/src/styles/*`。

### 移动到端专属目录

- `src/pages/MasterHome.tsx`、`DeviceMonitor.tsx`、`AttendanceBoard.tsx`、`GradeClassManage.tsx`、`BroadcastCenter.tsx`、`Analytics.tsx`、`TaskDashboard.tsx`、`TaskDetail.tsx` → `apps/affairs/src/pages/`。
- `src/pages/ClientHome.tsx`、`StudentRoster.tsx`、`CheckinPage.tsx`、`TaskManage.tsx`、`TaskMatrix.tsx`、`InboxPage.tsx` → `apps/classroom/src/pages/`。
- `src/components/setup/SetupWizard.tsx` → 拆为两个 app 的端专属向导；共享输入控件、密钥校验和目录查询函数保留在 `packages/shared`。
- `src/pages/Settings.tsx` → 拆为两个 app 的设置页；共享设置展示组件放入 `packages/shared/src/components/settings/`。

### 修改但不移动

- `package.json`、`tsconfig.json`、`tsconfig.node.json`、`vite.config.ts`：根依赖、路径别名和脚本。
- `vite.shared.ts`：两个 app 共用的 Vite 配置工厂。
- `src-tauri/src/app.rs`、`state.rs`、`config/mod.rs`、`config/settings.rs`、`commands/settings_cmd.rs`：固定 target、默认配置和模式切换移除。
- `src-tauri/src/net/server.rs`：只增加端口自动探测的回归验证，不改变 5178 起始端口和探测语义。
- `src-tauri/capabilities/default.json`：确保两个配置都使用同一能力集合。
- `tailwind.config.js`、`postcss.config.js`：让两个 Vite 根入口继续使用同一套样式配置。

## Task 1: 建立固定 App Target 契约

**Files:**
- Create: `packages/shared/src/app-target.ts`
- Create: `src-tauri/src/config/target.rs`
- Modify: `src-tauri/src/config/mod.rs`
- Modify: `src-tauri/src/state.rs`
- Test: `src-tauri/src/config/target.rs` 内的 Rust 单元测试

**Interfaces:**
- Produces frontend `APP_TARGET`, `AppTarget`, `appModeForTarget()`。
- Produces Rust `AppTarget::from_identifier(&str) -> Result<AppTarget, AppError>`、`AppTarget::mode() -> AppMode`。
- `AppState` 增加固定 `target: AppTarget`，保留 `mode()` 作为 `target.mode()` 的只读结果。

- [ ] **Step 1: 先写 Rust target 映射测试**

```rust
#[test]
fn maps_production_and_dev_identifiers_to_fixed_modes() {
    assert_eq!(AppTarget::from_identifier("cn.yipaike.lanworkbench.affairs").unwrap().mode(), AppMode::Master);
    assert_eq!(AppTarget::from_identifier("cn.yipaike.lanworkbench.classroom").unwrap().mode(), AppMode::Client);
    assert_eq!(AppTarget::from_identifier("cn.yipaike.lanworkbench.affairs.dev").unwrap().mode(), AppMode::Master);
    assert_eq!(AppTarget::from_identifier("cn.yipaike.lanworkbench.classroom.dev").unwrap().mode(), AppMode::Client);
}

#[test]
fn rejects_unknown_identifier_instead_of_defaulting_to_client() {
    assert!(AppTarget::from_identifier("cn.example.unknown").is_err());
}
```

- [ ] **Step 2: 运行测试确认新接口尚不存在**

Run: `cargo test target --manifest-path src-tauri/Cargo.toml`

Expected: FAIL because `target.rs` and `AppTarget` are not implemented.

- [ ] **Step 3: 实现 Rust target 类型和 AppState 固定角色**

实现 `AppTarget` 的四个 identifier 精确匹配；未知 identifier 返回 `AppError::validation`。在 `config/mod.rs` 导出模块，在 `AppState` 构造字段中保存 target，并让 `mode()` 从 target 返回固定值。

- [ ] **Step 4: 实现前端 target 常量**

```ts
export type AppTarget = 'affairs' | 'classroom';
export const APP_MODE_BY_TARGET = {
  affairs: 'master',
  classroom: 'client',
} as const;

export function appModeForTarget(target: AppTarget): 'master' | 'client' {
  return APP_MODE_BY_TARGET[target];
}
```

每个 app 入口必须显式导出固定值：`const APP_TARGET: AppTarget = 'affairs'` 或 `const APP_TARGET: AppTarget = 'classroom'`；共享模块不得在 target 缺失时默认成任一端。

- [ ] **Step 5: 运行测试确认 target 契约通过**

Run: `cargo test target --manifest-path src-tauri/Cargo.toml`

Expected: PASS for production/dev identifiers and unknown identifier rejection.

- [ ] **Step 6: Commit**

```bash
git add packages/shared/src/app-target.ts src-tauri/src/config/target.rs src-tauri/src/config/mod.rs src-tauri/src/state.rs
git commit -m "feat: add fixed app target contract"
```

## Task 2: 建立 shared 源码边界并保留可编译入口

**Files:**
- Move: `src/components/*`, `src/constants/*`, `src/hooks/*`, `src/lib/*`, `src/store/*`, `src/types/*` → `packages/shared/src/`
- Move: `src/index.css`, `src/theme.css`, `src/vite-env.d.ts` → `packages/shared/src/styles/` and `packages/shared/src/`
- Create: `packages/shared/src/index.ts`
- Modify: all moved TypeScript files with imports beginning `@/`
- Modify: `tsconfig.json`, `tsconfig.node.json`, `vite.config.ts`

**Interfaces:**
- Produces aliases `@shared/*` → `packages/shared/src/*` and `@affairs/*` / `@classroom/*` for app code.
- Shared code must not import from `apps/affairs` or `apps/classroom`.

- [ ] **Step 1: 记录当前基线编译结果**

Run: `npm run typecheck`

Expected: record the existing result before the move in the task notes; do not change unrelated failures.

- [ ] **Step 2: 移动共享源文件并更新路径别名**

使用 `git mv` 保留历史；将根 TS alias 改为：

```json
"paths": {
  "@shared/*": ["packages/shared/src/*"],
  "@affairs/*": ["apps/affairs/src/*"],
  "@classroom/*": ["apps/classroom/src/*"]
}
```

把共享文件内部的 `@/…` 改为 `@shared/…`，把 CSS 入口改为 `@shared/styles/index.css`。

- [ ] **Step 3: 创建 shared 统一出口并完成最小引用验证**

```ts
export * from './app-target';
export * from './types';
```

保留现有按文件路径导入方式，不强制把全部模块改成 barrel import，避免扩大重构范围。

- [ ] **Step 4: 运行类型检查确认 shared 移动没有引入新错误**

Run: `npm run typecheck`

Expected: PASS with `tsconfig.json` including `packages/shared/src`; no shared alias resolution error is allowed.

- [ ] **Step 5: Commit**

```bash
git add apps packages src tsconfig.json tsconfig.node.json vite.config.ts
git commit -m "refactor: move frontend infrastructure into shared package"
```

## Task 3: 拆分两个前端入口和固定路由

**Files:**
- Create: `apps/affairs/index.html`, `apps/affairs/src/main.tsx`, `apps/affairs/src/App.tsx`, `apps/affairs/src/router.tsx`
- Create: `apps/classroom/index.html`, `apps/classroom/src/main.tsx`, `apps/classroom/src/App.tsx`, `apps/classroom/src/router.tsx`
- Move: 教务端页面到 `apps/affairs/src/pages/`
- Move: 班级端页面到 `apps/classroom/src/pages/`
- Modify: `packages/shared/src/components/layout/AppShell.tsx`, `SideNav.tsx`, `TopBar.tsx`
- Remove: `src/main.tsx`, `src/App.tsx`, `src/router/index.tsx`（移动后不再作为入口）

**Interfaces:**
- Affairs router registers only `/`, `/devices`, `/attendance`, `/directory`, `/broadcast`, `/tasks`, `/tasks/:taskId`, `/analytics`, `/settings`。
- Classroom router registers only `/`, `/students`, `/checkin`, `/tasks`, `/matrix`, `/inbox`, `/settings`。
- Shared `AppShell` consumes `navItems` and `appTarget` props，不读取 `appMode` 决定路由。

- [ ] **Step 1: 先建立路由隔离检查脚本**

Create `scripts/check-app-boundaries.mjs`:

```js
import { readFileSync } from 'node:fs';

const checks = [
  ['apps/affairs/src/router.tsx', ['/client', 'ClientHome', 'StudentRoster']],
  ['apps/classroom/src/router.tsx', ['/master', 'MasterHome', 'TaskDashboard']],
];

for (const [file, forbidden] of checks) {
  const source = readFileSync(file, 'utf8');
  for (const value of forbidden) {
    if (source.includes(value)) throw new Error(`${file} contains forbidden reference: ${value}`);
  }
}
```

Add script: `"check:boundaries": "node scripts/check-app-boundaries.mjs"`。

- [ ] **Step 2: 运行边界检查确认它先失败**

Run: `npm run check:boundaries`

Expected: FAIL because the two new router files do not exist yet.

- [ ] **Step 3: 创建两个固定路由树**

教务端根路由直接渲染 `MasterHome`，班级端根路由直接渲染 `ClientHome`；两端分别复用共享的 bootstrap/loading/error 根组件，但不再使用 `RootRedirect` 选择模式。

- [ ] **Step 4: 把布局改成显式导航参数**

```tsx
type AppShellProps = {
  appTarget: AppTarget;
  navItems: NavItem[];
  children: React.ReactNode;
};
```

`SideNav` 只渲染传入的 `navItems`；删除 `mode === 'master' ? … : …` 分支。`TopBar` 和 `StatusBar` 接收只读 `appTarget`，显示固定端名称。

- [ ] **Step 5: 运行边界、类型和两端前端 build**

Run: `npm run check:boundaries`

Expected: PASS；两个 router 文件不包含另一端页面或路径。

Run: `npm run typecheck`

Expected: PASS after the root Vite entry is replaced by the app entries.

- [ ] **Step 6: Commit**

```bash
git add apps packages scripts package.json src
git commit -m "refactor: split affairs and classroom frontend routes"
```

## Task 4: 拆分首次配置、设置和模式切换入口

**Files:**
- Create: `apps/affairs/src/setup/SetupWizard.tsx`, `apps/classroom/src/setup/SetupWizard.tsx`
- Create: `packages/shared/src/components/settings/SettingsPanel.tsx`
- Move/Modify: `src/components/setup/SetupWizard.tsx`, `src/pages/Settings.tsx` into the target app files
- Modify: `packages/shared/src/lib/db.ts`, `packages/shared/src/store/useAppStore.ts`, `packages/shared/src/hooks/useBootstrap.ts`
- Modify: `packages/shared/src/types/models.ts`

**Interfaces:**
- `settingsCompleteSetup` no longer accepts user-selected `mode`; Rust derives the mode from `AppState`。
- `useAppStore` no longer exposes `switchMode`。
- `SettingsPanel` receives `{ appTarget: AppTarget }` and never renders a mode selector。

- [ ] **Step 1: 先修改 Rust/TS API 契约测试用例**

Add Rust test in `settings_cmd.rs`:

```rust
#[test]
fn setup_mode_is_derived_from_fixed_state_target() {
    assert_eq!(setup_mode_for_state(AppMode::Master), AppMode::Master);
    assert_eq!(setup_mode_for_state(AppMode::Client), AppMode::Client);
}
```

Add a TypeScript compile-time call site in both setup wizards using `settingsCompleteSetup({ deviceName, ... })` without a selectable mode field.

- [ ] **Step 2: 运行 Rust 测试确认新的固定角色函数尚不存在**

Run: `cargo test setup_mode --manifest-path src-tauri/Cargo.toml`

Expected: FAIL because `setup_mode_for_state` has not been added.

- [ ] **Step 3: 拆分两个向导**

教务端向导保留学校名、随机生成/输入共享密钥；班级端向导保留输入密钥、目录同步、教室认领和班级绑定。两者都调用同一份 shared 密钥格式校验和数据库函数，但不渲染“运行模式”选择。

- [ ] **Step 4: 抽取共享设置面板并移除切换模式 UI**

将通用缩放、主题、端口、密钥信息和重置逻辑放入 `SettingsPanel`；删除 `switchMode` 调用、模式切换按钮和依赖 `mode` 的跳转。班级端保留重置客户端配置，教务端不渲染该操作。

- [ ] **Step 5: 修改 bootstrap 和 app store 为固定 target**

`useBootstrap` 从 `APP_TARGET` 初始化 settings 中的只读 app mode；收到 `MODE_CHANGED` 事件时只刷新兼容状态，不允许改变 target，也不再触发路由重建。

- [ ] **Step 6: 运行测试和类型检查**

Run: `cargo test setup_mode --manifest-path src-tauri/Cargo.toml`

Expected: PASS。

Run: `npm run typecheck`

Expected: PASS；项目中不再存在 `switchMode` 的前端引用。

- [ ] **Step 7: Commit**

```bash
git add apps packages src-tauri/src/commands/settings_cmd.rs
git commit -m "feat: lock setup and settings to app target"
```

## Task 5: 增加双 Vite/Tauri 配置和根脚本

**Files:**
- Create: `apps/affairs/vite.config.ts`, `apps/classroom/vite.config.ts`
- Create: `vite.shared.ts`
- Create: `src-tauri/tauri.affairs.conf.json`, `src-tauri/tauri.classroom.conf.json`
- Create: `src-tauri/tauri.affairs.dev.conf.json`, `src-tauri/tauri.classroom.dev.conf.json`
- Modify: `package.json`, `vite.config.ts`, `tailwind.config.js`, `postcss.config.js`
- Modify: `apps/affairs/index.html`, `apps/classroom/index.html`
- Remove: 旧单 app `src-tauri/tauri.conf.json`（仅在两个新配置完全可用后删除）

**Interfaces:**
- `npm run dev:affairs` starts Vite at 1420 with HMR 1421。
- `npm run dev:classroom` starts Vite at 1430 with HMR 1431。
- `npm run build:affairs` and `npm run build:classroom` produce separate frontend dist directories。
- `npm run tauri:dev:affairs` and `npm run tauri:dev:classroom` merge each production config with its dev identifier override。
- `npm run tauri:build:affairs` and `npm run tauri:build:classroom` produce different product identifiers。

- [ ] **Step 1: 先写 package script 期望检查**

Create `scripts/check-build-targets.mjs`:

```js
import { readFileSync } from 'node:fs';

const packageJson = JSON.parse(readFileSync('package.json', 'utf8'));
for (const name of ['dev:affairs', 'dev:classroom', 'build:affairs', 'build:classroom', 'tauri:dev:affairs', 'tauri:dev:classroom', 'tauri:build:affairs', 'tauri:build:classroom']) {
  if (!packageJson.scripts[name]) throw new Error(`missing script: ${name}`);
}
```

Add script: `"check:build-targets": "node scripts/check-build-targets.mjs"`。

- [ ] **Step 2: 运行检查确认脚本尚不存在**

Run: `npm run check:build-targets`

Expected: FAIL because the new target scripts are not present.

- [ ] **Step 3: 创建每端 Vite 配置**

将当前 `vite.config.ts` 的公共逻辑抽到 `vite.shared.ts` 的 `createViteConfig({ appTarget, root, port, hmrPort, outDir })`；每份 app 配置只传入固定 target 和端口，并通过 `define` 注入 `import.meta.env.VITE_APP_TARGET`。教务端输出 `dist/affairs`，班级端输出 `dist/classroom`。

- [ ] **Step 4: 创建两个 Tauri 配置**

两个配置使用相同窗口、插件、capabilities 和 bundle targets，分别设置：

```json
{
  "productName": "教务端",
  "identifier": "cn.yipaike.lanworkbench.affairs",
  "build": {
    "beforeDevCommand": "npm run dev:affairs",
    "devUrl": "http://localhost:1420",
    "beforeBuildCommand": "npm run build:affairs",
    "frontendDist": "../dist/affairs"
  }
}
```

班级端对应 `班级端`、`cn.yipaike.lanworkbench.classroom`、1430 和 `../dist/classroom`。

同时创建只包含 identifier override 的开发配置，并按顺序合并基础配置和 override：

```json
{
  "identifier": "cn.yipaike.lanworkbench.affairs.dev"
}
```

班级端 override 使用 `cn.yipaike.lanworkbench.classroom.dev`；开发 override 不改变 productName、窗口配置、权限或前端 target。

- [ ] **Step 5: 添加根脚本并校验配置**

```json
"dev:affairs": "vite --config apps/affairs/vite.config.ts",
"dev:classroom": "vite --config apps/classroom/vite.config.ts",
"build:affairs": "tsc --noEmit && vite build --config apps/affairs/vite.config.ts",
"build:classroom": "tsc --noEmit && vite build --config apps/classroom/vite.config.ts",
"tauri:dev:affairs": "tauri dev --config src-tauri/tauri.affairs.conf.json --config src-tauri/tauri.affairs.dev.conf.json",
"tauri:dev:classroom": "tauri dev --config src-tauri/tauri.classroom.conf.json --config src-tauri/tauri.classroom.dev.conf.json",
"tauri:build:affairs": "tauri build --config src-tauri/tauri.affairs.conf.json",
"tauri:build:classroom": "tauri build --config src-tauri/tauri.classroom.conf.json"
```

- [ ] **Step 6: 运行脚本/配置静态检查和两个前端 build**

Run: `npm run check:build-targets`

Expected: PASS。

Run: `npm run build:affairs`

Expected: PASS and create `dist/affairs`。

Run: `npm run build:classroom`

Expected: PASS and create `dist/classroom`。

- [ ] **Step 7: Commit**

```bash
git add apps package.json scripts vite.config.ts vite.shared.ts src-tauri/tauri.affairs.conf.json src-tauri/tauri.classroom.conf.json src-tauri/tauri.affairs.dev.conf.json src-tauri/tauri.classroom.dev.conf.json tailwind.config.js postcss.config.js
git commit -m "build: add independent affairs and classroom app targets"
```

## Task 6: 让 Rust 后端根据 identifier 锁定角色

**Files:**
- Modify: `src-tauri/src/app.rs`, `src-tauri/src/state.rs`
- Modify: `src-tauri/src/config/settings.rs`
- Modify: `src-tauri/src/commands/settings_cmd.rs`, `src-tauri/src/commands/mod.rs`
- Test: `src-tauri/src/config/target.rs`, `src-tauri/src/commands/settings_cmd.rs`, `src-tauri/src/config/settings.rs`, `src-tauri/src/net/server.rs`

**Interfaces:**
- `bootstrap(app)` derives `AppTarget` from `app.config().identifier` before reading mutable app settings。
- `AppState.mode()` always returns `AppTarget::mode()`。
- `ensure_defaults(pool, identity_namespace, target_mode)` writes target mode for the new app database。
- `settings_complete_setup` persists the fixed `state.mode()` and has no user-selectable mode parameter。

- [ ] **Step 1: 先补默认配置和 command 的失败测试**

```rust
#[test]
#[tokio::test]
async fn fresh_target_defaults_use_requested_mode() {
    let dir = std::env::temp_dir().join(format!("lanwb_target_{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let pool = crate::db::create_pool(&dir.join("settings.db")).await.unwrap();
    crate::db::run_migrations(&pool).await.unwrap();
    crate::config::settings::ensure_defaults(
        &pool,
        "cn.yipaike.lanworkbench.affairs",
        AppMode::Master,
    ).await.unwrap();
    assert_eq!(crate::db::repo::settings_repo::get_string(&pool, "app_mode", "client").await.unwrap(), "master");
    pool.close().await;
    std::fs::remove_dir_all(dir).ok();
}
```

- [ ] **Step 2: 运行新增测试确认其失败**

Run: `cargo test fresh_target_defaults --manifest-path src-tauri/Cargo.toml`

Expected: FAIL until target-aware default initialization is wired into settings code.

- [ ] **Step 3: 修改 bootstrap 和 AppState 初始化**

在 `bootstrap` 中先读取 identifier 并调用 `AppTarget::from_identifier`；将 target 和 target.mode() 传入设置默认值与 `AppState`。未知 identifier 直接记录 bootstrap 错误并阻止启动，不降级为 client。

- [ ] **Step 4: 修改 ensure_defaults 使用固定 target**

新库初始化时写入传入的 app mode；不再先默认写 `client` 再等待首次向导切换。设备 ID 继续使用 identifier/target 命名空间，使 dev affairs 与 dev classroom 在同机上不同。

- [ ] **Step 5: 修改设置 command 并移除模式切换注册**

`settings_complete_setup` 直接使用 `state.mode()` 写入 `app_mode`，不再接受前端传入的 mode；删除或禁用 `settings_switch_mode`，并从 `invoke_handler` 中移除其注册。其他 command/HTTP handler 继续使用 `state.mode()` 的已有权限检查。

- [ ] **Step 6: 运行 Rust 全量测试**

Run: `cargo test --manifest-path src-tauri/Cargo.toml`

Expected: PASS，包含 target 映射、默认配置、设置校验和现有数据/网络测试。

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src
git commit -m "feat: lock rust runtime role to tauri target"
```

## Task 7: 完成共享引用清理和文档更新

**Files:**
- Modify: `packages/shared/src/hooks/useBootstrap.ts`, `packages/shared/src/store/useAppStore.ts`, `packages/shared/src/lib/db.ts`, `packages/shared/src/components/layout/*`
- Modify: `docs/01-architecture.md`, `docs/03-tasks.md`
- Test: `.gitignore` covers `dist/affairs`, `dist/classroom` and target build outputs through the existing `dist` rule

**Interfaces:**
- No shared module may import a target-specific page.
- Existing store and sync interfaces remain callable by both app entries。
- Documentation lists two app commands and no longer describes runtime mode selection as the primary architecture。

- [ ] **Step 1: 搜索旧单 app 引用并建立清理清单**

Run: `rg -n "from ['\"]@/|/client|/master|switchMode|settings_switch_mode|src/main\.tsx|tauri\.conf\.json" apps packages src-tauri package.json docs`

Expected: only target-specific route strings, compatibility comments, and the two new Tauri config names remain.

- [ ] **Step 2: 清理 shared 中的运行时 mode 分支**

删除布局和设置中的 role selector；将需要展示端名称的分支改为 `appTarget`；保留业务 stores 中确实影响同步策略的 `AppMode` 只读值。

- [ ] **Step 3: 更新架构与施工文档**

在 `docs/01-architecture.md` 的目录树、应用入口和模式决策章节中记录双 app；在 `docs/03-tasks.md` 的启动、构建和验收命令中增加两个 target，明确旧数据库不迁移。

- [ ] **Step 4: 运行清理检查**

Run: `npm run check:boundaries`

Expected: PASS。

Run: `npm run typecheck`

Expected: PASS。

- [ ] **Step 5: Commit**

```bash
git add packages docs .gitignore package-lock.json
git commit -m "docs: document dual-app development workflow"
```

## Task 8: 双端启动、构建和联调验收

**Files:**
- Modify: no planned implementation files; verification failures must be fixed in the task that introduced them before final acceptance。
- Test: `scripts/check-app-boundaries.mjs`, `scripts/check-build-targets.mjs`, both Vite builds, Rust tests, two Tauri dev launches。

**Interfaces:**
- `npm run tauri:dev:affairs` and `npm run tauri:dev:classroom` can run concurrently on one device。
- Production artifacts have distinct product names, identifiers, app data directories and fixed roles。

- [ ] **Step 1: 运行静态和单元验证**

```bash
npm run check:boundaries
npm run check:build-targets
npm run typecheck
cargo test --manifest-path src-tauri/Cargo.toml
```

Expected: all commands PASS。

- [ ] **Step 2: 分别构建两个前端产物**

```bash
npm run build:affairs
npm run build:classroom
```

Expected: `dist/affairs` 与 `dist/classroom` 均生成，HTML 分别引用对应入口且不引用另一端页面。

- [ ] **Step 3: 同机启动两个开发 app**

在两个终端分别运行：

```bash
npm run tauri:dev:affairs
npm run tauri:dev:classroom
```

Expected: 两个窗口同时启动；标题、端口、数据目录和本机设备 ID 不相同；教务端显示教务导航，班级端显示班级导航。

- [ ] **Step 4: 验证局域网互通链路**

使用教务端维护一个班级并下发测试任务；在班级端执行目录同步、教室认领、接收任务、登记一条考勤并等待回执。关闭/恢复一端网络后确认离线队列自动补发。

Expected: 目录、任务、考勤和回执保持现有协议行为；mDNS 发现列表中的角色和实际 API 端口正确。

- [ ] **Step 5: 验证生产配置和角色锁定**

检查两个 config 的 `productName`、`identifier`、`frontendDist`；在设置页确认没有模式切换；尝试通过 hash 进入另一端路径，确认不会渲染另一端页面。

- [ ] **Step 6: 记录最终验证结果并提交修复**

```bash
git status --short
git diff --check
git add apps packages scripts src-tauri package.json tsconfig.json vite.config.ts docs
git commit -m "test: verify dual-app build and local integration"
```

如果 Step 1–5 没有产生修复改动，则不创建空提交；将上述命令输出和验收结果写入最终交付说明。
