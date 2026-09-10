//! Rust 领域结构体：与前端 `src/types/models.ts` 一一对应。
//! 全部字段使用 snake_case 命名（与 SQLite 列名一致），由 serde `rename_all = "camelCase"`
//! 统一转换为前端所需的 camelCase JSON。

use serde::{Deserialize, Serialize};

// =============================================================================
// 枚举
// =============================================================================

/// 应用运行模式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AppMode {
    /// 班级端
    Client,
    /// 教务处端
    Master,
}

impl AppMode {
    /// 数据库 / 协议字符串。
    pub fn as_str(&self) -> &'static str {
        match self {
            AppMode::Client => "client",
            AppMode::Master => "master",
        }
    }

    /// 由字符串解析，未知回退 `Client`。
    pub fn parse(value: &str) -> AppMode {
        match value {
            "master" => AppMode::Master,
            _ => AppMode::Client,
        }
    }
}

impl Default for AppMode {
    fn default() -> AppMode {
        AppMode::Client
    }
}

/// 学生状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StudentStatus {
    /// 在读
    Active,
    /// 请假（长期）
    Leave,
    /// 已转出（自动排除日常统计）
    Transferred,
}

impl StudentStatus {
    /// 协议字符串。
    pub fn as_str(&self) -> &'static str {
        match self {
            StudentStatus::Active => "active",
            StudentStatus::Leave => "leave",
            StudentStatus::Transferred => "transferred",
        }
    }

    /// 由字符串解析，未知回退 `Active`。
    pub fn parse(value: &str) -> StudentStatus {
        match value {
            "leave" => StudentStatus::Leave,
            "transferred" => StudentStatus::Transferred,
            _ => StudentStatus::Active,
        }
    }
}

impl Default for StudentStatus {
    fn default() -> StudentStatus {
        StudentStatus::Active
    }
}

/// 考勤状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CheckinState {
    /// 出勤
    Present,
    /// 请假
    Leave,
    /// 缺勤
    Absent,
    /// 迟到
    Late,
}

impl CheckinState {
    /// 协议字符串。
    pub fn as_str(&self) -> &'static str {
        match self {
            CheckinState::Present => "present",
            CheckinState::Leave => "leave",
            CheckinState::Absent => "absent",
            CheckinState::Late => "late",
        }
    }

    /// 循环顺序的下一个状态：`present → leave → absent → present`。
    pub fn next(&self) -> CheckinState {
        match self {
            CheckinState::Present => CheckinState::Leave,
            CheckinState::Leave => CheckinState::Absent,
            CheckinState::Absent => CheckinState::Present,
            CheckinState::Late => CheckinState::Present,
        }
    }

    /// 由字符串解析，未知回退 `Present`。
    pub fn parse(value: &str) -> CheckinState {
        match value {
            "leave" => CheckinState::Leave,
            "absent" => CheckinState::Absent,
            "late" => CheckinState::Late,
            _ => CheckinState::Present,
        }
    }
}

impl Default for CheckinState {
    fn default() -> CheckinState {
        CheckinState::Present
    }
}

/// 同步状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SyncState {
    /// 仅本地
    Local,
    /// 待同步
    Pending,
    /// 已同步
    Synced,
    /// 冲突（last-write-wins 后保留标记）
    Conflict,
}

impl SyncState {
    /// 协议字符串。
    pub fn as_str(&self) -> &'static str {
        match self {
            SyncState::Local => "local",
            SyncState::Pending => "pending",
            SyncState::Synced => "synced",
            SyncState::Conflict => "conflict",
        }
    }

    /// 由字符串解析，未知回退 `Local`。
    pub fn parse(value: &str) -> SyncState {
        match value {
            "pending" => SyncState::Pending,
            "synced" => SyncState::Synced,
            "conflict" => SyncState::Conflict,
            _ => SyncState::Local,
        }
    }
}

impl Default for SyncState {
    fn default() -> SyncState {
        SyncState::Local
    }
}

// =============================================================================
// 领域实体
// =============================================================================

/// 应用配置项（`app_settings`）。
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct AppSetting {
    /// 主键 UUID。
    pub id: String,
    /// 配置键。
    pub setting_key: String,
    /// 字符串化取值。
    pub setting_value: Option<String>,
    /// 值类型：string / number / boolean / json / secret。
    pub value_type: String,
    /// 中文说明。
    pub remark: Option<String>,
    /// 创建时间（毫秒）。
    pub created_at: i64,
    /// 更新时间（毫秒）。
    pub updated_at: i64,
    /// 软删时间（毫秒）。
    pub deleted_at: Option<i64>,
}

