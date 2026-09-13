# 项目长期记忆：lan-workbench（局域网教务协同工作台）

> 条目格式：现象 → 根因 → 修法/约定。

## 1. 技术栈与结构
React 18 + TS 5 + Vite 5 + Tauri v2 + Tailwind v3.4 + Zustand 4；react-router-dom v6（HashRouter）；lucide-react；SQL 走 `@tauri-apps/plugin-sql`。

**双 App 拆分（2026-09-10）**：`apps/affairs`（教务端）/ `apps/classroom`（班级端）两个入口，共享代码在 `packages/shared`。
- 角色由 bundle **identifier** 决定：`AppTarget::from_identifier`（`src-tauri/src/config/target.rs`），未知 identifier 拒绝启动。
- `AppState.target` 取代可变 `mode`；`state.mode() === target.mode()`；`ensure_defaults()` 强制覆盖 DB 里被篡改的 `app_mode`。
- 前端 `mountApp(APP_TARGET, el)` 注入共享层；共享模块只能经 `getAppTarget()` 读，缺 target 禁止回退默认值。
- 别名 `@shared/*`、`@affairs/*`、`@classroom/*`；旧 `@/*` 已移除。
- 默认 `src-tauri/tauri.conf.json` 是「等价教务端」占位（裸 `cargo` 下 tauri-build 必须能读到）；出包一律 `tauri:build:affairs/classroom`（CLI 以 `--config` 覆盖）。
- 校验脚本：`check-app-boundaries.mjs`（端边界）、`check-build-targets.mjs`（8 npm 脚本 + 4 份 Tauri 配置）——**新增/改名配置或脚本要同步**。
- `AppRoot`（`packages/shared/src/components/layout/AppRoot.tsx`）以 `createAppRoutes({appTarget,navItems,setup,routes})` 收敛骨架，两端 router 只声明 `navItems`/`routes`。
- `settings_switch_mode` 与 `useAppStore.switchMode` 已删；`settings_complete_setup` 不再收 `mode`。

## 2. 主题 / 样式 / 布局约定
- 三套主题：light（默认，无类）/ dark（`.dark`）/ high-contrast（`.high-contrast`），由 `<html>` 上的类驱动。令牌在 `src/theme.css`（CSS 变量 `--surface*`/`--ink*`/`--brand*`/`--glass*`，各自覆盖）；`tailwind.config.js` 把 `surface.*`、`ink.*` 映射到 `rgb(var(--x)/<alpha-value>)`，`brand/state/node` 为固定高饱和色。
- 落地：`src/lib/theme.ts::applyThemeClass` + `main.tsx` 订阅 `useAppStore.theme`（`setTheme` 写 DB → 落地 DOM + localStorage，首帧前读 localStorage 防闪）；切主题给 `<html>` 挂 `.theme-anim`（320ms 后移除）。切换器 `ThemeSwitcher`。
- **写组件一律用语义令牌**（`bg-surface-raised`/`bg-surface-sunken`/`bg-surface-muted`/`border-surface-border`/`text-ink`/`text-ink-soft`/`text-ink-muted`），**禁止写死** `bg-white`、`slate-*`、`gray-*`、`zinc-*`、`neutral-*`。
- 动画关键帧集中在 `tailwind.config.js`；路由入场 `PageTransition`（按 pathname 重挂载），错落 `Stagger`/`Reveal`。`.glass` = 顶栏/侧栏；`.card-interactive` = 抬升卡片（Card 带 `data-card` 供高对比加描边）。
- **避免 `w-screen`**（用 `w-full`/`min-w-full` + `min-w-0`）：内容超高出现纵向滚动条时 `100vw` 会把滚动条宽度算进去 → 横向滚动条。
- 表单控件（Input/Select/Textarea 包装器与本机元素）统一加 `min-w-0`，防 `cols`/`size`/选项文本的内禀最小宽度撑开父容器。两列表单一律 `grid-cols-1 sm:grid-cols-2`。
- **固定定位组件必须 Portal 到 body**：`PageTransition` 等入场动画用 `transform`（且 `animation-fill-mode: both`），会篡改内部 fixed 元素的包含块 → 被 `main` 的 `overflow` 裁剪、遮罩覆盖不全、标题被挤出可视区。`Modal` 已改 `createPortal(document.body)`。

