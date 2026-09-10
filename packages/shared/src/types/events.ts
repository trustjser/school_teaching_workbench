import type { AppMode } from './enums';
import type { Device } from './models';
import type { ImportReport } from './api';
import type { BroadcastReceipt, BroadcastTask } from './broadcast';

/**
 * Tauri 事件名常量（Rust → 前端，单向）。
 * 命名与 docs/03-tasks.md §4.4 完全一致，禁止变更。
 * 前端不向 Rust 发事件（一律走 invoke 命令），避免双向事件循环。
 */
export const TAURI_EVENTS = {
  DEVICE_FOUND: 'device://found',
  DEVICE_LOST: 'device://lost',
  DEVICE_HEARTBEAT: 'device://heartbeat',
  DEVICE_OFFLINE: 'device://offline',
  SYNC_PROGRESS: 'sync://progress',
  SYNC_ERROR: 'sync://error',
  CHECKIN_UPDATED: 'checkin://updated',
  TASK_UPDATED: 'task://updated',
  BROADCAST_RECEIVED: 'broadcast://received',
  BROADCAST_RECEIPT: 'broadcast://receipt',
  DATA_IMPORTED: 'data://imported',
  CLASS_CHANGED: 'class://changed',
  CLASSROOM_CHANGED: 'classroom://changed',
  STUDENT_CHANGED: 'student://changed',
  MODE_CHANGED: 'mode://changed',
  PACKAGE_PROGRESS: 'package://progress',
} as const;

export type TauriEventName = (typeof TAURI_EVENTS)[keyof typeof TAURI_EVENTS];

/** device://found */
export interface DeviceFoundPayload extends Device {}

/** device://lost / device://offline */
export interface DeviceIdPayload {
  deviceId: string;
}

/** device://heartbeat */
export interface DeviceHeartbeatPayload {
  deviceId: string;
  latencyMs: number;
  ts: number;
}

/** sync://progress */
export interface SyncProgressPayload {
  pending: number;
  sending: number;
  lastError?: string | null;
}

/** sync://error */
export interface SyncErrorPayload {
  queueId: string;
  code: string;
}

/** checkin://updated */
export interface CheckinUpdatedPayload {
  date: string;
  period: string;
  classId: string | null;
}

/** task://updated */
export interface TaskUpdatedPayload {
  taskId: string;
}

/** broadcast://received */
export interface BroadcastReceivedPayload extends BroadcastTask {}

/** broadcast://receipt */
export interface BroadcastReceiptPayload extends BroadcastReceipt {}

/** data://imported */
export interface DataImportedPayload extends ImportReport {}
export interface ClassChangedPayload { id?: string }
export interface ClassroomChangedPayload { id?: string }
export interface StudentChangedPayload { id?: string }

/** mode://changed */
export interface ModeChangedPayload {
  mode: AppMode;
}

/** package://progress */
export interface PackageProgressPayload {
  current: number;
  total: number;
  phase: string;
}

/** 事件名 → Payload 类型映射 */
export interface TauriEventMap {
  [TAURI_EVENTS.DEVICE_FOUND]: DeviceFoundPayload;
  [TAURI_EVENTS.DEVICE_LOST]: DeviceIdPayload;
  [TAURI_EVENTS.DEVICE_HEARTBEAT]: DeviceHeartbeatPayload;
  [TAURI_EVENTS.DEVICE_OFFLINE]: DeviceIdPayload;
  [TAURI_EVENTS.SYNC_PROGRESS]: SyncProgressPayload;
  [TAURI_EVENTS.SYNC_ERROR]: SyncErrorPayload;
  [TAURI_EVENTS.CHECKIN_UPDATED]: CheckinUpdatedPayload;
  [TAURI_EVENTS.TASK_UPDATED]: TaskUpdatedPayload;
  [TAURI_EVENTS.BROADCAST_RECEIVED]: BroadcastReceivedPayload;
  [TAURI_EVENTS.BROADCAST_RECEIPT]: BroadcastReceiptPayload;
  [TAURI_EVENTS.DATA_IMPORTED]: DataImportedPayload;
  [TAURI_EVENTS.CLASS_CHANGED]: ClassChangedPayload;
  [TAURI_EVENTS.CLASSROOM_CHANGED]: ClassroomChangedPayload;
  [TAURI_EVENTS.STUDENT_CHANGED]: StudentChangedPayload;
  [TAURI_EVENTS.MODE_CHANGED]: ModeChangedPayload;
  [TAURI_EVENTS.PACKAGE_PROGRESS]: PackageProgressPayload;
}
