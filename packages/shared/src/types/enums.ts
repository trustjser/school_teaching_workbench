/**
 * 全局枚举定义。
 * 所有取值与 docs/02-ddl.sql 的 CHECK 约束、Rust 侧枚举严格一致。
 */

// `AppMode` 由固定 app target 定义，此处再导出以保持既有导入路径不变。
export type { AppMode } from '../app-target';

/** 学生状态：在读 / 请假(长期) / 已转出 */
export type StudentStatus = 'active' | 'leave' | 'transferred';

/** 性别 */
export type Gender = 'male' | 'female' | 'unknown';

/** 考勤状态：出勤 / 请假 / 缺勤 / 迟到 */
export type CheckinState = 'present' | 'leave' | 'absent' | 'late';

/** 考勤时段 */
export type CheckinPeriod = 'am' | 'pm' | 'all' | 'custom';

/** 同步状态 */
export type SyncState = 'local' | 'pending' | 'synced' | 'conflict';

/** 任务类型 */
export type TaskType = 'custom' | 'recitation' | 'homework' | 'temperature' | 'checkin' | 'other';

/** 任务作用域 */
export type TaskScope = 'class' | 'grade' | 'school';

/** 任务状态 */
export type TaskStatus = 'draft' | 'active' | 'closed' | 'archived';

/** 矩阵视图模式 */
export type ViewMode = 'grid' | 'table';

/** 状态节点配色 token（与 DDL CHECK 一致） */
export type ColorToken =
  | 'slate'
  | 'blue'
  | 'amber'
  | 'emerald'
  | 'rose'
  | 'violet'
  | 'cyan'
  | 'orange';

/** 广播目标类型 */
export type BroadcastTargetType = 'school' | 'grade' | 'class' | 'device';

/** 广播优先级 */
export type BroadcastPriority = 'low' | 'normal' | 'high' | 'urgent';

/** 广播方向：out=本端发出 / in=本端接收 */
export type BroadcastDirection = 'out' | 'in';

/** 广播任务状态 */
export type BroadcastStatus =
  | 'draft'
  | 'sending'
  | 'sent'
  | 'partial'
  | 'closed'
  | 'cancelled';

/** 回执状态 */
export type ReceiptStatus = 'received' | 'accepted' | 'rejected' | 'done';

/** 设备角色 */
export type DeviceRole = 'client' | 'master' | 'unknown';

/** 设备在线状态 */
export type DeviceStatus = 'online' | 'offline' | 'stale' | 'blocked';

/** 队列操作类型 */
export type OpType = 'upsert' | 'delete' | 'ack' | 'heartbeat' | 'broadcast';

/** 队列实体类型 */
export type EntityType =
  | 'student'
  | 'checkin'
  | 'custom_task'
  | 'task_node'
  | 'task_record'
  | 'broadcast_task'
  | 'receipt'
  | 'device'
  | 'grade'
  | 'class';

/** 队列条目状态 */
export type QueueStatus = 'pending' | 'sending' | 'done' | 'failed' | 'dead';

/** 同步日志结果 */
export type SyncLogResult = 'success' | 'failed' | 'partial' | 'rejected';

/** 导入批次来源类型 */
export type ImportSourceType = 'xlsx' | 'csv' | 'sch' | 'manual' | 'api';

/** 导入批次状态 */
export type ImportBatchStatus = 'processing' | 'completed' | 'failed' | 'rolled_back';

/** 离线包方向 */
export type PackageDirection = 'export' | 'import';

/** 离线包类型 */
export type PackageType = 'full' | 'delta';

/** 离线包状态 */
export type PackageStatus = 'pending' | 'done' | 'failed' | 'verified';

/** 数据来源 */
export type DataSource = 'local' | 'api' | 'broadcast' | 'sch_import';

/** 主题 */
export type ThemeName = 'light' | 'dark' | 'high-contrast';

/** 错误码（与 docs/03-tasks.md §4.3 完全一致） */
export type ErrorCode =
  | 'ERR_DB'
  | 'ERR_NET'
  | 'ERR_SIGN'
  | 'ERR_CRYPTO'
  | 'ERR_TS_WINDOW'
  | 'ERR_NONCE_REPLAY'
  | 'ERR_VALIDATION'
  | 'ERR_NOT_FOUND'
  | 'ERR_MODE'
  | 'ERR_PERMISSION'
  | 'ERR_IMPORT'
  | 'ERR_UNKNOWN';