/// 局域网节点（`devices`）。
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Device {
    /// 本地主键 UUID。
    pub id: String,
    /// 对端设备 UUID（全局唯一）。
    pub device_id: String,
    /// 显示名。
    pub device_name: String,
    /// 角色：client / master / unknown。
    pub device_role: String,
    /// IPv4 地址。
    pub ip_address: Option<String>,
    /// 端口。
    pub port: Option<i32>,
    /// mDNS 完整实例名。
    pub mdns_fullname: Option<String>,
    /// TXT：班级。
    pub txt_class_name: Option<String>,
    /// TXT：年级。
    pub txt_grade: Option<String>,
    /// TXT：API 版本。
    pub txt_api_version: Option<String>,
    /// TXT：密钥 kid。
    pub txt_key_id: Option<String>,
    /// 状态：online / offline / stale / blocked。
    pub status: String,
    /// 最近可见时间。
    pub last_seen_at: Option<i64>,
    /// 最近心跳成功时间。
    pub last_heartbeat_at: Option<i64>,
    /// 最近心跳往返耗时。
    pub last_latency_ms: Option<i64>,
    /// 连续心跳失败次数。
    pub miss_count: i32,
    /// 是否为本机。
    pub is_self: bool,
    /// 创建时间。
    pub created_at: i64,
    /// 更新时间。
    pub updated_at: i64,
    /// 软删时间。
    pub deleted_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Classroom {
    pub id: String,
    pub room_name: String,
    pub device_id: Option<String>,
    pub remark: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub deleted_at: Option<i64>,
    pub sync_state: String,
    pub dirty: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct ClassroomAssignment {
    pub id: String,
    pub classroom_id: String,
    pub school_year_id: String,
    pub class_id: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub deleted_at: Option<i64>,
    pub sync_state: String,
    pub dirty: bool,
}

/// 导入批次（`import_batches`）。
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct ImportBatch {
    /// 主键 UUID。
    pub id: String,
    /// 展示名。
    pub batch_name: String,
    /// 来源类型：xlsx / csv / sch / manual / api。
    pub source_type: String,
    /// 原始文件名。
    pub source_file: Option<String>,
    /// 总行数。
    pub total_rows: i64,
    /// 成功行数。
    pub success_rows: i64,
    /// 失败行数。
    pub failed_rows: i64,
    /// 状态：processing / completed / failed / rolled_back。
    pub status: String,
    /// 错误报告 JSON 数组。
    pub error_report: Option<String>,
    /// 操作者。
    pub imported_by: Option<String>,
    /// 创建时间。
    pub created_at: i64,
    /// 更新时间。
    pub updated_at: i64,
    /// 软删时间。
    pub deleted_at: Option<i64>,
}

/// 学生名册（`students`）。
///
/// 前端按 `Partial<Student>` 提交（新建时没有 id/createdAt/syncState 等），
/// 因此整结构体开启 `serde(default)`，缺失字段取默认值，由 repo 层补全。
#[derive(Debug, Clone, Default, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase", default)]
pub struct Student {
    /// 主键 UUID。
    pub id: String,
    /// 学号。
    pub student_no: String,
    /// 姓名。
    pub name: String,
    /// 性别：male / female / unknown。
    pub gender: String,
    /// 年级。
    pub grade: Option<String>,
    /// 班级。
    pub class_name: Option<String>,
    /// 关联 classes.id（目录统一维护后落位；为空时回退用 class_name 匹配）。
    pub class_id: Option<String>,
    /// 座位号。
    pub seat_no: Option<i64>,
    /// 状态：active / leave / transferred。
    pub status: String,
    /// 状态变更生效时间。
    pub status_since: Option<i64>,
    /// 备注。
    pub note: Option<String>,
    /// 家长联系电话。
    pub phone: Option<String>,
    /// 导入批次 ID。
    pub import_batch_id: Option<String>,
    /// 创建时间。
    pub created_at: i64,
    /// 更新时间。
    pub updated_at: i64,
    /// 软删时间。
    pub deleted_at: Option<i64>,
    /// 同步状态。
    pub sync_state: String,
    /// 是否待推送。
    pub dirty: bool,
}

/// 年级（`grades`）。
///
/// 教务端统一维护，全校唯一。前端按 `Partial<Grade>` 提交（新建时无 id/时间戳/syncState），
/// 因此整结构体开启 `serde(default)`，由 repo 层补全缺失字段。
#[derive(Debug, Clone, Default, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase", default)]
pub struct Grade {
    /// 主键 UUID。
    pub id: String,
    /// 年级编号，如 '3' / '2023'。
    pub grade_no: String,
    /// 展示名，如 '三年级'。
    pub grade_name: String,
    /// 排序（数字越小越靠前）。
    pub sort_order: i64,
    /// 备注。
    pub remark: Option<String>,
    /// 创建时间（毫秒）。
    pub created_at: i64,
    /// 更新时间（毫秒）。
    pub updated_at: i64,
    /// 软删时间（毫秒）。
    pub deleted_at: Option<i64>,
    /// 同步状态。
    pub sync_state: String,
    /// 是否待推送。
    pub dirty: bool,
}

/// 班级（`classes`）。
///
/// 归属某个年级，教务端统一维护；`grade_no` / `grade_name` 冗余存储便于聚合免 join。
/// 前端按 `Partial<Class>` 提交，见 `Grade` 说明。
#[derive(Debug, Clone, Default, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase", default)]
pub struct Class {
    /// 主键 UUID。
    pub id: String,
    /// 关联 grades.id（软删时置空）。
    pub grade_id: Option<String>,
    /// 关联 school_years.id（年隔离维度；为空表示未归入具体学年）。
    pub school_year_id: Option<String>,
    /// 冗余：年级编号。
    pub grade_no: Option<String>,
    /// 冗余：年级展示名。
    pub grade_name: Option<String>,
    /// 班号，如 '2'。
    pub class_no: Option<String>,
    /// 展示名，如 '三年级二班'。
    pub class_name: String,
    /// 班主任。
    pub head_teacher: Option<String>,
    /// 班级排序。
    pub sort_order: i64,
    /// 备注。
    pub remark: Option<String>,
    /// 创建时间（毫秒）。
    pub created_at: i64,
    /// 更新时间（毫秒）。
    pub updated_at: i64,
    /// 软删时间（毫秒）。
    pub deleted_at: Option<i64>,
    /// 同步状态。
    pub sync_state: String,
    /// 是否待推送。
    pub dirty: bool,
}

/// 学年 / 届（`school_years`）。
///
/// 物理机房 / 设备永久不变（device_id 不变），学年只是时间维度：
/// 同一教室每学年的班级人员、班主任都不同，但旧数据必须保留。
/// 班级真正身份 = (school_year_id, grade_id, class_no)，class_name 退为展示名。
/// 教务端统一维护，经离线队列同步到班级端（entity_type = 'school_year'）。
/// 前端按 `Partial<SchoolYear>` 提交，见 `Grade` 说明。
#[derive(Debug, Clone, Default, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase", default)]
pub struct SchoolYear {
    /// 主键 UUID。
    pub id: String,
    /// 届号，如 '2027'。
    pub school_year_no: String,
    /// 展示名，如 '2027届'。
    pub school_year_name: String,
    /// 开学日期 `YYYY-MM-DD`。
    pub start_date: Option<String>,
    /// 结束日期 `YYYY-MM-DD`。
    pub end_date: Option<String>,
    /// 排序（数字越小越靠前）。
    pub sort_order: i64,
    /// 备注。
    pub remark: Option<String>,
    /// 创建时间（毫秒）。
    pub created_at: i64,
    /// 更新时间（毫秒）。
    pub updated_at: i64,
    /// 软删时间（毫秒）。
    pub deleted_at: Option<i64>,
    /// 同步状态。
    pub sync_state: String,
    /// 是否待推送。
    pub dirty: bool,
}

/// 考勤记录（`checkin_records`）。
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct CheckinRecord {
    /// 主键 UUID。
    pub id: String,
    /// 学生 ID。
    pub student_id: String,
    /// 日期 `YYYY-MM-DD`。
    pub checkin_date: String,
    /// 时段：am / pm / all / custom。
    pub period: String,
    /// 自定义时段名。
    pub period_label: Option<String>,
    /// 状态：present / leave / absent / late。
    pub state: String,
    /// 标记人。
    pub marked_by: Option<String>,
    /// 标记时间。
    pub marked_at: Option<i64>,
    /// 备注。
    pub note: Option<String>,
    /// 来源：local / api / broadcast / sch_import。
    pub source: String,
    /// 创建时间。
    pub created_at: i64,
    /// 更新时间。
    pub updated_at: i64,
    /// 软删时间。
    pub deleted_at: Option<i64>,
    /// 同步状态。
    pub sync_state: String,
    /// 是否待推送。
    pub dirty: bool,
}

/// 自定义任务（`custom_tasks`）。前端按 `Partial<CustomTask>` 提交，见 `Student` 说明。
#[derive(Debug, Clone, Default, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase", default)]
pub struct CustomTask {
    /// 主键 UUID。
    pub id: String,
    /// 标题。
    pub title: String,
    /// 描述。
    pub description: Option<String>,
    /// 任务类型。
    pub task_type: String,
    /// 范围：class / grade / school。
    pub scope: String,
    /// 年级。
    pub grade: Option<String>,
    /// 班级。
    pub class_name: Option<String>,
    /// 截止时间。
    pub due_at: Option<i64>,
    /// 状态：draft / active / closed / archived。
    pub status: String,
    /// 视图：grid / table。
    pub view_mode: String,
    /// 是否启用评分。
    pub score_enabled: bool,
    /// 是否启用备注。
    pub note_enabled: bool,
    /// 默认节点 ID。
    pub default_node_id: Option<String>,
    /// 创建方设备。
    pub owner_device_id: Option<String>,
    /// 来源广播任务 ID。
    pub broadcast_task_id: Option<String>,
    /// 来源：local / broadcast / sch_import。
    pub source: String,
    /// 排序值。
    pub sort_order: i64,
    /// 创建时间。
    pub created_at: i64,
    /// 更新时间。
    pub updated_at: i64,
    /// 软删时间。
    pub deleted_at: Option<i64>,
    /// 同步状态。
    pub sync_state: String,
    /// 是否待推送。
    pub dirty: bool,
}

/// 任务状态节点（`task_status_nodes`）。前端按 `Partial<TaskStatusNode>` 提交，见 `Student` 说明。
#[derive(Debug, Clone, Default, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase", default)]
pub struct TaskStatusNode {
    /// 主键 UUID。
    pub id: String,
    /// 任务 ID。
    pub task_id: String,
    /// 稳定键，如 `todo`。
    pub node_key: String,
    /// 显示名。
    pub label: String,
    /// 颜色 token。
    pub color_token: String,
    /// 图标名。
    pub icon_name: Option<String>,
    /// 循环切换顺序。
    pub node_order: i32,
    /// 是否终态。
    pub is_final: bool,
    /// 是否默认节点。
    pub is_default: bool,
    /// 创建时间。
    pub created_at: i64,
    /// 更新时间。
    pub updated_at: i64,
    /// 软删时间。
    pub deleted_at: Option<i64>,
    /// 同步状态。
    pub sync_state: String,
    /// 是否待推送。
    pub dirty: bool,
}

/// 任务-学生矩阵单元格（`task_records`）。
/// 前端新建单元格时显式传 `id: undefined`，必须允许缺字段，见 `Student` 说明。
#[derive(Debug, Clone, Default, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase", default)]
pub struct TaskRecord {
    /// 主键 UUID。
    pub id: String,
    /// 任务 ID。
    pub task_id: String,
    /// 学生 ID。
    pub student_id: String,
    /// 当前节点 ID。
    pub node_id: Option<String>,
    /// 冗余节点 key。
    pub node_key: String,
    /// 评分 0–100。
    pub score: Option<i32>,
    /// 备注。
    pub note: Option<String>,
    /// 到达终态时间。
    pub completed_at: Option<i64>,
    /// 评价人。
    pub evaluated_by: Option<String>,
    /// 创建时间。
    pub created_at: i64,
    /// 更新时间。
    pub updated_at: i64,
    /// 软删时间。
    pub deleted_at: Option<i64>,
    /// 同步状态。
    pub sync_state: String,
    /// 是否待推送。
    pub dirty: bool,
}

/// 广播任务（`broadcast_tasks`）。前端按 `Partial<BroadcastTask>` 提交，见 `Student` 说明。
#[derive(Debug, Clone, Default, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase", default)]
pub struct BroadcastTask {
    /// 主键 UUID。
    pub id: String,
    /// 标题。
    pub title: String,
    /// 描述。
    pub description: Option<String>,
    /// 任务定义 JSON（含 status_nodes 数组）。
    pub payload: String,
    /// 目标类型：school / grade / class / device。
    pub target_type: String,
    /// 目标值 JSON 数组。
    pub target_value: Option<String>,
    /// 截止时间。
    pub due_at: Option<i64>,
    /// 优先级：low / normal / high / urgent。
    pub priority: String,
    /// 发布方设备 ID。
    pub publisher_device_id: String,
    /// 发布方名称。
    pub publisher_name: Option<String>,
    /// 方向：out / in。
    pub direction: String,
    /// 状态：draft / sending / sent / partial / closed / cancelled。
    pub status: String,    /// 发送时间。
    pub sent_at: Option<i64>,
    /// 关闭时间。
    pub closed_at: Option<i64>,
    /// 预期回执数。
    pub expect_count: i64,
    /// 已回执数。
    pub ack_count: i64,
    /// 创建时间。
    pub created_at: i64,
    /// 更新时间。
    pub updated_at: i64,
    /// 软删时间。
    pub deleted_at: Option<i64>,
    /// **派生字段，非表列**：是否已成功投递给至少一个班级端。
    ///
    /// 由查询用 `EXISTS(... pending_queue.status='done')` 算出。界面据此决定给
    /// 「取消」（尚未送达，可撤回）还是「关闭」（已送达，只能结束）。
    /// `upsert` 的列清单不含它；远端推送缺该字段时由 `serde(default)` 兜底。
    pub delivered: bool,
}

/// 广播回执（`broadcast_receipts`）。
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct BroadcastReceipt {
    /// 主键 UUID。
    pub id: String,
    /// 广播任务 ID。
    pub broadcast_task_id: String,
    /// 班级端设备 ID。
    pub device_id: String,
    /// 设备名。
    pub device_name: Option<String>,
    /// 班级名。
    pub class_name: Option<String>,
    /// 状态：received / accepted / rejected / done。
    pub status: String,
    /// 收到时间。
    pub received_at: Option<i64>,
    /// 一键生成待办时间。
    pub accepted_at: Option<i64>,
    /// 生成的本地任务 ID。
    pub local_task_id: Option<String>,
    /// 失败原因。
    pub fail_reason: Option<String>,
    /// 创建时间。
    pub created_at: i64,
    /// 更新时间。
    pub updated_at: i64,
    /// 软删时间。
    pub deleted_at: Option<i64>,
}

/// 离线待发队列条目（`pending_queue`）。
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct PendingQueueItem {
    /// 主键 UUID。
    pub id: String,
    /// 操作类型：upsert / delete / ack / heartbeat / broadcast。
    pub op_type: String,
    /// 实体类型。
    pub entity_type: String,
    /// 实体 ID。
    pub entity_id: String,
    /// 增量快照 JSON。
    pub payload: String,
    /// 目标设备 ID（空表示全部已知 master）。
    pub target_device_id: Option<String>,
    /// 目标端点。
    pub target_endpoint: String,
    /// 目标基址。
    pub target_base_url: Option<String>,
    /// 已尝试次数。
    pub attempt_count: i32,
    /// 最大尝试次数。
    pub max_attempts: i32,
    /// 下次重试时间。
    pub next_retry_at: i64,
    /// 最近错误码。
    pub last_error: Option<String>,
    /// 状态：pending / sending / done / failed / dead。
    pub status: String,
    /// 优先级（1 最高）。
    pub priority: i32,
    /// 批量归并 ID。
    pub batch_id: Option<String>,
    /// 创建时间。
    pub created_at: i64,
    /// 更新时间。
    pub updated_at: i64,
    /// 软删时间。
    pub deleted_at: Option<i64>,
}

/// 同步日志（`sync_log`）。
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct SyncLogEntry {
    /// 主键 UUID。
    pub id: String,
    /// 方向：out / in。
    pub direction: String,
    /// 对端设备 ID。
    pub peer_device_id: Option<String>,
    /// 对端名称。
    pub peer_name: Option<String>,
    /// 端点。
    pub endpoint: Option<String>,
    /// 实体类型。
    pub entity_type: Option<String>,
    /// 实体数量。
    pub entity_count: i64,
    /// 结果：success / failed / partial / rejected。
    pub result: String,
    /// HTTP 状态码。
    pub http_status: Option<i64>,
    /// 错误码。
    pub error_code: Option<String>,
    /// 错误信息。
    pub error_message: Option<String>,
    /// 耗时（毫秒）。
    pub duration_ms: Option<i64>,
    /// 关联队列条目。
    pub queue_id: Option<String>,
    /// 追踪 ID（等于信封 nonce）。
    pub trace_id: Option<String>,
    /// 创建时间。
    pub created_at: i64,
    /// 更新时间。
    pub updated_at: i64,
    /// 软删时间。
    pub deleted_at: Option<i64>,
}

/// 离线包审计记录（`offline_packages`）。
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct OfflinePackage {
    /// 主键 UUID。
    pub id: String,
    /// 文件名。
    pub file_name: String,
    /// 文件路径。
    pub file_path: Option<String>,
    /// 方向：export / import。
    pub direction: String,
    /// 类型：full / delta。
    pub package_type: String,
    /// 范围。
    pub scope: Option<String>,
    /// 实体计数 JSON。
    pub entity_counts: Option<String>,
    /// 校验和（SHA-256 hex）。
    pub checksum: Option<String>,
    /// 文件大小（字节）。
    pub size_bytes: i64,
    /// 状态：pending / done / failed / verified。
    pub status: String,
    /// delta 起点。
    pub since_ts: Option<i64>,
    /// delta 终点。
    pub until_ts: Option<i64>,
    /// 创建时间。
    pub created_at: i64,
    /// 更新时间。
    pub updated_at: i64,
    /// 软删时间。
    pub deleted_at: Option<i64>,
}

// =============================================================================
// 传输 / 视图 DTO
// =============================================================================

/// 行级导入错误。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RowError {
    /// 行号（1 起，含表头时为数据行序号）。
    pub row: usize,
    /// 出错字段。
    pub field: String,
    /// 错误说明。
    pub message: String,
}

/// 导入结果报告（名册 / `.sch` 共用）。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportReport {
    /// 批次 ID。
    pub batch_id: String,
    /// 总行数。
    pub total_rows: i64,
    /// 成功行数。
    pub success_rows: i64,
    /// 失败行数。
    pub failed_rows: i64,
    /// 冲突行数（last-write-wins 覆盖）。
    pub conflict_rows: i64,
    /// 行级错误明细。
    pub errors: Vec<RowError>,
    /// 是否整体成功。
    pub ok: bool,
    /// 人类可读摘要。
    pub message: String,
}

