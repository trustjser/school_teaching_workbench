import { invokeCmd } from './tauri';
import { safeJsonParse } from './format';
import type { AppMode, CheckinPeriod, CheckinState, StudentStatus, SyncState } from '@/types/enums';
import type {
  AppRuntimeSettings,
  AppSetting,
  CheckinRecord,
  Class,
  Classroom,
  ClassroomAssignment,
  CustomTask,
  DailySummary,
  Device,
  Grade,
  OfflinePackage,
  PendingQueueItem,
  SchoolYear,
  Student,
  SyncLogEntry,
  TaskRecord,
  TaskStatusNode,
} from '@/types/models';
import type {
  ClassAttendanceRow,
  ExceptionStudentRow,
  ExportSchResult,
  FlushReport,
  ImportReport,
  KeyInfo,
  Page,
  SchoolSummary,
  TaskCompletionRow,
  TaskProgressRow,
  TaskMatrix,
} from '@/types/api';
import type {
  BroadcastReceipt,
  BroadcastTask,
  SendReport,
} from '@/types/broadcast';
import { DEFAULT_SETTINGS } from '@/constants/app';

/**
 * 数据访问服务：按领域分组。
 * 所有函数内部统一走 lib/tauri.ts 的 invokeCmd，禁止前端直接写 SQL。
 */

/* -------------------------------------------------------------------------- */
/* settings                                                                    */
/* -------------------------------------------------------------------------- */

export async function settingsGetAll(): Promise<AppSetting[]> {
  return invokeCmd<AppSetting[]>('settings_get_all');
}

export async function settingsSet(
  key: string,
  value: string,
  valueType: AppSetting['valueType'] = 'string',
): Promise<void> {
  await invokeCmd<void>('settings_set', { key, value, valueType });
}

export async function settingsSetSharedSecret(secret: string, kid: string): Promise<void> {
  await settingsSet('shared_secret_b64', secret, 'secret');
  await settingsSet('key_id', kid, 'string');
}

export interface CompleteSetupArgs {
  mode: AppMode;
  deviceName: string;
  /** 班级端固定教室名称，首次配置时用于绑定本机设备 */
  roomName?: string | null;
  grade: string | null;
  className: string | null;
  /** 班级端绑定的班级目录 id（教务端目录消费主键） */
  classId?: string | null;
  /** 当前学年 id（班级端按年隔离消费） */
  schoolYearId?: string | null;
  /** 班级端当前绑定的学年班级 id（跨年只切绑定、不重装） */
  boundClassId?: string | null;
  schoolName?: string | null;
  /** Base64 共享密钥；两端初始化时均必填 */
  secret: string | null;
}

export async function settingsCompleteSetup(args: CompleteSetupArgs): Promise<void> {
  await invokeCmd<void>('settings_complete_setup', {
    mode: args.mode,
    deviceName: args.deviceName,
    grade: args.grade,
    className: args.className,
    classId: args.classId ?? null,
    schoolYearId: args.schoolYearId ?? null,
    boundClassId: args.boundClassId ?? null,
    schoolName: args.schoolName ?? null,
    secret: args.secret,
  });
}

export async function settingsSwitchMode(mode: AppMode): Promise<AppMode> {
  return invokeCmd<AppMode>('settings_switch_mode', { mode });
}

export async function settingsRotateKey(): Promise<{ kid: string }> {
  return invokeCmd<{ kid: string }>('settings_rotate_key');
}

export async function settingsKeyInfo(): Promise<KeyInfo> {
  return invokeCmd<KeyInfo>('settings_key_info');
}

/** 把 AppSetting[] 折叠成前端运行期设置对象 */
export function toRuntimeSettings(list: AppSetting[]): AppRuntimeSettings {
  const map: Record<string, string> = {};
  list.forEach((s) => {
    if (s && s.settingKey) map[s.settingKey] = s.settingValue ?? '';
  });
  const num = (key: string, fallback: number): number => {
    const v = Number.parseFloat(map[key]);
    return Number.isFinite(v) ? v : fallback;
  };
  const mode: AppMode = map.app_mode === 'master' ? 'master' : 'client';
  return {
    appMode: mode,
    firstRunDone:
      map.completed_setup === 'true' ||
      map.completed_setup === '1' ||
      map.first_run_done === 'true' ||
      map.first_run_done === '1',
    deviceId: map.device_id || '',
    deviceName: map.device_name || '',
    grade: map.grade || null,
    className: map.class_name || null,
    classId: map.class_id || null,
    schoolYearId: map.school_year_id || null,
    boundClassId: map.bound_class_id || map.class_id || null,
    schoolName: map.school_name || null,
    apiPort: num('api_port', DEFAULT_SETTINGS.apiPort),
    mdnsServiceType: map.mdns_service_type || DEFAULT_SETTINGS.mdnsServiceType,
    uiScale: num('ui_scale', DEFAULT_SETTINGS.uiScale),
    theme: (map.theme as AppRuntimeSettings['theme']) || 'light',
    hmacTsWindowSec: num('hmac_ts_window_sec', DEFAULT_SETTINGS.hmacTsWindowSec),
    heartbeatInterval: num('heartbeat_interval', DEFAULT_SETTINGS.heartbeatInterval),
    offlineTtlSec: num('offline_ttl_sec', DEFAULT_SETTINGS.offlineTtlSec),
    queueMaxAttempts: num('queue_max_attempts', DEFAULT_SETTINGS.queueMaxAttempts),
  };
}

