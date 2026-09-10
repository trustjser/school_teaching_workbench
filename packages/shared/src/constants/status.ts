import type { CheckinState, StudentStatus } from '@/types/enums';

/** 状态显示元数据：emoji + 中文文案 + 高对比配色（色盲友好：颜色 + 图标 + 文字） */
export interface StatusMeta {
  key: string;
  label: string;
  /** emoji 色块，用于大屏远距离识别 */
  emoji: string;
  /** 主色（十六进制，与 docs/03-tasks.md §4.7 一致） */
  color: string;
  /** Tailwind 背景类 */
  bgClass: string;
  /** Tailwind 文字类 */
  textClass: string;
  /** Tailwind 边框类 */
  borderClass: string;
}

/** 考勤状态元数据表 */
export const CHECKIN_STATUS_META: Record<CheckinState, StatusMeta> = {
  present: {
    key: 'present',
    label: '出勤',
    emoji: '🟢',
    color: '#16a34a',
    bgClass: 'bg-green-50',
    textClass: 'text-green-800',
    borderClass: 'border-green-600',
  },
  leave: {
    key: 'leave',
    label: '请假',
    emoji: '🟡',
    color: '#ca8a04',
    bgClass: 'bg-amber-50',
    textClass: 'text-amber-800',
    borderClass: 'border-amber-600',
  },
  absent: {
    key: 'absent',
    label: '缺勤',
    emoji: '🔴',
    color: '#dc2626',
    bgClass: 'bg-red-50',
    textClass: 'text-red-800',
    borderClass: 'border-red-600',
  },
  late: {
    key: 'late',
    label: '迟到',
    emoji: '🟠',
    color: '#ea580c',
    bgClass: 'bg-orange-50',
    textClass: 'text-orange-800',
    borderClass: 'border-orange-600',
  },
};

/**
 * 考勤点击循环顺序（反向标记核心）：
 *   present(🟢出勤) → leave(🟡请假) → absent(🔴缺勤) → present(🟢出勤)
 * late 为显式第四态：通过长按 / 右键 / 详情面板单独设置；
 * 若当前已是 late，再次点击回到 present。
 */
export const CHECKIN_CYCLE: CheckinState[] = ['present', 'leave', 'absent'];

/** 参与循环的 + late 的完整枚举顺序（用于图例与统计展示） */
export const CHECKIN_STATES_ALL: CheckinState[] = ['present', 'leave', 'absent', 'late'];

/** 计算考勤状态的下一个状态 */
export function nextCheckinState(current: CheckinState): CheckinState {
  if (current === 'late') return 'present';
  const idx = CHECKIN_CYCLE.indexOf(current);
  if (idx < 0) return 'leave';
  return CHECKIN_CYCLE[(idx + 1) % CHECKIN_CYCLE.length];
}

/** 学生名册状态元数据 */
export const STUDENT_STATUS_META: Record<StudentStatus, StatusMeta> = {
  active: {
    key: 'active',
    label: '在读',
    emoji: '🟢',
    color: '#16a34a',
    bgClass: 'bg-green-50',
    textClass: 'text-green-800',
    borderClass: 'border-green-600',
  },
  leave: {
    key: 'leave',
    label: '请假',
    emoji: '🟡',
    color: '#ca8a04',
    bgClass: 'bg-amber-50',
    textClass: 'text-amber-800',
    borderClass: 'border-amber-600',
  },
  transferred: {
    key: 'transferred',
    label: '已转出',
    emoji: '⚪',
    color: '#64748b',
    bgClass: 'bg-surface-muted',
    textClass: 'text-ink-soft',
    borderClass: 'border-surface-border',
  },
};

export const STUDENT_STATUSES: StudentStatus[] = ['active', 'leave', 'transferred'];

/** 考勤时段 */
export const PERIOD_OPTIONS: { value: string; label: string }[] = [
  { value: 'am', label: '上午' },
  { value: 'pm', label: '下午' },
  { value: 'all', label: '全天' },
  { value: 'custom', label: '自定义时段' },
];