/// 名册导入行（前端解析后传入，或 Rust 解析 xlsx/csv 后产生）。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StudentImportRow {
    /// 学号。
    pub student_no: String,
    /// 姓名。
    pub name: String,
    /// 性别。
    pub gender: Option<String>,
    /// 年级。
    pub grade: Option<String>,
    /// 班级。
    pub class_name: Option<String>,
    /// 关联 classes.id（导入时若已绑定班级则落位）。
    pub class_id: Option<String>,
    /// 座位号。
    pub seat_no: Option<i64>,
    /// 联系电话。
    pub phone: Option<String>,
    /// 备注。
    pub note: Option<String>,
}

/// 日考勤汇总。
#[derive(Debug, Clone, Default, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct DailySummary {
    /// 年级。
    pub grade: Option<String>,
    /// 班级。
    pub class_name: Option<String>,
    /// 日期。
    pub checkin_date: String,
    /// 时段。
    pub period: String,
    /// 出勤数。
    pub present_cnt: i64,
    /// 请假数。
    pub leave_cnt: i64,
    /// 缺勤数。
    pub absent_cnt: i64,
    /// 迟到数。
    pub late_cnt: i64,
    /// 已标记总数。
    pub marked_cnt: i64,
    /// 在读总人数（含未标记的「默认出勤」）。
    pub total_cnt: i64,
}