/* -------------------------------------------------------------------------- */
/* student                                                                     */
/* -------------------------------------------------------------------------- */

export interface StudentListArgs {
  className?: string | null;
  /** 班级目录 id（教务端目录消费主路径） */
  classId?: string | null;
  status?: StudentStatus | null;
  keyword?: string | null;
  includeDeleted?: boolean;
}

export async function studentList(args: StudentListArgs = {}): Promise<Student[]> {
  return invokeCmd<Student[]>('student_list', {
    className: args.className ?? null,
    classId: args.classId ?? null,
    status: args.status ?? null,
    keyword: args.keyword ?? null,
    includeDeleted: args.includeDeleted ?? false,
  });
}

export async function studentUpsert(student: Partial<Student> & { name: string }): Promise<Student> {
  return invokeCmd<Student>('student_upsert', { student });
}

/** 批量导入行（前端校验后的干净数据） */
export interface StudentImportRowInput {
  studentNo: string;
  name: string;
  gender: string;
  grade: string | null;
  className: string | null;
  /** 关联班级目录 id（教务端在班级上下文中导入时落位） */
  classId?: string | null;
  seatNo: number | null;
  phone: string | null;
  note: string | null;
}

export async function studentBatchImport(
  rows: StudentImportRowInput[],
  batchName: string,
): Promise<ImportReport> {
  return invokeCmd<ImportReport>('student_batch_import', { rows, batchName });
}

export async function studentUpdateStatus(id: string, status: StudentStatus): Promise<Student> {
  return invokeCmd<Student>('student_update_status', { id, status });
}

export async function studentDelete(id: string): Promise<void> {
  await invokeCmd<void>('student_delete', { id });
}

/* -------------------------------------------------------------------------- */
/* directory（年级 / 班级）                                                       */
/* -------------------------------------------------------------------------- */

export async function gradeList(): Promise<Grade[]> {
  return invokeCmd<Grade[]>('grade_list');
}

export async function gradeUpsert(grade: Partial<Grade> & { gradeName: string }): Promise<Grade> {
  return invokeCmd<Grade>('grade_upsert', { grade });
}

export async function gradeDelete(id: string): Promise<void> {
  await invokeCmd<void>('grade_delete', { id });
}

export async function classList(
  gradeId?: string | null,
  schoolYearId?: string | null,
): Promise<Class[]> {
  return invokeCmd<Class[]>('class_list', {
    gradeId: gradeId ?? null,
    schoolYearId: schoolYearId ?? null,
  });
}

export interface DirectorySyncReport {
  schoolYears: number;
  grades: number;
  classes: number;
  classrooms: number;
  assignments: number;
}

export async function directorySync(): Promise<DirectorySyncReport> {
  return invokeCmd<DirectorySyncReport>('directory_sync');
}

export async function classUpsert(klass: Partial<Class> & { className: string }): Promise<Class> {
  return invokeCmd<Class>('class_upsert', { class: klass });
}

export async function classDelete(id: string): Promise<void> {
  await invokeCmd<void>('class_delete', { id });
}

export async function classroomList(): Promise<Classroom[]> {
  return invokeCmd<Classroom[]>('classroom_list');
}

export async function classroomAssignments(schoolYearId?: string | null): Promise<ClassroomAssignment[]> {
  return invokeCmd<ClassroomAssignment[]>('classroom_assignments', { schoolYearId: schoolYearId ?? null });
}

export async function classroomUpsert(room: Partial<Classroom> & { roomName: string }): Promise<Classroom> {
  return invokeCmd<Classroom>('classroom_upsert', { classroom: room });
}

export async function classroomDelete(id: string): Promise<void> {
  await invokeCmd<void>('classroom_delete', { id });
}

export async function classroomAssign(classroomId: string, schoolYearId: string, classId: string): Promise<ClassroomAssignment> {
  return invokeCmd<ClassroomAssignment>('classroom_assign', { classroomId, schoolYearId, classId });
}

export async function classroomClaim(classroomId: string, schoolYearId: string): Promise<Classroom> {
  return invokeCmd<Classroom>('classroom_claim', { classroomId, schoolYearId });
}

