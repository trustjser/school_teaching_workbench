/** UI 常量：大屏断点、缩放档位、卡片尺寸、动效时长 */

/** 大屏断点（px） */
export const BREAKPOINTS = {
  /** 笔记本 */
  laptop: 1280,
  /** 教室一体机 */
  board: 1600,
  /** 大屏 / 投影 */
  wall: 1920,
} as const;

/** UI 缩放档位 */
export const UI_SCALE_OPTIONS: { value: number; label: string }[] = [
  { value: 1, label: '标准 100%' },
  { value: 1.25, label: '较大 125%' },
  { value: 1.5, label: '大屏 150%' },
];

/** 缩放档位边界：屏幕宽度 ≥ wall 用 1.5，≥ board 用 1.25，否则 1.0 */
export const UI_SCALE_RULES: { minWidth: number; scale: number }[] = [
  { minWidth: BREAKPOINTS.wall, scale: 1.5 },
  { minWidth: BREAKPOINTS.board, scale: 1.25 },
  { minWidth: 0, scale: 1 },
];

/** 触控目标最小尺寸（px） */
export const TOUCH_TARGET = 44;

/** 考勤网格卡片尺寸档位 */
export const GRID_CARD_SIZE = {
  /** 紧凑（≥60 人班级） */
  compact: { minWidth: 132, padding: 'p-2', nameSize: 'text-base', emojiSize: 'text-2xl' },
  /** 标准（30–59 人） */
  normal: { minWidth: 160, padding: 'p-3', nameSize: 'text-lg', emojiSize: 'text-3xl' },
  /** 大屏（≤29 人 / 大屏模式） */
  large: { minWidth: 200, padding: 'p-4', nameSize: 'text-2xl', emojiSize: 'text-4xl' },
} as const;

export type GridCardSizeKey = keyof typeof GRID_CARD_SIZE;

/** 依据人数与是否大屏推断卡片档位 */
export function resolveGridCardSize(count: number, bigScreen: boolean): GridCardSizeKey {
  if (bigScreen && count <= 36) return 'large';
  if (count >= 56) return 'compact';
  return 'normal';
}

/** Toast 展示时长（毫秒） */
export const TOAST_DURATION = {
  success: 2200,
  info: 2600,
  warning: 4000,
  error: 6000,
} as const;

/** 防抖时长（毫秒） */
export const DEBOUNCE_MS = {
  search: 260,
  note: 500,
  score: 300,
} as const;

/** 进度条动画步长（毫秒） */
export const PROGRESS_TICK_MS = 120;

/** 侧边导航宽度 */
export const SIDENAV_WIDTH = {
  expanded: 248,
  collapsed: 76,
} as const;

/** 表格行高（Tailwind class） */
export const TABLE_ROW_CLASS = 'h-14 min-h-touch';

/** 空状态插画尺寸 */
export const EMPTY_ILLUSTRATION_SIZE = { width: 240, height: 160 } as const;