/// 矩阵视图中的学生条目。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskMatrixStudent {
    /// 学生 ID。
    pub student_id: String,
    /// 姓名。
    pub name: String,
    /// 学号。
    pub student_no: String,
    /// 座位号。
    pub seat_no: Option<i64>,
    /// 状态。
    pub status: String,
}

/// 矩阵视图中的记录条目。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskMatrixRecord {
    /// 记录 ID。
    pub id: String,
    /// 学生 ID。
    pub student_id: String,
    /// 节点 ID。
    pub node_id: Option<String>,
    /// 节点 key。
    pub node_key: String,
    /// 评分。
    pub score: Option<i32>,
    /// 备注。
    pub note: Option<String>,
    /// 完成时间。
    pub completed_at: Option<i64>,
    /// 更新时间。
    pub updated_at: i64,
}

/// 一次性返回的任务矩阵（学生 × 记录）。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskMatrix {
    /// 任务 ID。
    pub task_id: String,
    /// 任务实体。
    pub task: Option<CustomTask>,
    /// 状态节点（按 order 升序）。
    pub nodes: Vec<TaskStatusNode>,
    /// 参与学生（默认排除已转出）。
    pub students: Vec<TaskMatrixStudent>,
    /// 已有记录。
    pub records: Vec<TaskMatrixRecord>,
}

