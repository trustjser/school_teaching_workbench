import type {
  BroadcastDirection,
  BroadcastPriority,
  BroadcastStatus,
  BroadcastTargetType,
  ColorToken,
  ReceiptStatus,
} from './enums';
import type { BaseEntity } from './models';

/** 广播 payload 中携带的任务模板定义（班级端一键生成待办时使用） */
export interface BroadcastTaskTemplate {
  title: string;
  description: string | null;
  taskType: string;
  dueAt: number | null;
  scoreEnabled: boolean;
  noteEnabled: boolean;
  viewMode: 'grid' | 'table';
  /** 2~4 个状态节点模板 */
  statusNodes: BroadcastNodeTemplate[];
}

/** 状态节点模板 */
export interface BroadcastNodeTemplate {
  nodeKey: string;
  label: string;
  colorToken: ColorToken;
  iconName: string | null;
  nodeOrder: number;
  isFinal: boolean;
  isDefault: boolean;
}

/** 教务处下发的广播任务 */
export interface BroadcastTask extends BaseEntity {
  title: string;
  description: string | null;
  /** JSON 字符串，反序列化后为 BroadcastTaskTemplate */
  payload: string;
  targetType: BroadcastTargetType;
  /** JSON 数组字符串：["3"] / ["三年级二班"] / ["<device_id>"] */
  targetValue: string | null;
  dueAt: number | null;
  priority: BroadcastPriority;
  publisherDeviceId: string;
  publisherName: string | null;
  direction: BroadcastDirection;
  status: BroadcastStatus;
  sentAt: number | null;
  closedAt: number | null;
  expectCount: number;
  ackCount: number;
}

/** 广播回执 */
export interface BroadcastReceipt extends BaseEntity {
  broadcastTaskId: string;
  deviceId: string;
  deviceName: string | null;
  className: string | null;
  status: ReceiptStatus;
  receivedAt: number | null;
  acceptedAt: number | null;
  localTaskId: string | null;
  failReason: string | null;
}

/** 目标选择器（下发页 UI 状态） */
export interface TargetSelector {
  targetType: BroadcastTargetType;
  /** 已选中的年级 / 班级 / 设备 ID 列表 */
  values: string[];
}

/** 下发结果报告 */
export interface SendReport {
  broadcastTaskId: string;
  expectCount: number;
  enqueued: number;
  skipped: number;
}

/** 回执汇总（教务处端面板用） */
export interface ReceiptSummary {
  total: number;
  received: number;
  accepted: number;
  rejected: number;
  done: number;
}

/** 解析后的广播 payload（带解析失败兜底） */
export function parseBroadcastPayload(raw: string): BroadcastTaskTemplate | null {
  if (!raw) return null;
  try {
    const parsed = JSON.parse(raw) as BroadcastTaskTemplate;
    if (!parsed || typeof parsed.title !== 'string') return null;
    if (!Array.isArray(parsed.statusNodes)) parsed.statusNodes = [];
    return parsed;
  } catch {
    return null;
  }
}

/** 将模板序列化为 payload 字符串 */
export function stringifyBroadcastPayload(template: BroadcastTaskTemplate): string {
  return JSON.stringify(template);
}