## 3. 组件与接口约定
- **className 透传必须用 `cn(...)`**（`src/lib/cn.ts`，基于 `tailwind-merge` v2）。现象：调用方 `py-0 px-0` 覆盖不了组件内部 `px-1 py-2`；根因：同类 Tailwind 属性的优先级由**编译后 CSS 生成顺序**决定，与 className 字符串先后无关，裸拼必被内部类覆盖。`Toggle.tsx`/`Button.tsx` 已改用 `cn()`。**禁止 `.join(' ')` 裸拼。**（`Modal.tsx` 的 `className` 实际拼的是 `widthClass`，没接到面板，遗留 bug 未修。）
- **Tauri 命令契约（既有，勿改）**：参数一律扁平 **camelCase**（见 `src/lib/tauri.ts` 头部注释）；整结构体命令传 `Partial<T>`，Rust 侧实体开 `serde(default)`。「UI 选择器 → Rust 只认 ID 列表」的展开在前端做（如 `broadcast_send` 的 targets 先展开成 `deviceId[]`）。
- **SQLx 绑定顺序铁律**：现象——编辑学生报 `NOT NULL constraint failed: students.status`；根因——`student_repo.rs::upsert` 的 INSERT 列含 `seat_no`（第 8 列）但 `.bind()` 链漏了 `.bind(student.seat_no)`，**少一个 bind 会让其后所有绑定整体错位**（`status` 落进 `seat_no`、`status_since`(常 `None`) 落进 `status`）；快乐路径两字段都非空只静默错位。`upsert_in_tx` 一直正确 ⇒ 批量导入（走 tx）不暴露、单条编辑（走主 upsert）才炸。**约定：改 INSERT 列必须同步核对 `.bind()` 链；排查 NOT NULL 第一步先数 bind/column 元数。**

## 4. 数据层（SQLite）
- **迁移只经 `run_migrations`**（`src-tauri/src/db/mod.rs`，由 `init_db` 调用）；`tauri-plugin-sql` 在 `app.rs` 中**未注册** migrations。
- **本项目捆绑的 SQLite < 3.35**：不支持 `ALTER TABLE ... ADD COLUMN IF NOT EXISTS`（ALTER 无 IF NOT EXISTS）。`run_migrations` 对 `ADD COLUMN` 做幂等保护（`PRAGMA table_info` 查列，存在跳过、否则普通 ALTER）。**约定：给旧表加列一律写普通 `ALTER TABLE x ADD COLUMN y TYPE`，幂等交给 Rust 侧。**
- **改 CHECK 约束 = 表重建（2026-09-08，代价：pending_queue 永久丢表）**。SQLite 不支持 `ALTER COLUMN`，只能「改名旧表 → 建新表 → 拷数据 → 删旧表」，三个坑：
  1. **`RENAME TO` 会连带改写其他表的 `REFERENCES`**：`sync_log` 有 `FOREIGN KEY (queue_id) REFERENCES pending_queue(id)`，`pending_queue` 改名 `_old` 后该外键被改写指向 `_old`；`_old` 一 DROP 外键悬空 → 此后所有 `INSERT INTO sync_log` 报 `no such table: main.pending_queue_old`。开关是 **`PRAGMA foreign_keys`（不是 `legacy_alter_table`）**——外键启用时 RENAME 才改写 REFERENCES，`legacy_alter_table` 只管触发器体与视图定义里的表名。做法：术前 `PRAGMA foreign_keys = OFF` + `PRAGMA legacy_alter_table = ON`，术后复位。
  2. **PRAGMA 是连接级设置，连接池会失效**（`DB_MAX_CONNECTIONS = 4`）：在 `&DbPool` 上执行 PRAGMA、后续语句可能落到另一条连接。**整个手术例程必须 `pool.acquire()` 取单一连接，全部语句跑在 `&mut *conn` 上**；Rust 无 `finally`，每处 `?` 提前返回都要保证 PRAGMA 已复位，否则连接带 `foreign_keys = OFF` 回池复用，外键约束在应用余生静默失效。
  3. **建表 DDL 的表名必须是最终真名**：原 Bug 是 DDL 写 `CREATE TABLE pending_queue_new`、拷数据却写 `INSERT INTO pending_queue`（已被改名走），语句失败又被 `.ok()` 吞掉，再 DROP 旧表 → 表永久消失。**禁止对结构手术类语句用 `.ok()` 吞错**；拷数据用**显式列名**（不用 `SELECT *`）；收尾必须断言目标表存在，不存在即 `Err` 让启动期暴露。修复例程写成幂等自愈式（`repair_queue_schema`，每次启动跑）。注意 001 的 `CREATE TABLE IF NOT EXISTS` 会兜底建空表 ⇒ 经 `run_migrations` 的测试走不到「表已丢失」分支，**验证该分支必须直调修复例程**。
- **文档同步**：`src-tauri/migrations/001_init.sql` 声明「与 `docs/02-ddl.sql` 完全一致，必须两处同步」（002/003 曾长期未同步，2026-09-08 已补齐）。**改迁移必须同步 `docs/02-ddl.sql`**，保证 `sqlite3 :memory: < docs/02-ddl.sql` 整份可执行。