/// 下发结果报告。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendReport {
    /// 广播任务 ID。
    pub broadcast_task_id: String,
    /// 目标设备总数（与前端 `expectCount` 对齐）。
    pub expect_count: i64,
    /// 已入队数（与前端 `enqueued` 对齐）。
    pub enqueued: i64,
    /// 跳过的设备数：离线或缺少地址（与前端 `skipped` 对齐）。
    pub skipped: i64,
    /// 目标设备列表。
    pub targets: Vec<String>,
}

/// 立即补发结果。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FlushReport {
    /// 成功条数。
    pub sent: i64,
    /// 失败条数。
    pub failed: i64,
    /// 剩余待发条数。
    pub remaining: i64,
}

/// 密钥轮换结果。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyInfo {
    /// 密钥标识。
    pub kid: String,
    /// 指纹（SHA-256 前 8 字节 hex）。
    pub fingerprint: String,
    /// Base64 形式的共享密钥（导出用）。
    pub secret_b64: String,
}

/// `/api/v1/ping` 响应体。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PingResponse {
    /// 设备 ID。
    pub device_id: String,
    /// 角色。
    pub role: String,
    /// 设备名。
    pub device_name: String,
    /// 年级。
    pub grade: Option<String>,
    /// 班级。
    pub class_name: Option<String>,
    /// 请求时间戳回显。
    pub ts: i64,
    /// 服务端当前时间戳（用于时钟偏移补偿）。
    pub server_ts: i64,
}

