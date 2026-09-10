import { invoke, isTauri as _isTauri } from '@tauri-apps/api/core';
import type { ErrorCode } from '@shared/types/enums';

/* =============================================================================
 * 前端唯一 invoke 出口。禁止在其它文件中裸调 invoke()。
 *
 * ---------------------------------------------------------------------------
 * 【本文件调用的全部 Tauri 命令名清单】（供 Rust 侧对账）
 *
 * —— A. 来自 docs/03-tasks.md §5.2 / docs/01-architecture.md，已锁定 ——
 *  1  settings_get_all            () -> AppSetting[]
 *  2  settings_set                ({ key, value, valueType }) -> void
 *  3  settings_complete_setup     ({ mode, deviceName, grade, className, classId?, schoolName, secret }) -> void
 *  4  settings_switch_mode        ({ mode }) -> AppMode
 *  5  settings_rotate_key         () -> { kid }
 *  6  student_list                ({ className?, status?, keyword?, includeDeleted? }) -> Student[]
 *  7  student_upsert              (Student) -> Student
 *  8  student_batch_import        ({ rows, batchName }) -> ImportReport
 *  9  student_update_status       ({ id, status }) -> Student
 * 10  checkin_list                ({ date, period }) -> CheckinRecord[]
 * 11  checkin_mark                ({ studentId, date, period, state, note? }) -> CheckinRecord
 * 12  checkin_batch_mark          ({ items }) -> CheckinRecord[]
 * 13  checkin_daily_summary       ({ date }) -> DailySummary[]
 * 14  task_list                   ({ status? }) -> CustomTask[]
 * 15  task_upsert                 (CustomTask) -> CustomTask
 * 16  task_node_upsert            (TaskStatusNode) -> TaskStatusNode
 * 17  task_node_delete            ({ id }) -> void
 * 18  task_record_upsert          (TaskRecord) -> TaskRecord
 * 19  task_matrix_query           ({ taskId }) -> TaskMatrix
 * 20  broadcast_create            (BroadcastTask) -> BroadcastTask
 * 21  broadcast_send              ({ id, targets: deviceId[] }) -> SendReport
 * 22  broadcast_list              ({ direction }) -> BroadcastTask[]
 * 23  broadcast_receipts          ({ broadcastTaskId }) -> BroadcastReceipt[]
 * 24  broadcast_accept            ({ broadcastTaskId }) -> CustomTask
 * 25  device_list                 () -> Device[]
 * 26  device_refresh              () -> Device[]
 * 27  device_forget               ({ deviceId }) -> void
 * 28  sync_queue_list             ({ status? }) -> PendingQueueItem[]
 * 29  sync_flush                  () -> { sent, failed }
 * 30  sync_retry                  ({ id }) -> void
 * 31  sync_log_list               ({ limit }) -> SyncLogEntry[]
 * 32  package_export_sch          ({ scope, sinceTs, path }) -> ExportSchResult
 * 33  package_import_sch          ({ path }) -> ImportReport
 *
 * —— B. 文档未定义、按同一命名风格补充（<domain>_<action>，domain 取自
 *      settings|student|checkin|task|broadcast|device|sync|package）——
 * 34  task_node_list              ({ taskId }) -> TaskStatusNode[]
 *       理由：状态节点编辑器需要独立读取节点列表；task_matrix_query 的返回
 *             在「草稿任务（无学生记录）」场景下无法稳定携带节点。
 * 35  task_delete                 ({ id }) -> void
 *       理由：任务列表的删除操作（软删）。
 * 36  student_delete              ({ id }) -> void
 *       理由：名册表格的删除操作（软删，区别于 update_status 转出）。
 * 37  checkin_school_summary      ({ date }) -> SchoolSummary
 *       理由：教务处端首页/大屏需要全校汇总（出勤率、班级数、已提交数）。
 * 38  checkin_class_attendance    ({ date }) -> ClassAttendanceRow[]
 *       理由：全校考勤大屏按班级聚合，含「是否已提交」标记。
 * 39  checkin_exception_students  ({ date, className? }) -> ExceptionStudentRow[]
 *       理由：异常学生名单（缺勤/请假）需跨班级查询，教务处端本地库才有全量。
 * 40  task_completion_stats       ({ sinceTs? }) -> TaskCompletionRow[]
 *       理由：教务处端「任务完成率」统计与导出。
 * 41  package_list                () -> OfflinePackage[]
 *       理由：离线包导出/导入历史展示。
 * 42  settings_key_info           () -> { kid, fingerprint }
 *       理由：设置页展示当前共享密钥标识与指纹，便于人工核对各端是否一致。
 *
 * —— C. 年级 / 班级目录（教务端统一维护，班级端消费）——
 * 43  grade_list                 () -> Grade[]
 *       理由：教务端管理年级列表；班级端首次同步后本地只读消费。
 * 44  grade_upsert               (Grade) -> Grade
 *       理由：新增 / 修改年级，写入待发队列同步到班级端。
 * 45  grade_delete               ({ id }) -> void
 *       理由：软删年级（其下班级 grade_id 置空）。
 * 46  class_list                 ({ gradeId? }) -> Class[]
 *       理由：班级列表（可按年级过滤）。
 * 47  class_upsert               (Class) -> Class
 *       理由：新增 / 修改班级（grade_id 关联年级，冗余年级名），写入待发队列。
 * 48  class_delete               ({ id }) -> void
 *       理由：软删班级。
 *
 * 参数与返回字段一律 camelCase（与 Rust #[serde(rename_all = "camelCase")] 对齐），
 * 时间为 number 毫秒时间戳，ID 为 string UUID。
 *
 * ---------------------------------------------------------------------------
 * 【本文件不发送任何 Tauri 事件】——按 docs/03-tasks.md §4.4，事件仅用于
 * Rust → 前端单向推送，前端一律走 invoke 命令。
 * ========================================================================== */