## 5. 代码风格与 CI 关卡
- **提交 Rust 前必跑 `npm run fmt`**（= `cargo fmt --manifest-path src-tauri/Cargo.toml`）；`npm run fmt:check` 复现 CI 的 `Cargo fmt check`。
- **判断 rustfmt 改动无语义**：「去空白比哈希」不够（rustfmt 会加行尾逗号与闭包花括号）。应 `tr -d '[:space:],'` 后逐字符 diff，预期只剩 `use` 重排与花括号增减。
- **工具链**：CI 与 release 统一钉 **Rust 1.96.1**，唯一来源是两份 workflow 的 `env.RUST_TOOLCHAIN`，step 用 `${{ env.RUST_TOOLCHAIN }}` 引用。
- **`Cargo.toml` 的 `rust-version = "1.88"` 是「依赖树真实 MSRV」而非编译版本**，作用是让过旧工具链报「requires rustc 1.88 or newer」这句人话，而不是解析依赖时才抛 `feature edition2024 is required`。**MSRV 会随 `Cargo.lock` 漂移**：曾是 1.77.2，2026-09-13 最高已 1.88.0（darling 0.23 / time 0.3.55 / plist 1.10.1 / icu 2.3），40+ 包是 edition 2024（≥1.85）。**升级依赖后 CI 报 edition2024，先提 `RUST_TOOLCHAIN`。**
- **查真实 MSRV**：读 `Cargo.lock` 全部包 → 查 crates.io 稀疏索引 `https://index.crates.io/{分片}` 取每版本 `rust_version` 求最大。分片：名长 1→`1/{n}`、2→`2/{n}`、3→`3/{首字母}/{n}`、≥4→`{前2}/{次2}/{n}`。**只看 `~/.cargo/registry/src/` 会漏掉非当前平台的依赖。**
- **全绿基线**：fmt ✅ / clippy 仅 warning（**未开 `-D warnings`**，约 20+ 条历史告警）/ `Cargo test` **92 个 lib 层单测** ✅ / `npm run typecheck` ✅ / `check:boundaries` ✅ / `check:build-targets` ✅。

## 6. 打包 / 签名 / 发版铁律（2026-09-13 release 三连坑后确立）
- **本项目三平台都不签名**。**绝不要**把 secrets 直接映射成 workflow 级 `env`（`APPLE_CERTIFICATE: ${{ secrets.APPLE_CERTIFICATE }}` 这类）：**未配置的 secret 展开成空字符串而非「未设置」**，而 tauri-bundler 用 `var_os("APPLE_CERTIFICATE")` 判断是否导入证书——**没有 `is_empty()`** ⇒ 给 `security import` 喂 0 字节 p12，报 `SecKeychainItemImport: One or more parameters passed to a function were not valid.` + `failed to import keychain certificate`，macOS job 整条挂掉。将来要签名**只在 macos 的 build 步骤内按需注入**（`>> $GITHUB_ENV` 且仅当值非空）。未签名产物 macOS 首次打开需右键→打开。
- Tauri v2 **不读 `WINDOWS_CERTIFICATE`/`WINDOWS_CERTIFICATE_PASSWORD`**（实测 CLI 二进制里连字符串都没有）；Windows 签名走 `bundle.windows.certificateThumbprint` + `signCommand`。
- **MSI 必须配 `bundle.windows.wix.language = "zh-CN"`**（主因：产品名含中文）。Tauri 的 WiX 模板用 `Package/@SummaryCodepage = !(loc.TauriCodepage)`，`languages.json` 里 `en-US → 1252`、`zh-CN → 936`；默认 en-US 走 CP1252，**产品名含中文时 `light.exe` 因字符无法在 1252 表示（LGHT0311）失败**——现象是 candle 成功、light 失败、输出 `教务端_0.1.0_x64_en-US.msi` 后退出 1。三份完整配置（`tauri.conf.json`/`tauri.affairs.conf.json`/`tauri.classroom.conf.json`）均已设 `"language": "zh-CN"`。
  - 已核实：`wix314-binaries.zip` 里**无任何 `.wxl`**（只有 `WixUIExtension.dll`），Tauri 会**动态生成** `locale.wxl`（`Culture="zh-cn"` + `Codepage="936"`）经 `-loc` 给 light ⇒ **切 zh-CN 不会引出新的「缺本地化」失败**；社区同现象均以此配置解决。