/// `/api/v1/whoami` 响应体。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WhoamiResponse {
    /// 设备 ID。
    pub device_id: String,
    /// 设备名。
    pub device_name: String,
    /// 角色。
    pub role: String,
    /// API 版本。
    pub api_version: String,
    /// 密钥 kid。
    pub kid: String,
    /// 端口。
    pub port: u16,
}

/// `/api/v1/ingest` 请求体中的单条增量。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IngestItem {
    /// 实体类型。
    pub entity_type: String,
    /// 操作类型。
    pub op_type: String,
    /// 实体快照。
    pub entity: serde_json::Value,
}

/// `/api/v1/ingest` 请求体。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IngestRequest {
    /// 发送方设备 ID。
    pub device_id: String,
    /// 发送方班级（便于 master 归类）。
    pub class_name: Option<String>,
    /// 增量条目。
    pub items: Vec<IngestItem>,
}

/// `/api/v1/ingest` 响应体。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IngestResponse {
    /// 接受条数。
    pub accepted: i64,
    /// 拒绝条数。
    pub rejected: i64,
    /// 冲突条数。
    pub conflicts: i64,
    /// 追踪 ID。
    pub trace_id: String,
}

/// `/api/v1/broadcast` 请求体。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BroadcastPush {
    /// 广播任务实体。
    pub task: BroadcastTask,
    /// 状态节点模板（班级端一键生成待办时使用）。
    pub nodes: Vec<serde_json::Value>,
}

