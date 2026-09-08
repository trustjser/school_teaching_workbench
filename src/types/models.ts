import type {
  AppMode,
  BroadcastDirection,
  BroadcastPriority,
  BroadcastStatus,
  BroadcastTargetType,
  CheckinPeriod,
  CheckinState,
  ColorToken,
  DataSource,
  DeviceRole,
  DeviceStatus,
  EntityType,
  Gender,
  ImportBatchStatus,
  ImportSourceType,
  OpType,
  PackageDirection,
  PackageStatus,
  PackageType,
  QueueStatus,
  ReceiptStatus,
  StudentStatus,
  SyncLogResult,
  SyncState,
  TaskScope,
  TaskStatus,
  TaskType,
  ThemeName,
  ViewMode,
} from './enums';

/** 枚举类型再导出（store / api 从 models 统一引用） */
export type { StudentStatus, CheckinState } from './enums';

/**
 * 实体公共基类。
 * 所有时间戳均为 **毫秒 epoch UTC**；ID 为 UUID v4 小写带连字符。
 */
export interface BaseEntity {
  id: string;
  createdAt: number;
  updatedAt: number;
  deletedAt: number | null;
  syncState: SyncState;
  dirty: boolean;
}

/** 应用配置项（KV） */
export interface AppSetting extends BaseEntity {
  settingKey: string;
  settingValue: string | null;
  valueType: 'string' | 'number' | 'boolean' | 'json' | 'secret';
  remark: string | null;
}

/** 学生名册 */
export interface Student extends BaseEntity {
  studentNo: string;
  name: string;
  gender: Gender;
  grade: string | null;
  className: string | null;
  /** 关联班级目录 id（教务端统一维护后落位；为空时回退用 className 匹配） */
  classId: string | null;
  seatNo: number | null;
  status: StudentStatus;
  statusSince: number | null;
  note: string | null;
  phone: string | null;
  importBatchId: string | null;
}

/** 年级（教务端统一维护，全校唯一） */
export interface Grade extends BaseEntity {
  /** 年级编号，如 '3' / '2023' */
  gradeNo: string;
  /** 展示名，如 '三年级' */
  gradeName: string;
  /** 排序（数字越小越靠前） */
  sortOrder: number;
  remark: string | null;
}

/** 班级（归属某个年级，教务端统一维护） */
export interface Class extends BaseEntity {
  /** 关联 grades.id（软删时置空） */
  gradeId: string | null;
  /** 冗余：年级编号 */
  gradeNo: string | null;
  /** 冗余：年级展示名 */
  gradeName: string | null;
  /** 班号，如 '2' */
  classNo: string | null;
  /** 展示名，如 '三年级二班' */
  className: string;
  /** 班主任 */
  headTeacher: string | null;
  /** 班级排序 */
  sortOrder: number;
  remark: string | null;
}

/** 导入批次 */
export interface ImportBatch extends BaseEntity {
  batchName: string;
  sourceType: ImportSourceType;
  sourceFile: string | null;
  totalRows: number;
  successRows: number;
  failedRows: number;
  status: ImportBatchStatus;
  errorReport: string | null;
  importedBy: string | null;
}

/** 考勤记录 */
export interface CheckinRecord extends BaseEntity {
  studentId: string;
  checkinDate: string;
  period: CheckinPeriod;
  periodLabel: string | null;
  state: CheckinState;
  markedBy: string | null;
  markedAt: number | null;
  note: string | null;
  source: DataSource;
}

/** 自定义任务 */
export interface CustomTask extends BaseEntity {
  title: string;
  description: string | null;
  taskType: TaskType;
  scope: TaskScope;
  grade: string | null;
  className: string | null;
  dueAt: number | null;
  status: TaskStatus;
  viewMode: ViewMode;
  scoreEnabled: boolean;
  noteEnabled: boolean;
  defaultNodeId: string | null;
  ownerDeviceId: string | null;
  broadcastTaskId: string | null;
  source: 'local' | 'broadcast' | 'sch_import';
  sortOrder: number;
}

/** 任务自定义状态节点（每个任务 2~4 个） */
export interface TaskStatusNode extends BaseEntity {
  taskId: string;
  nodeKey: string;
  label: string;
  colorToken: ColorToken;
  iconName: string | null;
  nodeOrder: number;
  isFinal: boolean;
  isDefault: boolean;
}