/** 统一错误类型 */
export class AppError extends Error {
  readonly code: ErrorCode | string;
  readonly raw: unknown;

  constructor(code: ErrorCode | string, message: string, raw?: unknown) {
    super(message);
    this.name = 'AppError';
    this.code = code;
    this.raw = raw;
  }
}

/** 当前是否运行在 Tauri 壳内 */
export function isTauriRuntime(): boolean {
  try {
    if (typeof _isTauri === 'function') {
      return Boolean(_isTauri());
    }
  } catch {
    // 忽略，走下面的兜底探测
  }
  if (typeof window === 'undefined') return false;
  const w = window as unknown as Record<string, unknown>;
  return Boolean(w.__TAURI_INTERNALS__ || w.__TAURI__);
}

/** Rust 侧错误负载的可能形态 */
function normalizeError(err: unknown): AppError {
  if (err instanceof AppError) return err;

  // Tauri v2 会把 Result::Err 序列化为 { code, message } 或纯字符串
  if (err && typeof err === 'object') {
    const obj = err as Record<string, unknown>;
    const code = typeof obj.code === 'string' ? obj.code : undefined;
    const message = typeof obj.message === 'string' ? obj.message : undefined;
    if (code || message) {
      return new AppError(code ?? 'ERR_UNKNOWN', message ?? '发生未知错误', err);
    }
    if (typeof obj.error === 'string') {
      return new AppError('ERR_UNKNOWN', obj.error, err);
    }
  }
  if (typeof err === 'string') {
    // "command xxx not found" 之类的运行时错误
    if (err.includes('not found') || err.includes('not allowed')) {
      return new AppError('ERR_MODE', `本地命令不可用：${err}`, err);
    }
    return new AppError('ERR_UNKNOWN', err, err);
  }
  return new AppError('ERR_UNKNOWN', '发生未知错误', err);
}

export interface InvokeOptions {
  /** 为 true 时不由调用方 toast（已由上层统一处理），默认 false */
  silent?: boolean;
  /** 自定义错误上下文，便于排障 */
  context?: string;
}

/**
 * invoke 封装：统一错误归一化。
 * @param cmd 命令名（见文件头清单）
 * @param args 参数对象（camelCase）
 */
export async function invokeCmd<T>(
  cmd: string,
  args: Record<string, unknown> = {},
  options: InvokeOptions = {},
): Promise<T> {
  if (!isTauriRuntime()) {
    throw new AppError(
      'ERR_TAURI',
      '未能连接到本地服务（Tauri 运行时不可用），请通过应用窗口启动',
      cmd,
    );
  }
  try {
    const result = await invoke<T>(cmd, args);
    return result;
  } catch (err) {
    const appErr = normalizeError(err);
    if (import.meta.env.VITE_DEBUG === 'true') {
      // eslint-disable-next-line no-console
      console.error(`[invoke:${cmd}]`, options.context ?? '', appErr);
    }
    throw appErr;
  }
}

/** 静默版本：捕获错误并返回 fallback，不抛出（用于可选能力探测） */
export async function invokeCmdSafe<T>(
  cmd: string,
  args: Record<string, unknown> = {},
  fallback: T,
): Promise<T> {
  try {
    return await invokeCmd<T>(cmd, args, { silent: true });
  } catch {
    return fallback;
  }
}

/** 判断是否为「命令未实现」类错误（用于优雅降级） */
export function isCommandMissing(err: unknown): boolean {
  if (!(err instanceof AppError)) return false;
  return err.code === 'ERR_MODE' && String(err.message).includes('本地命令不可用');
}
