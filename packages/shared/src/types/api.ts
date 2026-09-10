import type { ErrorCode } from './enums';
import type { CheckinState, Student, TaskRecord, TaskStatusNode } from './models';
import type { CustomTask } from './models';

/**
 * P2P 传输信封（Rust ↔ Rust，前端仅用于日志/调试展示，不参与加解密）。
 * 字段定义见 docs/01-architecture.md §7.5。
 */
export interface Envelope {
  v: number;
  alg: string;
  from: string;
  to: string;
  ts: number;
  nonce: string;
  kid: string;
  iv: string;
  aad: string;
  cipher: string;
  tag: string;
  sig: string;
}

/** Rust 侧 AppError 序列化后的形态 */
export interface RustErrorPayload {
  code: ErrorCode | string;
  message: string;
}

/** 标准 API 响应 */
export interface ApiResponse<T> {
  ok: boolean;
  data: T | null;
  error: RustErrorPayload | null;
  traceId: string | null;
}

/** 分页结构 */
export interface Page<T> {
  items: T[];
  total: number;
  page: number;
  pageSize: number;
}

/** 导入单行错误 */
export interface ImportRowError {
  row: number;
  field: string;
  message: string;
}

/** 导入结果报告 */
export interface ImportReport {
  batchId: string;
  totalRows: number;
  successRows: number;
  failedRows: number;
  errors: ImportRowError[];
  status: 'completed' | 'failed' | 'rolled_back';
}

/** 任务矩阵一次性查询结果 */
export interface TaskMatrix {
  task: CustomTask;
  nodes: TaskStatusNode[];
  students: Student[];
  records: TaskRecord[];
}

/** 手动补发结果 */
export interface FlushReport {
  sent: number;
  failed: number;
}

/** 密钥信息（设置页展示） */
export interface KeyInfo {
  kid: string;
  fingerprint: string;
  configured?: boolean;
}

/** 教务处端：全校汇总 */
export interface SchoolSummary {
  date: string;
  classCount: number;
  submittedClassCount: number;
  totalStudents: number;
  markedStudents: number;
  present: number;
  leave: number;
  absent: number;
  late: number;
  attendanceRate: number;
  conflictCount: number;
}

/** 教务处端：单班级出勤行 */
export interface ClassAttendanceRow {
  className: string;
  grade: string | null;
  deviceId: string | null;
  deviceName: string | null;
  total: number;
  present: number;
  leave: number;
  absent: number;
  late: number;
  marked: number;
  attendanceRate: number;
  submitted: boolean;
  lastUpdatedAt: number | null;
}

/** 教务处端：异常学生行 */
export interface ExceptionStudentRow {
  studentId: string;
  studentNo: string;
  name: string;
  className: string | null;
  grade: string | null;
  state: CheckinState;
  date: string;
  note: string | null;
}

/** 任务完成率统计行 */
export interface TaskCompletionRow {
  taskId: string;
  title: string;
  className: string | null;
  grade: string | null;
  total: number;
  finalCount: number;
  completionRate: number;
  avgScore: number | null;
}

/** 教务端任务按班级聚合进度 */
export interface TaskProgressRow {
  taskId: string;
  title: string;
  className: string;
  grade: string | null;
  deviceId: string | null;
  deviceName: string | null;
  deviceStatus: string | null;
  total: number;
  finalCount: number;
  processingCount: number;
  pendingCount: number;
  completionRate: number;
  avgScore: number | null;
  lastUpdatedAt: number | null;
}

/** .sch 导出入参 */
export interface ExportSchArgs {
  scope: 'class' | 'grade' | 'school';
  sinceTs: number | null;
  path: string;
}

/** .sch 导出结果 */
export interface ExportSchResult {
  path: string;
  fileName: string;
  sizeBytes: number;
  checksum: string;
  entityCounts: Record<string, number>;
}

/** 前端本地导入预览行（excel.ts 产出） */
export interface ParsedStudentRow {
  rowIndex: number;
  studentNo: string;
  name: string;
  gender: string;
  grade: string;
  className: string;
  seatNo: number | null;
  phone: string;
  note: string;
  errors: ImportRowError[];
}

/** 导出 sheet 定义 */
export interface ExportSheet {
  name: string;
  columns: string[];
  rows: (string | number | null)[][];
}