/// `/api/v1/receipt` 请求体。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReceiptPush {
    /// 广播任务 ID。
    pub broadcast_task_id: String,
    /// 班级端设备 ID。
    pub device_id: String,
    /// 设备名。
    pub device_name: Option<String>,
    /// 班级。
    pub class_name: Option<String>,
    /// 回执状态。
    pub status: String,
    /// 生成的本地任务 ID。
    pub local_task_id: Option<String>,
    /// 失败原因。
    pub fail_reason: Option<String>,
}

/// `/api/v1/pull` 响应体：班级端主动拉取未收广播。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PullResponse {
    /// 未接收的广播任务列表。
    pub tasks: Vec<BroadcastPush>,
    /// 服务端时间。
    pub server_ts: i64,
}

/// 通用 ACK 响应体。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AckResponse {
    /// 是否接受。
    pub accepted: bool,
    /// 追踪 ID。
    pub trace_id: String,
    /// 附加说明。
    pub message: Option<String>,
}

/// `.sch` 离线包导出结果。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportSchResult {
    /// 导出文件绝对路径。
    pub file_path: String,
    /// 文件名。
    pub file_name: String,
    /// 各实体计数（`{ student, checkin, custom_task, ... }`）。
    pub entity_counts: serde_json::Value,
    /// SHA-256 校验和（hex）。
    pub checksum: String,
    /// 文件大小（字节）。
    pub size_bytes: i64,
}

/// 全校考勤汇总（教务处大屏首页）。
///
/// 前端 `SchoolSummary`（TS）约定字段：date / classCount / submittedClassCount /
/// totalStudents / markedStudents / present / leave / absent / late /
/// conflictCount / attendanceRate（0–100 百分比数字）。
/// 这里用 `#[serde(rename)]` 把内部 snake_case 字段映射到 camelCase API。
#[derive(Debug, Clone, Default, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct SchoolSummary {
    /// 汇总日期（YYYY-MM-DD）。
    pub date: String,
    /// 在读学生总数（排除已转出）。
    pub total_students: i64,
    /// 班级数。
    #[serde(rename = "classCount")]
    pub total_classes: i64,
    /// 已提交考勤的班级数。
    #[serde(rename = "submittedClassCount")]
    pub submitted_classes: i64,
    /// 当日已有考勤记录的学生数（不含默认出勤）。
    #[serde(rename = "markedStudents")]
    pub marked_students: i64,
    /// 冲突/重复打卡数（当前置 0）。
    #[serde(rename = "conflictCount")]
    pub conflict_count: i64,
    /// 出勤数（含「默认出勤」）。
    #[serde(rename = "present")]
    pub present_cnt: i64,
    /// 请假数。
    #[serde(rename = "leave")]
    pub leave_cnt: i64,
    /// 缺勤数。
    #[serde(rename = "absent")]
    pub absent_cnt: i64,
    /// 迟到数。
    #[serde(rename = "late")]
    pub late_cnt: i64,
    /// 出勤率（0–100 百分比数字）。
    pub attendance_rate: f64,
}