/** 状态节点配色 token → 十六进制主色（与 tailwind.config.js node 色板一致） */
export const COLOR_TOKEN_HEX: Record<string, string> = {
  slate: '#475569',
  blue: '#2563eb',
  amber: '#ca8a04',
  emerald: '#16a34a',
  rose: '#e11d48',
  violet: '#7c3aed',
  cyan: '#0891b2',
  orange: '#ea580c',
};

/** 状态节点配色 token → 中文名 */
export const COLOR_TOKEN_LABEL: Record<string, string> = {
  slate: '石板灰',
  blue: '蓝色',
  amber: '琥珀',
  emerald: '翠绿',
  rose: '玫红',
  violet: '紫罗兰',
  cyan: '青色',
  orange: '橙色',
};

export const COLOR_TOKENS: string[] = [
  'slate',
  'blue',
  'amber',
  'emerald',
  'rose',
  'violet',
  'cyan',
  'orange',
];

/** 状态节点可用图标（key 与 lucide 组件映射见 components/ui 的 ICON_MAP） */
export const NODE_ICON_OPTIONS: { value: string; label: string }[] = [
  { value: 'circle', label: '圆圈' },
  { value: 'dot', label: '圆点' },
  { value: 'clock', label: '时钟' },
  { value: 'check', label: '对勾' },
  { value: 'x', label: '叉号' },
  { value: 'minus', label: '横线' },
  { value: 'star', label: '星标' },
  { value: 'flag', label: '旗帜' },
  { value: 'book', label: '书本' },
  { value: 'award', label: '奖章' },
  { value: 'thumbs-up', label: '点赞' },
  { value: 'hourglass', label: '沙漏' },
];

/** 默认状态节点模板（新建任务 / 广播下发时预置） */
export const DEFAULT_NODE_TEMPLATES: {
  nodeKey: string;
  label: string;
  colorToken: string;
  iconName: string;
  isFinal: boolean;
  isDefault: boolean;
}[] = [
  {
    nodeKey: 'todo',
    label: '未开始',
    colorToken: 'slate',
    iconName: 'circle',
    isFinal: false,
    isDefault: true,
  },
  {
    nodeKey: 'doing',
    label: '进行中',
    colorToken: 'amber',
    iconName: 'hourglass',
    isFinal: false,
    isDefault: false,
  },
  {
    nodeKey: 'done',
    label: '已完成',
    colorToken: 'emerald',
    iconName: 'check',
    isFinal: true,
    isDefault: false,
  },
];

/** 任务类型选项 */
export const TASK_TYPE_OPTIONS: { value: string; label: string }[] = [
  { value: 'custom', label: '自定义' },
  { value: 'recitation', label: '背诵' },
  { value: 'homework', label: '作业' },
  { value: 'temperature', label: '体温' },
  { value: 'checkin', label: '签到' },
  { value: 'other', label: '其他' },
];

/** 任务状态选项 */
export const TASK_STATUS_OPTIONS: { value: string; label: string }[] = [
  { value: 'draft', label: '草稿' },
  { value: 'active', label: '进行中' },
  { value: 'closed', label: '已结束' },
  { value: 'archived', label: '已归档' },
];

/** 教务任务下发的生命周期状态（对应 broadcast_tasks.status）。 */
export const BROADCAST_STATUS_OPTIONS: { value: string; label: string }[] = [
  { value: 'draft', label: '草稿' },
  { value: 'sending', label: '发送中' },
  { value: 'sent', label: '已发送' },
  { value: 'partial', label: '部分完成' },
  { value: 'closed', label: '已关闭' },
  { value: 'cancelled', label: '已取消' },
];

/** 广播优先级选项 */
export const PRIORITY_OPTIONS: { value: string; label: string }[] = [
  { value: 'low', label: '普通' },
  { value: 'normal', label: '常规' },
  { value: 'high', label: '重要' },
  { value: 'urgent', label: '紧急' },
];

/** 设备状态元数据 */
export const DEVICE_STATUS_META: Record<string, { label: string; color: string; emoji: string }> =
  {
    online: { label: '在线', color: '#16a34a', emoji: '🟢' },
    offline: { label: '离线', color: '#64748b', emoji: '⚪' },
    stale: { label: '待确认', color: '#ca8a04', emoji: '🟡' },
    blocked: { label: '已忽略', color: '#dc2626', emoji: '🔴' },
  };