- **Linux deb/rpm 包名对中文不友好（CI 不红但装不上）**：`Package:` = `heck::AsKebabCase(productName)`，heck 只切 ASCII 大小写边界 ⇒ 纯 CJK 原样保留 → `Package: 教务端`，违反 Debian（`[a-z0-9][a-z0-9+.-]*`）/RPM 包名字符集。tauri-bundler 是**纯 Rust 自己拼 .deb（ar+tar+gz）、不调 `dpkg-deb`/`rpmbuild`** ⇒ 构建期无校验，但 `dpkg -i` 会失败。`DebConfig`/`RpmConfig` **均无 name 字段**；`desktopTemplate` 可自定义（Handlebars，变量 `categories/comment/exec/icon/name`）以单独控制菜单显示名。唯一杠杆是改 `productName` 为 ASCII。
  - **`--config` 可重复且按顺序 merge**（CLI help：*Configurations are merged in the order they are provided*）⇒ 可**只在 Linux 构建时追加 ASCII `productName` 覆盖**，不动 macOS/Windows 的中文名（项目已有此用法：`tauri:dev:*` 传两个 `--config`）。
- **CI 诊断步骤不要用「管道 + head」**：GitHub 的 `shell: bash` 默认 `bash -eo pipefail`，`ls -R dir | head -60` 里 `ls` 返回非 0（GNU ls 目录不存在/条目读不动 → **2**，SIGPIPE → 141）整步判失败。写法：`ls -R dir > file || true` 落盘再 `head file` + `if: always()`。上传前加一步断言产物数 > 0，避免「job 绿了但 Release 是空的」。
- **上传 release 资源按扩展名 glob**，不要 `bundle/**/*`（后者会把解包目录与 `.app` 内部文件全当资源上传）。
- **release 触发**：`release.yml` = `tags: ["v*"]` + `workflow_dispatch`。**tag 必须带 `v` 前缀**（`v1.0.0` 可以，`1.0.0` 不触发）。救急走 Actions → Release → Run workflow。
- **`tauri build` 内部顺序**：① CLI 启动 → ② `cargo metadata`（打印 "Looking up installed tauri packages"）→ ③ `beforeBuildCommand`（= `npm run build:affairs`；本机 tsc 25.8s + vite 32s）→ ④ cargo 编译（本机全量约 5m08s）→ ⑤ 打包 .app/.dmg。② 在 ③ 之前且依赖 PATH 里有 `cargo`。
- **GitHub Actions 日志匿名取用**：公开仓库原始日志（`/actions/jobs/{id}/logs`、`/runs/{id}/logs`、`/lines`）一律 403/404；能匿名拿到的只有 `GET /repos/{o}/{r}/actions/runs/{id}/jobs`（逐 step conclusion + 起止时间）与 `check-runs/{id}/annotations`（只有 `Process completed with exit code 1.`）。**靠 step 耗时反推失败阶段最有效**（tauri build 仅 10–19s 就挂 ⇒ 必在 cargo metadata/前端构建之前）。

## 7. 本机环境坑
- **本机 zsh 非交互，`cargo` 不在 PATH**。跑 Rust 前先 `export PATH="$HOME/.cargo/bin:$PATH"`。
- **`npx tsc` 会拉到错误版本**，误报 `Cannot find module '@tauri-apps/api/core'`。**必须用 `./node_modules/.bin/tsc --noEmit`**。
- WorkBuddy 文件代理读 `node_modules/@tauri-apps/api/core.js` 会触发「敏感内容审批超时」，导致 `vite build` 在打包末期被拦截（与代码无关）。
- **本机跑 DMG 打包会被沙箱拦**：`hdiutil` 要写 `/Volumes/{productName}/…`，报 `file-write-unlink` 被拒 ⇒ 本地只能验证到 `.app`；`dangerouslyDisableSandbox` 实测不生效。
- **双实例联调（教务端⇄班级端）**：两份副本 **identifier 必须不同**（相同则共用 SQLite/设备身份/模式/端口，无法互相同步；副本改 `cn.yipaike.lanworkbench`→`…workbench.client`）。vite `strictPort:true` 且 `devUrl=1420` ⇒ 第二份改 `server.port`→1421 + `build.devUrl`→`:1421`。API 端口（`API_PORT=5178`，`net/server.rs` 探测 +20）**不用改**，自动错开 5178/5179。模式/`completed_setup`/device_id 存 DB（`app_settings`）⇒ 不同 identifier 即独立库。**两端共享密钥必须一致**。班级端首次配置时目录未同步 ⇒ 年级/班级先手填、待同步后重绑。mDNS（`_schworkbench._tcp.local.`，5353）靠回环组播；收不到目录先查两端 DeviceMonitor 互见 + 密钥一致。**当前无手动 IP:端口 添加对端的兜底 UI**。
