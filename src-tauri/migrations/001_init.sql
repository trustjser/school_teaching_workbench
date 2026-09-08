-- =============================================================================
--  局域网分布式教务与班级协同工作台  (LAN School-Affairs Workbench)
--  文件: docs/02-ddl.sql
--  目标: SQLite (通过 tauri-plugin-sql 使用，SQLite >= 3.35)
--  约定:
--    * 所有主键 id 为 TEXT(UUID v4，无连字符小写或带连字符均可，统一 36 位带连字符)
--    * 所有时间字段为 INTEGER 毫秒时间戳 (Unix epoch millis, UTC)
--    * 日期字段 checkin_date 为 TEXT 'YYYY-MM-DD'（便于按天分组与索引）
--    * 全表软删: deleted_at IS NULL 视为有效数据；所有唯一索引均为「部分索引」只约束有效行
--    * sync_state: local(仅本地) / pending(待同步) / synced(已同步) / conflict(冲突)
--    * dirty: 0=已同步快照, 1=本地已修改待推送
--    * 时间统一由 repo 层写入，不使用数据库触发器自动维护 updated_at（便于批量导入控制）
-- =============================================================================
--
--  ER 关系说明 (Mermaid erDiagram):
--
--  erDiagram
--    APP_SETTINGS   ||--o{ DEVICE : "本机身份与密钥"
--    IMPORT_BATCH   ||--o{ STUDENT : "一次导入产生多名学生"
--    STUDENT        ||--o{ CHECKIN_RECORD : "考勤记录"
--    STUDENT        ||--o{ TASK_RECORD : "任务完成记录"
--    CUSTOM_TASK    ||--o{ TASK_STATUS_NODE : "2~4 个自定义状态节点"
--    CUSTOM_TASK    ||--o{ TASK_RECORD : "任务-学生矩阵"
--    TASK_STATUS_NODE ||--o{ TASK_RECORD : "当前所处节点"
--    BROADCAST_TASK ||--o{ BROADCAST_RECEIPT : "下发回执(班级端登记)"
--    BROADCAST_TASK ||--o{ CUSTOM_TASK : "一键生成班级待办"
--    PENDING_QUEUE  ||--o{ SYNC_LOG : "每次补发产生日志"
--    DEVICE         ||--o{ PENDING_QUEUE : "目标设备"
--    OFFLINE_PACKAGE }o--o{ STUDENT : ".sch 离线包导出/导入"
--
--  文字版主外键:
--    students.import_batch_id        -> import_batches.id        (ON DELETE SET NULL)
--    checkin_records.student_id      -> students.id              (ON DELETE CASCADE)
--    task_status_nodes.task_id       -> custom_tasks.id          (ON DELETE CASCADE)
--    task_records.task_id            -> custom_tasks.id          (ON DELETE CASCADE)
--    task_records.student_id         -> students.id              (ON DELETE CASCADE)
--    custom_tasks.broadcast_task_id  -> broadcast_tasks.id       (ON DELETE SET NULL)
--    broadcast_receipts.broadcast_task_id -> broadcast_tasks.id  (ON DELETE CASCADE)
--    sync_log.queue_id               -> pending_queue.id         (ON DELETE SET NULL)
--    注: task_records.node_id 与 custom_tasks.default_node_id 有意不建外键，
--        避免与 task_status_nodes 形成循环依赖，由应用层保证一致性。
-- =============================================================================

-- =============================================================================
--  tauri-plugin-sql 迁移文件  version = 1
--  注册方式（src-tauri/src/db/migrations.rs）：
--    Migration {
--        version: 1,
--        description: "init schema",
--        sql: include_str!("../migrations/001_init.sql"),
--        kind: MigrationKind::Up,
--    }
--  注意:
--    1) 本文件内容与 docs/02-ddl.sql 完全一致，任何修改必须两处同步。
--    2) 若 sqlx 的 execute() 不支持多语句，请在 db::migrations 中按 ";\n" 拆分为
--       多条语句逐条执行（本文件不含字符串内嵌分号，拆分是安全的）。
--    3) PRAGMA foreign_keys 在事务中无效，需由 Rust 侧在建立连接时执行。
-- =============================================================================

PRAGMA foreign_keys = ON;   -- 若运行在事务内为 no-op，实际由连接参数启用

-- =============================================================================
-- 1. app_settings —— 应用配置 / 运行模式 / 密钥 (KV 存储，单实例)
-- =============================================================================
CREATE TABLE IF NOT EXISTS app_settings (
    id            TEXT    NOT NULL PRIMARY KEY,              -- UUID
    setting_key   TEXT    NOT NULL,                          -- 见下方「配置键字典」
    setting_value TEXT,                                      -- 字符串化取值（json 类型时为 JSON 文本）
    value_type    TEXT    NOT NULL DEFAULT 'string'
                          CHECK (value_type IN ('string','number','boolean','json','secret')),
    remark        TEXT,                                      -- 中文说明，展示在设置页
    created_at    INTEGER NOT NULL,
    updated_at    INTEGER NOT NULL,
    deleted_at    INTEGER,
    sync_state    TEXT    NOT NULL DEFAULT 'local'
                          CHECK (sync_state IN ('local','pending','synced','conflict')),
    dirty         INTEGER NOT NULL DEFAULT 0 CHECK (dirty IN (0, 1))
);
-- 配置键字典(setting_key):
--   app_mode             'client' | 'master'                 运行模式
--   completed_setup     是否已完成首次启动向导（前端据此跳过引导，权威键）
--   first_run_done       遗留兼容键，值与 completed_setup 保持一致
--   device_id            本机设备 UUID（全局唯一，同时用于 HMAC 身份）
--   device_name          本机显示名（如「三年级二班-讲台机」）
--   grade                所属年级（client 模式）
--   class_name           所属班级（client 模式）
--   school_name          学校名（master 模式）
--   api_port             Axum 监听端口，默认 5178
--   mdns_service_type    '_schworkbench._tcp.local.'
--   shared_secret_b64    HMAC-SHA256 共享密钥(Base64, 32B)，type=secret
--   aes_root_key_b64     AES-256-GCM 根密钥(Base64, 32B)，type=secret
--   key_id               密钥标识 kid，随信封传输，便于轮换
--   hmac_ts_window_sec   时间戳窗口，默认 300
--   heartbeat_interval   心跳周期(秒)，默认 15
--   offline_ttl_sec      设备离线判定阈值(秒)，默认 45
--   queue_max_attempts   离线队列最大重试次数，默认 5
--   ui_scale             大屏缩放 1.0 / 1.25 / 1.5
--   theme                'light' | 'dark' | 'high-contrast'
CREATE UNIQUE INDEX IF NOT EXISTS ux_app_settings_key
    ON app_settings(setting_key) WHERE deleted_at IS NULL;

-- =============================================================================
-- 2. devices —— mDNS 发现的局域网节点 + 心跳状态
-- =============================================================================
CREATE TABLE IF NOT EXISTS devices (
    id                TEXT    NOT NULL PRIMARY KEY,          -- UUID(本地主键)
    device_id         TEXT    NOT NULL,                      -- 对端设备 UUID（全局唯一，去重依据）
    device_name       TEXT    NOT NULL,                      -- 如「三年级二班」
    device_role       TEXT    NOT NULL DEFAULT 'unknown'
                              CHECK (device_role IN ('client','master','unknown')),
    ip_address        TEXT,                                  -- IPv4，如 192.168.1.23
    port              INTEGER CHECK (port IS NULL OR (port > 0 AND port <= 65535)),
    mdns_fullname     TEXT,                                  -- mDNS 完整实例名
    txt_class_name    TEXT,                                  -- TXT 记录：班级
    txt_grade         TEXT,                                  -- TXT 记录：年级
    txt_api_version   TEXT,                                  -- TXT 记录：API 版本 v1
    txt_key_id        TEXT,                                  -- TXT 记录：密钥 kid
    status            TEXT    NOT NULL DEFAULT 'offline'
                              CHECK (status IN ('online','offline','stale','blocked')),
    last_seen_at      INTEGER,                               -- 最近一次 mDNS/心跳可见时间
    last_heartbeat_at INTEGER,                               -- 最近一次 /api/v1/ping 成功时间
    last_latency_ms   INTEGER,                               -- 最近一次心跳往返耗时
    miss_count        INTEGER NOT NULL DEFAULT 0,            -- 连续心跳失败次数
    is_self           INTEGER NOT NULL DEFAULT 0 CHECK (is_self IN (0, 1)),
    created_at        INTEGER NOT NULL,
    updated_at        INTEGER NOT NULL,
    deleted_at        INTEGER,
    sync_state        TEXT    NOT NULL DEFAULT 'local'
                              CHECK (sync_state IN ('local','pending','synced','conflict')),
    dirty             INTEGER NOT NULL DEFAULT 0 CHECK (dirty IN (0, 1))
);
CREATE UNIQUE INDEX IF NOT EXISTS ux_devices_device_id
    ON devices(device_id) WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS ix_devices_status ON devices(status, last_seen_at);
CREATE INDEX IF NOT EXISTS ix_devices_role ON devices(device_role, txt_class_name);

-- =============================================================================
-- 3. import_batches —— 导入批次（xlsx / csv / sch），用于审计与回滚
-- =============================================================================
CREATE TABLE IF NOT EXISTS import_batches (
    id            TEXT    NOT NULL PRIMARY KEY,
    batch_name    TEXT    NOT NULL,                          -- 展示名，如「2026春-三年级二班名册」
    source_type   TEXT    NOT NULL DEFAULT 'xlsx'
                          CHECK (source_type IN ('xlsx','csv','sch','manual','api')),
    source_file   TEXT,                                      -- 原始文件名/路径
    total_rows    INTEGER NOT NULL DEFAULT 0 CHECK (total_rows >= 0),
    success_rows  INTEGER NOT NULL DEFAULT 0 CHECK (success_rows >= 0),
    failed_rows   INTEGER NOT NULL DEFAULT 0 CHECK (failed_rows >= 0),
    status        TEXT    NOT NULL DEFAULT 'processing'
                          CHECK (status IN ('processing','completed','failed','rolled_back')),
    error_report  TEXT,                                      -- JSON 数组: [{row, field, message}]
    imported_by   TEXT,                                      -- 操作者/设备名
    created_at    INTEGER NOT NULL,
    updated_at    INTEGER NOT NULL,
    deleted_at    INTEGER,
    sync_state    TEXT    NOT NULL DEFAULT 'local'
                          CHECK (sync_state IN ('local','pending','synced','conflict')),
    dirty         INTEGER NOT NULL DEFAULT 0 CHECK (dirty IN (0, 1))
);
CREATE INDEX IF NOT EXISTS ix_import_batches_created ON import_batches(created_at);

-- =============================================================================
-- 4. students —— 学生名册
--    status: active=在读 / leave=请假(长期) / transferred=已转出(自动排除日常统计)
-- =============================================================================
CREATE TABLE IF NOT EXISTS students (
    id              TEXT    NOT NULL PRIMARY KEY,
    student_no      TEXT    NOT NULL,                        -- 学号
    name            TEXT    NOT NULL,
    gender          TEXT    NOT NULL DEFAULT 'unknown'
                            CHECK (gender IN ('male','female','unknown')),
    grade           TEXT,                                    -- 年级，如 '3'
    class_name      TEXT,                                    -- 班级，如 '三年级二班'
    seat_no         INTEGER,                                 -- 座位号/序号，用于矩阵排序
    status          TEXT    NOT NULL DEFAULT 'active'
                            CHECK (status IN ('active','leave','transferred')),
    status_since    INTEGER,                                 -- 状态变更生效时间
    note            TEXT,
    phone           TEXT,                                    -- 家长联系电话（可选）
    import_batch_id TEXT,
    created_at      INTEGER NOT NULL,
    updated_at      INTEGER NOT NULL,
    deleted_at      INTEGER,
    sync_state      TEXT    NOT NULL DEFAULT 'pending'
                            CHECK (sync_state IN ('local','pending','synced','conflict')),
    dirty           INTEGER NOT NULL DEFAULT 1 CHECK (dirty IN (0, 1)),
    FOREIGN KEY (import_batch_id) REFERENCES import_batches(id) ON DELETE SET NULL
);
CREATE UNIQUE INDEX IF NOT EXISTS ux_students_no
    ON students(COALESCE(grade, ''), COALESCE(class_name, ''), student_no)
    WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS ix_students_class  ON students(class_name, grade, seat_no);
CREATE INDEX IF NOT EXISTS ix_students_status ON students(status);
CREATE INDEX IF NOT EXISTS ix_students_name   ON students(name);

-- =============================================================================
-- 5. checkin_records —— 考勤记录（反向标记：默认全体在读 = present，仅记录被点名的学生）
--    state: present=出勤(green) / leave=请假(amber) / absent=缺勤(red) / late=迟到(显式可选)
--    循环切换: present -> leave -> absent -> present
-- =============================================================================
CREATE TABLE IF NOT EXISTS checkin_records (
    id            TEXT    NOT NULL PRIMARY KEY,
    student_id    TEXT    NOT NULL,
    checkin_date  TEXT    NOT NULL,                          -- 'YYYY-MM-DD'
    period        TEXT    NOT NULL DEFAULT 'am'
                          CHECK (period IN ('am','pm','all','custom')),
    period_label  TEXT,                                      -- 自定义时段名，如 '第一节'
    state         TEXT    NOT NULL DEFAULT 'present'
                          CHECK (state IN ('present','leave','absent','late')),
    marked_by     TEXT,                                      -- 标记人/设备名
    marked_at     INTEGER,                                   -- 标记时间(毫秒)
    note          TEXT,
    source        TEXT    NOT NULL DEFAULT 'local'
                          CHECK (source IN ('local','api','broadcast','sch_import')),
    created_at    INTEGER NOT NULL,
    updated_at    INTEGER NOT NULL,
    deleted_at    INTEGER,
    sync_state    TEXT    NOT NULL DEFAULT 'pending'
                          CHECK (sync_state IN ('local','pending','synced','conflict')),
    dirty         INTEGER NOT NULL DEFAULT 1 CHECK (dirty IN (0, 1)),
    FOREIGN KEY (student_id) REFERENCES students(id) ON DELETE CASCADE
);
CREATE UNIQUE INDEX IF NOT EXISTS ux_checkin_unique
    ON checkin_records(student_id, checkin_date, period) WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS ix_checkin_date   ON checkin_records(checkin_date, state);
CREATE INDEX IF NOT EXISTS ix_checkin_student ON checkin_records(student_id, checkin_date);
CREATE INDEX IF NOT EXISTS ix_checkin_sync   ON checkin_records(sync_state, dirty);

-- =============================================================================
-- 6. custom_tasks —— 自定义任务（班级端自建 或 由教务处广播生成）
-- =============================================================================
CREATE TABLE IF NOT EXISTS custom_tasks (
    id                 TEXT    NOT NULL PRIMARY KEY,
    title              TEXT    NOT NULL,                     -- 如「《春晓》课文背诵」
    description        TEXT,
    task_type          TEXT    NOT NULL DEFAULT 'custom'
                               CHECK (task_type IN ('custom','recitation','homework','temperature','checkin','other')),
    scope              TEXT    NOT NULL DEFAULT 'class'
                               CHECK (scope IN ('class','grade','school')),
    grade              TEXT,
    class_name         TEXT,
    due_at             INTEGER,                              -- 截止时间戳
    status             TEXT    NOT NULL DEFAULT 'draft'
                               CHECK (status IN ('draft','active','closed','archived')),
    view_mode          TEXT    NOT NULL DEFAULT 'grid'
                               CHECK (view_mode IN ('grid','table')),
    score_enabled      INTEGER NOT NULL DEFAULT 1 CHECK (score_enabled IN (0, 1)),
    note_enabled       INTEGER NOT NULL DEFAULT 1 CHECK (note_enabled IN (0, 1)),
    default_node_id    TEXT,                                 -- 新建记录时的默认节点(task_status_nodes.id)
    owner_device_id    TEXT,                                 -- 创建方设备
    broadcast_task_id  TEXT,                                 -- 由教务处广播生成时回填
    source             TEXT    NOT NULL DEFAULT 'local'
                               CHECK (source IN ('local','broadcast','sch_import')),
    sort_order         INTEGER NOT NULL DEFAULT 0,
    created_at         INTEGER NOT NULL,
    updated_at         INTEGER NOT NULL,
    deleted_at         INTEGER,
    sync_state         TEXT    NOT NULL DEFAULT 'pending'
                               CHECK (sync_state IN ('local','pending','synced','conflict')),
    dirty              INTEGER NOT NULL DEFAULT 1 CHECK (dirty IN (0, 1)),
    FOREIGN KEY (broadcast_task_id) REFERENCES broadcast_tasks(id) ON DELETE SET NULL
);
CREATE INDEX IF NOT EXISTS ix_tasks_scope  ON custom_tasks(scope, grade, class_name, status);
CREATE INDEX IF NOT EXISTS ix_tasks_status ON custom_tasks(status, due_at);
CREATE INDEX IF NOT EXISTS ix_tasks_source ON custom_tasks(source, broadcast_task_id);

-- =============================================================================
-- 7. task_status_nodes —— 任务自定义状态节点（每个任务 2~4 个）
--    color_token 使用 Tailwind 色板名，前端映射为高对比度大屏配色
-- =============================================================================
CREATE TABLE IF NOT EXISTS task_status_nodes (
    id           TEXT    NOT NULL PRIMARY KEY,
    task_id      TEXT    NOT NULL,
    node_key     TEXT    NOT NULL,                           -- 稳定键: 'todo' / 'doing' / 'done'
    label        TEXT    NOT NULL,                           -- 显示名: '未开始' / '进行中' / '已通过'
    color_token  TEXT    NOT NULL DEFAULT 'slate'
                         CHECK (color_token IN ('slate','blue','amber','emerald','rose','violet','cyan','orange')),
    icon_name    TEXT,                                       -- lucide-react 图标名, 如 'CircleCheck'
    node_order   INTEGER NOT NULL DEFAULT 0,                 -- 循环切换顺序
    is_final     INTEGER NOT NULL DEFAULT 0 CHECK (is_final IN (0, 1)),
    is_default   INTEGER NOT NULL DEFAULT 0 CHECK (is_default IN (0, 1)),
    created_at   INTEGER NOT NULL,
    updated_at   INTEGER NOT NULL,
    deleted_at   INTEGER,
    sync_state   TEXT    NOT NULL DEFAULT 'pending'
                         CHECK (sync_state IN ('local','pending','synced','conflict')),
    dirty        INTEGER NOT NULL DEFAULT 1 CHECK (dirty IN (0, 1)),
    FOREIGN KEY (task_id) REFERENCES custom_tasks(id) ON DELETE CASCADE
);
CREATE UNIQUE INDEX IF NOT EXISTS ux_nodes_task_key
    ON task_status_nodes(task_id, node_key) WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS ix_nodes_task ON task_status_nodes(task_id, node_order);

-- 硬约束：每个任务最多 4 个状态节点（最少 2 个由应用层在「启用任务」时校验）
DROP TRIGGER IF EXISTS trg_nodes_max4;
CREATE TRIGGER IF NOT EXISTS trg_nodes_max4
BEFORE INSERT ON task_status_nodes
WHEN (SELECT COUNT(*) FROM task_status_nodes
      WHERE task_id = NEW.task_id AND deleted_at IS NULL) >= 4
BEGIN
    SELECT RAISE(ABORT, 'TASK_NODE_LIMIT: 每个任务最多 4 个状态节点');
END;

-- =============================================================================
-- 8. task_records —— 任务完成记录（任务 × 学生 矩阵单元格）
-- =============================================================================
CREATE TABLE IF NOT EXISTS task_records (
    id            TEXT    NOT NULL PRIMARY KEY,
    task_id       TEXT    NOT NULL,
    student_id    TEXT    NOT NULL,
    node_id       TEXT,                                      -- 当前状态节点 id
    node_key      TEXT    NOT NULL,                          -- 冗余节点 key，便于离线渲染
    score         INTEGER CHECK (score IS NULL OR (score >= 0 AND score <= 100)),
    note          TEXT,
    completed_at  INTEGER,                                   -- 到达 is_final 节点的时间
    evaluated_by  TEXT,
    created_at    INTEGER NOT NULL,
    updated_at    INTEGER NOT NULL,
    deleted_at    INTEGER,
    sync_state    TEXT    NOT NULL DEFAULT 'pending'
                          CHECK (sync_state IN ('local','pending','synced','conflict')),
    dirty         INTEGER NOT NULL DEFAULT 1 CHECK (dirty IN (0, 1)),
    FOREIGN KEY (task_id)    REFERENCES custom_tasks(id) ON DELETE CASCADE,
    FOREIGN KEY (student_id) REFERENCES students(id)     ON DELETE CASCADE
);
CREATE UNIQUE INDEX IF NOT EXISTS ux_task_record_unique
    ON task_records(task_id, student_id) WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS ix_task_records_node   ON task_records(task_id, node_key);
CREATE INDEX IF NOT EXISTS ix_task_records_student ON task_records(student_id);

-- =============================================================================
-- 9. broadcast_tasks —— 教务处下发的广播任务
--    direction: out=本机(教务处)发出 / in=本机(班级端)接收到的副本
-- =============================================================================
CREATE TABLE IF NOT EXISTS broadcast_tasks (
    id                  TEXT    NOT NULL PRIMARY KEY,
    title               TEXT    NOT NULL,
    description         TEXT,
    payload             TEXT    NOT NULL,                    -- JSON: 任务定义(含 status_nodes 数组)
    target_type         TEXT    NOT NULL DEFAULT 'school'
                                CHECK (target_type IN ('school','grade','class','device')),
    target_value        TEXT,                                -- JSON 数组: ["3"] / ["三年级二班"] / ["<device_id>"]
    due_at              INTEGER,
    priority            TEXT    NOT NULL DEFAULT 'normal'
                                CHECK (priority IN ('low','normal','high','urgent')),
    publisher_device_id TEXT    NOT NULL,
    publisher_name      TEXT,
    direction           TEXT    NOT NULL DEFAULT 'out'
                                CHECK (direction IN ('out','in')),
    status              TEXT    NOT NULL DEFAULT 'draft'
                                CHECK (status IN ('draft','sending','sent','partial','closed','cancelled')),
    sent_at             INTEGER,
    closed_at           INTEGER,
    expect_count        INTEGER NOT NULL DEFAULT 0 CHECK (expect_count >= 0),
    ack_count           INTEGER NOT NULL DEFAULT 0 CHECK (ack_count >= 0),
    created_at          INTEGER NOT NULL,
    updated_at          INTEGER NOT NULL,
    deleted_at          INTEGER,
    sync_state          TEXT    NOT NULL DEFAULT 'pending'
                                CHECK (sync_state IN ('local','pending','synced','conflict')),
    dirty               INTEGER NOT NULL DEFAULT 1 CHECK (dirty IN (0, 1))
);
CREATE INDEX IF NOT EXISTS ix_broadcast_status ON broadcast_tasks(status, created_at);
CREATE INDEX IF NOT EXISTS ix_broadcast_dir    ON broadcast_tasks(direction, created_at);
CREATE INDEX IF NOT EXISTS ix_broadcast_pub    ON broadcast_tasks(publisher_device_id);

-- =============================================================================
-- 10. broadcast_receipts —— 广播任务接收/登记回执（教务处端汇总，班级端登记）
-- =============================================================================
CREATE TABLE IF NOT EXISTS broadcast_receipts (
    id                TEXT    NOT NULL PRIMARY KEY,
    broadcast_task_id TEXT    NOT NULL,
    device_id         TEXT    NOT NULL,                      -- 班级端设备 UUID
    device_name       TEXT,
    class_name        TEXT,
    status            TEXT    NOT NULL DEFAULT 'received'
                              CHECK (status IN ('received','accepted','rejected','done')),
    received_at       INTEGER,                               -- 班级端收到时间
    accepted_at       INTEGER,                               -- 一键生成待办时间
    local_task_id     TEXT,                                  -- 生成后的本地 custom_tasks.id
    fail_reason       TEXT,
    created_at        INTEGER NOT NULL,
    updated_at        INTEGER NOT NULL,
    deleted_at        INTEGER,
    sync_state        TEXT    NOT NULL DEFAULT 'pending'
                              CHECK (sync_state IN ('local','pending','synced','conflict')),
    dirty             INTEGER NOT NULL DEFAULT 1 CHECK (dirty IN (0, 1)),
    FOREIGN KEY (broadcast_task_id) REFERENCES broadcast_tasks(id) ON DELETE CASCADE
);
CREATE UNIQUE INDEX IF NOT EXISTS ux_receipt_unique
    ON broadcast_receipts(broadcast_task_id, device_id) WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS ix_receipt_task ON broadcast_receipts(broadcast_task_id, status);

-- =============================================================================
-- 11. pending_queue —— 离线待发送队列 (Outbox)
--     同一 (entity_type, entity_id, op_type, target_device_id) 只保留一条待发记录，
--     后续变更直接覆盖 payload，实现「增量合并」，避免网络恢复后重复补发。
-- =============================================================================
CREATE TABLE IF NOT EXISTS pending_queue (
    id               TEXT    NOT NULL PRIMARY KEY,
    op_type          TEXT    NOT NULL CHECK (op_type IN ('upsert','delete','ack','heartbeat','broadcast')),
    entity_type      TEXT    NOT NULL CHECK (entity_type IN
                     ('student','checkin','custom_task','task_node','task_record',
                      'broadcast_task','receipt','device','grade','class')),
    entity_id        TEXT    NOT NULL,
    payload          TEXT    NOT NULL,                       -- JSON: 实体增量快照
    target_device_id TEXT,                                   -- 为空表示发给所有已知 master
    target_endpoint  TEXT    NOT NULL DEFAULT '/api/v1/ingest',
    target_base_url  TEXT,                                   -- 运行时解析: http://192.168.1.10:5178
    attempt_count    INTEGER NOT NULL DEFAULT 0 CHECK (attempt_count >= 0),
    max_attempts     INTEGER NOT NULL DEFAULT 5 CHECK (max_attempts > 0),
    next_retry_at    INTEGER NOT NULL DEFAULT 0,             -- 退避后的下次尝试时间
    last_error       TEXT,
    status           TEXT    NOT NULL DEFAULT 'pending'
                             CHECK (status IN ('pending','sending','done','failed','dead')),
    priority         INTEGER NOT NULL DEFAULT 5,             -- 1=最高(考勤) 5=普通 9=最低
    batch_id         TEXT,                                   -- 同一次批量操作归并
    created_at       INTEGER NOT NULL,
    updated_at       INTEGER NOT NULL,
    deleted_at       INTEGER,
    sync_state       TEXT    NOT NULL DEFAULT 'pending'
                             CHECK (sync_state IN ('local','pending','synced','conflict')),
    dirty            INTEGER NOT NULL DEFAULT 1 CHECK (dirty IN (0, 1))
);
CREATE UNIQUE INDEX IF NOT EXISTS ux_queue_dedup
    ON pending_queue(entity_type, entity_id, op_type, COALESCE(target_device_id, '*'))
    WHERE deleted_at IS NULL AND status IN ('pending','sending');
CREATE INDEX IF NOT EXISTS ix_queue_due    ON pending_queue(status, next_retry_at, priority);
CREATE INDEX IF NOT EXISTS ix_queue_entity ON pending_queue(entity_type, entity_id);

-- =============================================================================
-- 12. sync_log —— 同步/请求日志（排障与审计）
-- =============================================================================
CREATE TABLE IF NOT EXISTS sync_log (
    id             TEXT    NOT NULL PRIMARY KEY,
    direction      TEXT    NOT NULL CHECK (direction IN ('out','in')),
    peer_device_id TEXT,
    peer_name      TEXT,
    endpoint       TEXT,
    entity_type    TEXT,
    entity_count   INTEGER NOT NULL DEFAULT 0 CHECK (entity_count >= 0),
    result         TEXT    NOT NULL CHECK (result IN ('success','failed','partial','rejected')),
    http_status    INTEGER,
    error_code     TEXT,                                     -- 见 03-tasks.md 错误码表
    error_message  TEXT,
    duration_ms    INTEGER,
    queue_id       TEXT,
    trace_id       TEXT,                                     -- 与信封 nonce 对齐，便于端到端追踪
    created_at     INTEGER NOT NULL,
    updated_at     INTEGER NOT NULL,
    deleted_at     INTEGER,
    sync_state     TEXT    NOT NULL DEFAULT 'local'
                           CHECK (sync_state IN ('local','pending','synced','conflict')),
    dirty          INTEGER NOT NULL DEFAULT 0 CHECK (dirty IN (0, 1)),
    FOREIGN KEY (queue_id) REFERENCES pending_queue(id) ON DELETE SET NULL
);
CREATE INDEX IF NOT EXISTS ix_sync_log_created ON sync_log(created_at);
CREATE INDEX IF NOT EXISTS ix_sync_log_peer    ON sync_log(peer_device_id, created_at);
CREATE INDEX IF NOT EXISTS ix_sync_log_result  ON sync_log(result, created_at);

-- =============================================================================
-- 13. offline_packages —— .sch 离线数据包导出/导入审计
-- =============================================================================
CREATE TABLE IF NOT EXISTS offline_packages (
    id            TEXT    NOT NULL PRIMARY KEY,
    file_name     TEXT    NOT NULL,                          -- 如 2026-09-08_三年级二班_delta.sch
    file_path     TEXT,
    direction     TEXT    NOT NULL CHECK (direction IN ('export','import')),
    package_type  TEXT    NOT NULL DEFAULT 'delta'
                          CHECK (package_type IN ('full','delta')),
    scope         TEXT,                                      -- class / grade / school
    entity_counts TEXT,                                      -- JSON: {"student":42,"checkin":120}
    checksum      TEXT,                                      -- SHA-256(hex)
    size_bytes    INTEGER NOT NULL DEFAULT 0 CHECK (size_bytes >= 0),
    status        TEXT    NOT NULL DEFAULT 'pending'
                          CHECK (status IN ('pending','done','failed','verified')),
    since_ts      INTEGER,                                   -- delta 起点
    until_ts      INTEGER,                                   -- delta 终点
    created_at    INTEGER NOT NULL,
    updated_at    INTEGER NOT NULL,
    deleted_at    INTEGER,
    sync_state    TEXT    NOT NULL DEFAULT 'local'
                          CHECK (sync_state IN ('local','pending','synced','conflict')),
    dirty         INTEGER NOT NULL DEFAULT 0 CHECK (dirty IN (0, 1))
);
CREATE INDEX IF NOT EXISTS ix_packages_created ON offline_packages(created_at, direction);

-- =============================================================================
-- 14. 便捷视图（可选，供大屏聚合查询直接复用）
-- =============================================================================
DROP VIEW IF EXISTS v_active_students;
CREATE VIEW IF NOT EXISTS v_active_students AS
    SELECT * FROM students WHERE deleted_at IS NULL AND status <> 'transferred';

DROP VIEW IF EXISTS v_daily_checkin_summary;
CREATE VIEW IF NOT EXISTS v_daily_checkin_summary AS
    SELECT
        s.grade                                                        AS grade,
        s.class_name                                                   AS class_name,
        c.checkin_date                                                 AS checkin_date,
        c.period                                                       AS period,
        SUM(CASE WHEN c.state = 'present' THEN 1 ELSE 0 END)           AS present_cnt,
        SUM(CASE WHEN c.state = 'leave'   THEN 1 ELSE 0 END)           AS leave_cnt,
        SUM(CASE WHEN c.state = 'absent'  THEN 1 ELSE 0 END)           AS absent_cnt,
        SUM(CASE WHEN c.state = 'late'    THEN 1 ELSE 0 END)           AS late_cnt,
        COUNT(*)                                                       AS marked_cnt
    FROM checkin_records c
    JOIN students s ON s.id = c.student_id
    WHERE c.deleted_at IS NULL AND s.deleted_at IS NULL
    GROUP BY s.grade, s.class_name, c.checkin_date, c.period;

-- =============================================================================
-- 初始化默认配置（首次启动向导会覆盖）
-- =============================================================================
INSERT OR IGNORE INTO app_settings (id, setting_key, setting_value, value_type, remark, created_at, updated_at)
VALUES
 ('00000000-0000-4000-8000-000000000001','app_mode','client','string','运行模式: client=班级端 / master=教务处端', 0, 0),
 ('00000000-0000-4000-8000-000000000002','first_run_done','false','boolean','是否已完成首次启动向导', 0, 0),
 ('00000000-0000-4000-8000-000000000003','api_port','5178','number','Axum 本地 API 监听端口', 0, 0),
 ('00000000-0000-4000-8000-000000000004','mdns_service_type','_schworkbench._tcp.local.','string','mDNS 服务类型', 0, 0),
 ('00000000-0000-4000-8000-000000000005','hmac_ts_window_sec','300','number','HMAC 时间戳容差窗口(秒)', 0, 0),
 ('00000000-0000-4000-8000-000000000006','heartbeat_interval','15','number','心跳周期(秒)', 0, 0),
 ('00000000-0000-4000-8000-000000000007','offline_ttl_sec','45','number','判定设备离线的连续静默时长(秒)', 0, 0),
 ('00000000-0000-4000-8000-000000000008','queue_max_attempts','5','number','离线队列最大重试次数', 0, 0),
 ('00000000-0000-4000-8000-000000000009','ui_scale','1.25','number','大屏 UI 缩放', 0, 0),
 ('00000000-0000-4000-8000-00000000000a','theme','light','string','主题: light/dark/high-contrast', 0, 0);
