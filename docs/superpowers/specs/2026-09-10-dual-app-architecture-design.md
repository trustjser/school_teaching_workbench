# 教务端与班级端双 App 拆分设计

## 1. 背景与目标

当前项目是一个 Vite + React + Tauri 应用，通过 `appMode` 在同一个安装包内切换教务端和班级端。路由、侧边栏、首次配置向导和部分后端能力都依赖运行时模式，因此用户安装后仍然面对一个“可切换角色”的综合应用。

本变更将其拆为两个独立 app，同时继续在一个 Git 仓库中维护：

- 教务端 app：固定提供学校级管理、考勤大屏、任务下发和统计能力。
- 班级端 app：固定提供班级名册、考勤、任务执行和教务通知能力。
- 两端共享类型、UI、状态、数据库访问、同步协议和 Rust 后端实现。
- 生产环境不要求两端安装在同一台电脑上。
- 开发环境必须可以在同一台设备上同时启动两端，进行真实联调。
- 不迁移旧版综合 app 的本地数据库；新 app 首次启动时重新初始化并配置。

本设计不改变现有局域网通信协议、SQLite 领域模型或业务同步语义，主要调整应用入口、代码边界、构建配置和角色初始化方式。

## 2. 非目标

- 不复制两套 Rust 后端工程。
- 不为两个 app 建立两套业务数据库 schema。
- 不支持生产环境下从设置页把教务端切换为班级端，或反向切换。
- 不提供旧版数据库自动导入、数据迁移或兼容性转换。
- 不在本次变更中重做业务页面视觉设计。

## 3. 现状分析

当前前端只有一套入口：`src/main.tsx` → `App` → 单一路由表。路由表同时注册 `/client/*` 和 `/master/*`，`SideNav` 和 `AppShell` 通过 `useAppStore.settings.appMode` 决定菜单与展示内容。

当前 Rust 侧只有一个 `src-tauri/tauri.conf.json`、一个应用 identifier 和一个产品配置。`AppState` 保存运行模式，若干 command 和 HTTP handler 已经依据该模式做权限校验。`config/settings.rs` 已经使用 Tauri identifier 参与新设备 ID 命名空间生成；Axum 服务会从 5178 开始自动探测可用端口。

因此拆分可以复用绝大多数业务实现，但必须把“路由可见性”和“应用角色”从数据库可变配置升级为 app target 的固定属性。

## 4. 总体架构

目标目录结构如下：

```text
lan-workbench/
├── apps/
│   ├── affairs/
│   │   ├── index.html
│   │   └── src/
│   │       ├── main.tsx
│   │       ├── App.tsx
│   │       ├── router.tsx
│   │       ├── pages/
│   │       └── setup/
│   └── classroom/
│       ├── index.html
│       └── src/
│           ├── main.tsx
│           ├── App.tsx
│           ├── router.tsx
│           ├── pages/
│           └── setup/
├── packages/
│   └── shared/
│       └── src/
│           ├── components/
│           ├── constants/
│           ├── hooks/
│           ├── lib/
│           ├── store/
│           ├── types/
│           └── styles/
├── src-tauri/
│   ├── tauri.affairs.conf.json
│   ├── tauri.classroom.conf.json
│   └── src/
└── package.json
```

第一阶段继续使用根目录 `package.json` 和单一依赖锁文件，不引入重复依赖或多套 Node 安装流程。`apps/*` 和 `packages/shared` 是源码边界；构建由根脚本统一编排。

### 4.1 应用入口

两个 app 各自拥有独立的 HTML、React 入口、根组件和路由表。入口负责：

1. 应用 target 常量初始化。
2. 主题和全局样式加载。
3. 注册共享 Toast 与启动引导。
4. 挂载只属于本端的路由树。

路由表不再注册另一端的页面。这样即使用户手动修改 hash，也不会进入另一端页面。

### 4.2 共享模块

以下代码迁入 `packages/shared`，并保持单一实现：

- `types`、`constants`。
- Tauri command 封装、数据库数据服务、事件、导出、格式化和加密工具。
- Zustand stores：学生、考勤、任务、广播、设备、队列及共享 app 状态。
- 通用 UI 组件、主题、动效、Toast、加载态和错误展示。
- 与角色无关的同步与网络状态组件。

共享 `AppShell` 只负责布局，不再根据 `mode` 决定导航；`TopBar`、`StatusBar` 和 `SyncIndicator` 通过只读的 app target 展示必要的端类型信息。

### 4.3 端专属模块

教务端专属页面：

- `MasterHome`
- `DeviceMonitor`
- `AttendanceBoard`
- `GradeClassManage`
- `BroadcastCenter`
- `Analytics`
- `TaskDashboard`
- `TaskDetail`

班级端专属页面：

- `ClientHome`
- `StudentRoster`
- `CheckinPage`
- `TaskManage`
- `TaskMatrix`
- `InboxPage`

首次配置和设置页分别拆为教务端、班级端版本。共享密钥校验、输入控件和通用表单片段仍放在 `packages/shared`。教务端向导负责学校信息与密钥；班级端向导负责连接教务端、加载目录、认领教室和录入密钥。

## 5. App target 与角色锁定