export async function classroomRelease(classroomId: string): Promise<void> {
  await invokeCmd<void>('classroom_release', { classroomId });
}

export async function settingsResetClient(): Promise<void> {
  await invokeCmd<void>('settings_reset_client');
}

/* -------------------------------------------------------------------------- */
/* school year（学年 / 届）                                                      */
/* -------------------------------------------------------------------------- */

export async function schoolYearList(): Promise<SchoolYear[]> {
  return invokeCmd<SchoolYear[]>('school_year_list');
}

export async function schoolYearUpsert(
  schoolYear: Partial<SchoolYear> & { schoolYearName: string },
): Promise<SchoolYear> {
  return invokeCmd<SchoolYear>('school_year_upsert', { schoolYear });
}

export async function schoolYearDelete(id: string): Promise<void> {
  await invokeCmd<void>('school_year_delete', { id });
}

/* -------------------------------------------------------------------------- */
/* checkin                                                                     */
/* -------------------------------------------------------------------------- */

export async function checkinList(date: string, period: CheckinPeriod): Promise<CheckinRecord[]> {
  return invokeCmd<CheckinRecord[]>('checkin_list', { date, period });
}

export interface CheckinMarkArgs {
  studentId: string;
  date: string;
  period: CheckinPeriod;
  state: CheckinState;
  note?: string | null;
}

export async function checkinMark(args: CheckinMarkArgs): Promise<CheckinRecord> {
  return invokeCmd<CheckinRecord>('checkin_mark', {
    studentId: args.studentId,
    date: args.date,
    period: args.period,
    state: args.state,
    note: args.note ?? null,
  });
}

export async function checkinBatchMark(items: CheckinMarkArgs[]): Promise<CheckinRecord[]> {
  return invokeCmd<CheckinRecord[]>('checkin_batch_mark', { items });
}

export async function checkinDailySummary(date: string): Promise<DailySummary[]> {
  return invokeCmd<DailySummary[]>('checkin_daily_summary', { date });
}

export async function checkinSchoolSummary(date: string): Promise<SchoolSummary> {
  return invokeCmd<SchoolSummary>('checkin_school_summary', { date });
}

export async function checkinClassAttendance(date: string): Promise<ClassAttendanceRow[]> {
  return invokeCmd<ClassAttendanceRow[]>('checkin_class_attendance', { date });
}

export async function checkinExceptionStudents(
  date: string,
  className?: string | null,
): Promise<ExceptionStudentRow[]> {
  return invokeCmd<ExceptionStudentRow[]>('checkin_exception_students', {
    date,
    className: className ?? null,
  });
}

/* -------------------------------------------------------------------------- */
/* task                                                                        */
/* -------------------------------------------------------------------------- */

export async function taskList(status?: string | null): Promise<CustomTask[]> {
  return invokeCmd<CustomTask[]>('task_list', { status: status ?? null });
}

export async function taskPage(
  page: number,
  pageSize: number,
  keyword?: string | null,
  status?: string | null,
): Promise<Page<CustomTask>> {
  return invokeCmd<Page<CustomTask>>('task_page', {
    page,
    pageSize,
    keyword: keyword?.trim() || null,
    status: status?.trim() || null,
  });
}

export async function taskUpsert(task: Partial<CustomTask> & { title: string }): Promise<CustomTask> {
  return invokeCmd<CustomTask>('task_upsert', { task });
}

export async function taskDelete(id: string): Promise<void> {
  await invokeCmd<void>('task_delete', { id });
}

export async function taskNodeList(taskId: string): Promise<TaskStatusNode[]> {
  return invokeCmd<TaskStatusNode[]>('task_node_list', { taskId });
}

export async function taskNodeUpsert(
  node: Partial<TaskStatusNode> & { taskId: string; label: string },
): Promise<TaskStatusNode> {
  return invokeCmd<TaskStatusNode>('task_node_upsert', { node });
}

export async function taskNodeDelete(id: string): Promise<void> {
  await invokeCmd<void>('task_node_delete', { id });
}

export async function taskRecordUpsert(
  record: Partial<TaskRecord> & { taskId: string; studentId: string; nodeKey: string },
): Promise<TaskRecord> {
  return invokeCmd<TaskRecord>('task_record_upsert', { record });
}

export async function taskRecordsBatchUpsert(records: TaskRecord[]): Promise<TaskRecord[]> {
  return invokeCmd<TaskRecord[]>('task_records_batch_upsert', { records });
}

export async function taskMatrixQuery(taskId: string): Promise<TaskMatrix> {
  return invokeCmd<TaskMatrix>('task_matrix_query', { taskId });
}

export async function taskProgressList(
  taskId: string,
  grade?: string | null,
  className?: string | null,
): Promise<TaskProgressRow[]> {
  return invokeCmd<TaskProgressRow[]>('task_progress_list', {
    taskId,
    grade: grade ?? null,
    className: className ?? null,
  });
}

