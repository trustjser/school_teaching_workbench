import type {
  BroadcastDirection,
  BroadcastPriority,
  BroadcastStatus,
  BroadcastTargetType,
  ColorToken,
  ReceiptStatus,
} from './enums';
import type { BaseEntity, Device } from './models';

/** 展开目标时使用的设备最小信息 */
export type TargetableDevice = Pick<Device, 'deviceId' | 'txtGrade' | 'txtClassName' | 'isSelf'>;

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
  /**
   * 派生字段（非表列）：是否已成功投递给至少一个班级端。
   *
   * 由后端查询算出。界面据此决定给「取消」（尚未送达，可撤回）还是「关闭」。
   */
  delivered: boolean;
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

/**
 * 将 UI 的目标选择器展开为设备 ID 列表。
 *
 * Rust 侧 `broadcast_send` 的 `targets` 只认 device_id 数组，选择器展开在前端完成：
 * - school：除本端外的全部设备
 * - grade：按设备广播的 txtGrade 匹配
 * - class：按设备广播的 txtClassName 匹配
 * - device：直接取已选设备（顺带剔除非在册的过期 ID）
 */
export function resolveTargetDeviceIds(selector: TargetSelector, devices: TargetableDevice[]): string[] {
  const values = new Set(selector.values);
  return devices
    .filter((d) => {
      if (d.isSelf) return false;
      switch (selector.targetType) {
        case 'school':
          return true;
        case 'grade':
          return !!d.txtGrade && values.has(d.txtGrade);
        case 'class':
          return !!d.txtClassName && values.has(d.txtClassName);
        case 'device':
          return values.has(d.deviceId);
        default:
          return false;
      }
    })
    .map((d) => d.deviceId);
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
