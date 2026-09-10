//! 全局常量单一来源，与前端 `src/constants/app.ts` 保持一致。

use once_cell::sync::Lazy;

/// Axum 本地 API 默认监听端口。
pub const API_PORT: u16 = 5178;
/// 端口被占用时向后探测的最大次数（5178 → 5197）。
pub const API_PORT_PROBE_MAX: u16 = 20;
/// API 版本前缀（拼接后为 `/api/v1`）。
pub const API_VERSION: &str = "v1";
/// API 路径前缀。
pub const API_PREFIX: &str = "/api/v1";

/// mDNS 服务类型。
pub const MDNS_SERVICE_TYPE: &str = "_schworkbench._tcp.local.";
/// mDNS 标准 UDP 端口。
pub const MDNS_PORT: u16 = 5353;
/// mDNS TXT 协议版本值。
pub const MDNS_TXT_VERSION: &str = "1";

/// 心跳周期（秒）。
pub const HEARTBEAT_INTERVAL_SEC: i64 = 15;
/// 判定设备离线的连续静默时长（秒）。
pub const OFFLINE_TTL_SEC: i64 = 45;
/// 连续心跳失败多少次后标记离线。
pub const HEARTBEAT_MISS_LIMIT: i32 = 3;

/// HMAC 时间戳容差窗口（秒）。
pub const HMAC_TS_WINDOW_SEC: i64 = 300;
/// nonce 缓存 TTL（秒）= 2 × 窗口。
pub const NONCE_TTL_SEC: i64 = 600;
/// nonce 缓存容量上限。
pub const NONCE_CACHE_CAPACITY: usize = 10_000;

/// 离线队列最大重试次数。
pub const QUEUE_MAX_ATTEMPTS: i32 = 5;
/// 单次补发最多取多少条。
pub const QUEUE_BATCH_SIZE: i32 = 50;
/// 退避基数（毫秒）。
pub const BACKOFF_BASE_MS: i64 = 2_000;
/// 退避上限（毫秒）。
pub const BACKOFF_CAP_MS: i64 = 300_000;
/// 退避抖动比例（±20%）。
pub const BACKOFF_JITTER_RATIO: f64 = 0.2;

/// 同步 worker 轮询周期（毫秒）。
pub const SYNC_POLL_INTERVAL_MS: u64 = 3_000;
/// 队列为空时的退避轮询周期（毫秒）。
pub const SYNC_IDLE_INTERVAL_MS: u64 = 10_000;

/// SQLite busy_timeout（毫秒）。
pub const DB_BUSY_TIMEOUT_MS: u64 = 5_000;
/// 数据库连接池最大连接数（SQLite 写串行，保持小值）。
pub const DB_MAX_CONNECTIONS: u32 = 4;

/// HTTP 客户端超时（秒）。
pub const HTTP_TIMEOUT_SEC: u64 = 5;

/// 算法套件标识，写入 `Envelope.alg`。
pub const CRYPTO_ALG: &str = "AES-256-GCM+HMAC-SHA256";
/// 信封版本号。
pub const ENVELOPE_VERSION: i32 = 1;
/// canonical string 前缀。
pub const SIGN_PREFIX: &str = "SCH1";
/// HKDF info 前缀（架构 §7.4：`sch-workbench/v1/aes-gcm` || kid）。
pub const HKDF_INFO_PREFIX: &str = "sch-workbench/v1/aes-gcm";
/// 广播目标（`to = "*"`）时的会话密钥 salt 前缀。
pub const BROADCAST_SALT_PREFIX: &str = "broadcast";
/// 离线包魔数。
pub const SCH_MAGIC: &str = "SCHWB";
/// 离线包格式版本。
pub const SCH_VERSION: i32 = 1;

/// 队列优先级：考勤（最高）。
pub const PRIORITY_CHECKIN: i32 = 1;
/// 队列优先级：广播。
pub const PRIORITY_BROADCAST: i32 = 2;
/// 队列优先级：任务。
pub const PRIORITY_TASK: i32 = 3;
/// 队列优先级：名册（普通）。
pub const PRIORITY_STUDENT: i32 = 5;
/// 队列优先级：设备心跳（最低）。
pub const PRIORITY_DEVICE: i32 = 9;

/// SQLite 数据库文件名。
pub const DB_FILE_NAME: &str = "lan_workbench.db";

/// Tauri 事件名常量集合，与 `src/lib/events.ts` 一一对应。
pub struct Events;

impl Events {
    /// 发现新节点。
    pub const DEVICE_FOUND: &'static str = "device://found";
    /// 节点失联。
    pub const DEVICE_LOST: &'static str = "device://lost";
    /// 心跳成功。
    pub const DEVICE_HEARTBEAT: &'static str = "device://heartbeat";
    /// 节点离线。
    pub const DEVICE_OFFLINE: &'static str = "device://offline";
    /// 设备列表整体变化。
    pub const DEVICE_CHANGED: &'static str = "device://changed";
    /// 同步进度。
    pub const SYNC_PROGRESS: &'static str = "sync://progress";
    /// 同步错误（条目转死信）。
    pub const SYNC_ERROR: &'static str = "sync://error";
    /// 待发队列变化。
    pub const SYNC_QUEUE_CHANGED: &'static str = "sync://queue-changed";
    /// 收到对端考勤增量。
    pub const CHECKIN_UPDATED: &'static str = "checkin://updated";
    /// 任务或矩阵变更。
    pub const TASK_UPDATED: &'static str = "task://updated";
    /// 班级端收到下发任务。
    pub const BROADCAST_RECEIVED: &'static str = "broadcast://received";
    /// 回执到达。
    pub const BROADCAST_RECEIPT: &'static str = "broadcast://receipt";
    /// 教务端撤回已送达的下发（班级端收到撤回指令后发出）。
    pub const BROADCAST_RECALLED: &'static str = "broadcast://recalled";
    /// 名册或 `.sch` 导入完成。
    pub const DATA_IMPORTED: &'static str = "data://imported";
    /// 年级目录变更（新增/修改/软删）。
    pub const GRADE_CHANGED: &'static str = "grade://changed";
    /// 班级目录变更（新增/修改/软删）。
    pub const CLASS_CHANGED: &'static str = "class://changed";
    /// 教室认领或解除认领。
    pub const CLASSROOM_CHANGED: &'static str = "classroom://changed";
    /// 学生目录变更。
    pub const STUDENT_CHANGED: &'static str = "student://changed";
    /// 学年目录变更（新增/修改/软删）。
    pub const SCHOOL_YEAR_CHANGED: &'static str = "school_year://changed";
    /// 模式热切换。
    pub const MODE_CHANGED: &'static str = "mode://changed";
    /// 导出/导入进度。
    pub const PACKAGE_PROGRESS: &'static str = "package://progress";
}

/// 进程级设备 ID 兜底（极少数场景在 DB 就绪前就需要身份时使用）。
pub static PROCESS_ID: Lazy<String> = Lazy::new(|| uuid::Uuid::new_v4().to_string());