export async function taskClassMatrixQuery(taskId: string, className: string): Promise<TaskMatrix> {
  return invokeCmd<TaskMatrix>('task_class_matrix_query', { taskId, className });
}

export async function taskCompletionStats(sinceTs?: number | null): Promise<TaskCompletionRow[]> {
  return invokeCmd<TaskCompletionRow[]>('task_completion_stats', { sinceTs: sinceTs ?? null });
}

/* -------------------------------------------------------------------------- */
/* broadcast                                                                   */
/* -------------------------------------------------------------------------- */

export async function broadcastList(direction: 'out' | 'in'): Promise<BroadcastTask[]> {
  return invokeCmd<BroadcastTask[]>('broadcast_list', { direction });
}

export async function broadcastPage(
  direction: 'out' | 'in',
  page: number,
  pageSize: number,
  keyword?: string | null,
  status?: string | null,
): Promise<Page<BroadcastTask>> {
  return invokeCmd<Page<BroadcastTask>>('broadcast_page', {
    direction,
    page,
    pageSize,
    keyword: keyword?.trim() || null,
    status: status?.trim() || null,
  });
}

export async function broadcastCreate(
  task: Partial<BroadcastTask> & { title: string; payload: string },
): Promise<BroadcastTask> {
  return invokeCmd<BroadcastTask>('broadcast_create', { task });
}

/**
 * 下发广播任务。
 *
 * `targetDeviceIds` 必须是**已展开的 device_id 数组**——Rust 侧 `targets: Vec<String>`
 * 只认设备 ID，不认 `{ targetType, values }` 选择器。展开请用
 * `resolveTargetDeviceIds(selector, devices)`。
 */
export async function broadcastSend(
  id: string,
  targetDeviceIds: string[],
): Promise<SendReport> {
  return invokeCmd<SendReport>('broadcast_send', { id, targets: targetDeviceIds });
}

export async function broadcastReceipts(broadcastTaskId: string): Promise<BroadcastReceipt[]> {
  return invokeCmd<BroadcastReceipt[]>('broadcast_receipts', { broadcastTaskId });
}

export async function broadcastAccept(broadcastTaskId: string): Promise<CustomTask> {
  return invokeCmd<CustomTask>('broadcast_accept', { broadcastTaskId });
}

/* -------------------------------------------------------------------------- */
/* device                                                                      */
/* -------------------------------------------------------------------------- */

export async function deviceList(): Promise<Device[]> {
  return invokeCmd<Device[]>('device_list');
}

export async function deviceRefresh(): Promise<Device[]> {
  return invokeCmd<Device[]>('device_refresh');
}

export async function deviceForget(deviceId: string): Promise<void> {
  await invokeCmd<void>('device_forget', { deviceId });
}

/* -------------------------------------------------------------------------- */
/* sync                                                                        */
/* -------------------------------------------------------------------------- */

export async function syncQueueList(status?: string | null): Promise<PendingQueueItem[]> {
  return invokeCmd<PendingQueueItem[]>('sync_queue_list', { status: status ?? null });
}

export async function syncFlush(): Promise<FlushReport> {
  return invokeCmd<FlushReport>('sync_flush');
}

export async function syncRetry(id: string): Promise<void> {
  await invokeCmd<void>('sync_retry', { id });
}

export async function syncLogList(limit = 100): Promise<SyncLogEntry[]> {
  return invokeCmd<SyncLogEntry[]>('sync_log_list', { limit });
}

/* -------------------------------------------------------------------------- */
/* package                                                                     */
/* -------------------------------------------------------------------------- */

export async function packageExportSch(
  scope: 'class' | 'grade' | 'school',
  sinceTs: number | null,
  path: string,
): Promise<ExportSchResult> {
  return invokeCmd<ExportSchResult>('package_export_sch', { scope, sinceTs, path });
}

export async function packageImportSch(path: string): Promise<ImportReport> {
  return invokeCmd<ImportReport>('package_import_sch', { path });
}

export async function packageList(): Promise<OfflinePackage[]> {
  return invokeCmd<OfflinePackage[]>('package_list');
}

/** 解析 offline_packages.entity_counts 字段 */
export function parseEntityCounts(raw: string | null): Record<string, number> {
  return safeJsonParse<Record<string, number>>(raw, {});
}

/** 解析 pending_queue.payload 字段（异常时不抛错） */
export function parseQueuePayload(raw: string): Record<string, unknown> {
  return safeJsonParse<Record<string, unknown>>(raw, {});
}

/** 同步状态展示文案 */
export const SYNC_STATE_LABEL: Record<SyncState, string> = {
  local: '仅本地',
  pending: '待同步',
  synced: '已同步',
  conflict: '冲突',
};