/// 班级考勤大屏行（按班级聚合）。
///
/// 前端 `ClassAttendanceRow`（TS）约定字段：className / grade / total / present /
/// leave / absent / late / attendanceRate（0–100 百分比数字）/ submitted。
#[derive(Debug, Clone, Default, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct ClassAttendanceRow {
    /// 年级。
    pub grade: Option<String>,
    /// 班级。
    pub class_name: String,
    /// 在读总数。
    #[serde(rename = "total")]
    pub total_cnt: i64,
    /// 出勤数。
    #[serde(rename = "present")]
    pub present_cnt: i64,
    /// 请假数。
    #[serde(rename = "leave")]
    pub leave_cnt: i64,
    /// 缺勤数。
    #[serde(rename = "absent")]
    pub absent_cnt: i64,
    /// 迟到数。
    #[serde(rename = "late")]
    pub late_cnt: i64,
    /// 出勤率（0–100 百分比数字）。
    pub attendance_rate: f64,
    /// 本班是否已提交（存在当日考勤记录）。
    pub submitted: bool,
}

/// 异常学生行（缺勤 / 请假 / 迟到），供教务处跨班查询。
#[derive(Debug, Clone, Default, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct ExceptionStudentRow {
    /// 学生 ID。
    pub student_id: String,
    /// 学号。
    pub student_no: String,
    /// 姓名。
    pub name: String,
    /// 年级。
    pub grade: Option<String>,
    /// 班级。
    pub class_name: Option<String>,
    /// 考勤状态。
    pub state: String,
    /// 日期。
    pub date: String,
    /// 时段。
    pub period: String,
    /// 班级端登记的备注（请假 / 迟到 / 跟进说明），未填为 `None`。
    pub note: Option<String>,
}

/// 任务完成率统计行（教务处端统计与导出）。
#[derive(Debug, Clone, Default, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct TaskCompletionRow {
    /// 任务 ID。
    pub task_id: String,
    /// 任务标题。
    pub title: String,
    /// 所属班级。
    pub class_name: Option<String>,
    /// 所属年级。
    pub grade: Option<String>,
    /// 参与学生数。
    pub total: i64,
    /// 到达终态数。
    pub final_count: i64,
    /// 完成率（0–1）。
    pub completion_rate: f64,
    /// 已评分记录的平均分。
    pub avg_score: Option<f64>,
}

/// 任务按班级聚合的处理进度（教务端看板）。
#[derive(Debug, Clone, Default, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct TaskProgressRow {
    pub task_id: String,
    pub title: String,
    pub class_name: String,
    pub grade: Option<String>,
    pub device_id: Option<String>,
    pub device_name: Option<String>,
    pub device_status: Option<String>,
    pub total: i64,
    pub final_count: i64,
    pub processing_count: i64,
    pub pending_count: i64,
    pub completion_rate: f64,
    pub avg_score: Option<f64>,
    pub last_updated_at: Option<i64>,
}

/// 通用分页响应。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Page<T> {
    pub items: Vec<T>,
    pub total: i64,
    pub page: i64,
    pub page_size: i64,
}

#[cfg(test)]
mod task_completion_row_tests {
    use super::TaskCompletionRow;

    #[test]
    fn serializes_the_completion_fields_consumed_by_the_frontend() {
        let row = TaskCompletionRow {
            task_id: "task-1".into(),
            title: "任务".into(),
            class_name: Some("一年级1班".into()),
            grade: Some("一年级".into()),
            total: 28,
            final_count: 3,
            completion_rate: 3.0 / 28.0,
            avg_score: Some(92.5),
        };
        let json = serde_json::to_value(row).expect("序列化统计行");
        assert_eq!(json["className"], "一年级1班");
        assert_eq!(json["grade"], "一年级");
        assert_eq!(json["finalCount"], 3);
        assert_eq!(json["avgScore"], 92.5);
    }
}