新增一个固定的 app target 概念，取值为 `affairs` 或 `classroom`。前端入口使用固定 target，不从 URL 或数据库选择；共享状态可以保存只读的 target/app mode，用于展示和数据分流，但不提供修改方法。

Rust 侧根据 `app.config().identifier` 判定 target：

```text
cn.yipaike.lanworkbench.affairs          -> master
cn.yipaike.lanworkbench.classroom        -> client
cn.yipaike.lanworkbench.affairs.dev      -> master
cn.yipaike.lanworkbench.classroom.dev    -> client
```

启动时将 target 作为 `AppState` 的固定角色：

- `ensure_defaults` 以 target 作为新库默认 `app_mode` 和设备 ID 命名空间。
- 首次配置 command 只接受与 target 匹配的 mode；端专属向导不再让用户选择 mode。
- `settings_switch_mode` 不再注册为前端可用 command；若保留后端函数用于兼容编译，必须统一返回模式错误。
- 现有 command 和 HTTP handler 的模式校验继续保留，必要处补齐，使错误配置不能绕过前端边界。

这样即便用户修改本地配置，也不能把教务端安装包变成班级端安装包。

## 6. Tauri 与构建配置

新增两套 Tauri 配置，分别定义：

- `productName`：教务端、班级端。
- `identifier`：生产环境使用不同 bundle identifier。
- `build.beforeDevCommand`、`build.devUrl` 和 `build.frontendDist`。
- 端专属窗口标题与图标。
- 共享的 capabilities、插件和窗口尺寸配置。

生产 identifier：

```text
cn.yipaike.lanworkbench.affairs
cn.yipaike.lanworkbench.classroom
```

开发 identifier：

```text
cn.yipaike.lanworkbench.affairs.dev
cn.yipaike.lanworkbench.classroom.dev
```

不同 identifier 会使 Tauri 使用不同的应用数据目录，满足新 app 独立初始化，也避免同机联调时数据库、设备 ID 和日志相互覆盖。

根脚本提供：

```text
npm run dev:affairs
npm run dev:classroom
npm run tauri:dev:affairs
npm run tauri:dev:classroom
npm run tauri:build:affairs
npm run tauri:build:classroom
```

前端开发端口固定为：

```text
教务端：1420，HMR：1421
班级端：1430，HMR：1431
```

Rust API 继续使用现有从 5178 起的端口探测机制。双开时一个实例使用 5178，另一个实例自动选择下一个可用端口，并通过 mDNS 发布实际端口；不把开发端口写死到业务协议中。

## 7. 数据与兼容策略

本次明确不迁移旧版综合 app 数据。具体规则：

- 新 identifier 对应新应用数据目录。
- 新 app 首次启动执行现有数据库 migration，并进入端专属首次配置。
- 旧版 identifier 的数据库保留在原目录，不读取、不删除、不转换。
- 不修改已有 migration 的历史内容。
- 端专属默认配置必须在新库初始化时写入正确的 target，避免首次启动先落成 client 再切换。

局域网侧仍使用现有共享密钥、mDNS 服务、Axum API、HMAC/AES 封装、广播回执和离线队列。两个 app 的本地数据库独立，但业务数据通过既有协议同步。

## 8. 错误处理

- 用户尝试访问另一端 hash 路由时，落入本端首页或 404，不加载另一端页面。
- target 与 command mode 不一致时，Rust 返回现有模式错误，前端统一显示错误 Toast。
- 两个开发实例端口冲突时，沿用现有自动探测并在状态栏显示实际 API 端口。
- 首次配置失败时保持在本端向导，不将数据库标记为已完成。
- 共享同步协议和密钥校验失败沿用现有错误码和离线队列机制。

## 9. 测试与验收

### 静态与单元验证

- `npm run typecheck` 通过。
- 教务端和班级端分别执行生产前端 build，均通过。
- `cargo test` 通过，覆盖 target 判定、模式锁定、默认配置和现有网络/数据测试。
- 搜索确认端专属入口不再导入另一端页面和另一端路由。

### 双端启动验证

- `npm run tauri:dev:affairs` 可启动教务端窗口。
- `npm run tauri:dev:classroom` 可在同一台设备启动班级端窗口。
- 两端同时运行时，各自使用独立数据目录、设备 ID、窗口标题和前端端口。
- 两端实际 API 端口不同，mDNS 发现结果包含正确角色和端口。

### 业务回归验证

- 教务端可以维护目录、查看节点和考勤大屏、下发任务、查看回执与导出统计。
- 班级端可以加载/认领目录、维护名册、登记考勤、处理任务和接收教务通知。
- 教务端下发任务后，班级端能收到并生成班级待办；班级端回执能回到教务端。
- 离线队列、自动补发、密钥错误和端口冲突行为保持可用。
- 生产构建产物的产品名、identifier、图标和默认角色均正确。

## 10. 实施顺序

1. 建立共享源码边界和双 app 入口，保留现有业务实现。
2. 拆分两个 app 的路由、布局导航、设置和首次配置。
3. 增加双 Tauri 配置及根目录开发/构建脚本。
4. 修改 Rust target 判定、默认配置和模式锁定。
5. 删除旧的运行时模式切换入口，补齐相关类型和测试。
6. 执行类型检查、两个前端构建、Rust 测试和同机双端联调。