/** 任务 × 学生 矩阵单元格 */
export interface TaskRecord extends BaseEntity {
  taskId: string;
  studentId: string;
  nodeId: string | null;
  nodeKey: string;
  score: number | null;
  note: string | null;
  completedAt: number | null;
  evaluatedBy: string | null;
}

/** 局域网节点（mDNS 发现 + 心跳维护） */
export interface Device extends BaseEntity {
  deviceId: string;
  deviceName: string;
  deviceRole: DeviceRole;
  ipAddress: string | null;
  port: number | null;
  mdnsFullname: string | null;
  txtClassName: string | null;
  txtGrade: string | null;
  txtApiVersion: string | null;
  txtKeyId: string | null;
  status: DeviceStatus;
  lastSeenAt: number | null;
  lastHeartbeatAt: number | null;
  lastLatencyMs: number | null;
  missCount: number;
  isSelf: boolean;
}

/** 离线待发队列条目（Outbox） */
export interface PendingQueueItem extends BaseEntity {
  opType: OpType;
  entityType: EntityType;
  entityId: string;
  payload: string;
  targetDeviceId: string | null;
  targetEndpoint: string;
  targetBaseUrl: string | null;
  attemptCount: number;
  maxAttempts: number;
  nextRetryAt: number;
  lastError: string | null;
  status: QueueStatus;
  priority: number;
  batchId: string | null;
}

/** 同步日志 */
export interface SyncLogEntry extends BaseEntity {
  direction: 'out' | 'in';
  peerDeviceId: string | null;
  peerName: string | null;
  endpoint: string | null;
  entityType: EntityType | null;
  entityCount: number;
  result: SyncLogResult;
  httpStatus: number | null;
  errorCode: string | null;
  errorMessage: string | null;
  durationMs: number | null;
  queueId: string | null;
  traceId: string | null;
}

/** .sch 离线包审计记录 */
export interface OfflinePackage extends BaseEntity {
  fileName: string;
  filePath: string | null;
  direction: PackageDirection;
  packageType: PackageType;
  scope: string | null;
  entityCounts: string | null;
  checksum: string | null;
  sizeBytes: number;
  status: PackageStatus;
  sinceTs: number | null;
  untilTs: number | null;
}

/** 前端本地运行期设置（由 app_settings 折叠而来） */
export interface AppRuntimeSettings {
  appMode: AppMode;
  firstRunDone: boolean;
  deviceId: string;
  deviceName: string;
  grade: string | null;
  className: string | null;
  /** 当前班级端绑定的班级目录 id（教务端目录消费主键） */
  classId: string | null;
  schoolName: string | null;
  apiPort: number;
  mdnsServiceType: string;
  uiScale: number;
  theme: ThemeName;
  hmacTsWindowSec: number;
  heartbeatInterval: number;
  offlineTtlSec: number;
  queueMaxAttempts: number;
}

/** 教务处下发任务（定义见 types/broadcast.ts，此处仅为聚合引用） */
export interface BroadcastTaskRef {
  id: string;
  title: string;
  status: BroadcastStatus;
  targetType: BroadcastTargetType;
  priority: BroadcastPriority;
  direction: BroadcastDirection;
}

/** 学生 + 考勤态的组合视图（考勤网格渲染用） */
export interface CheckinRow {
  student: Student;
  record: CheckinRecord | null;
  /** 有效状态：无记录时视为 present（反向标记） */
  effectiveState: CheckinState;
}

/** 任务矩阵单元格视图 */
export interface TaskMatrixCell {
  student: Student;
  record: TaskRecord | null;
  /** 有效节点 key：无记录时取任务默认节点 */
  nodeKey: string;
}

/** 每日考勤汇总（单班级） */
export interface DailySummary {
  grade: string | null;
  className: string | null;
  checkinDate: string;
  period: string;
  presentCnt: number;
  leaveCnt: number;
  absentCnt: number;
  lateCnt: number;
  markedCnt: number;
}

/** 班级上下文（教务端目录消费主键）：学生新增 / 导入时落位使用 */
export interface ClassContext {
  classId: string | null;
  className: string;
  grade: string | null;
}
