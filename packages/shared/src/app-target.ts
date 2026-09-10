/**
 * 固定 app target。
 *
 * 本仓库构建两个独立 app：教务端（affairs）与班级端（classroom）。每个入口在
 * 编译期就固定自己的 target，不从 URL、localStorage 或数据库读取，也**不提供**
 * 运行时切换能力。共享模块只能消费 target，不得在 target 缺失时默认成任一端。
 */

export type AppTarget = 'affairs' | 'classroom';

/** 教务端固定映射 Rust `master`，班级端固定映射 Rust `client`。 */
export type AppMode = 'master' | 'client';

export const APP_MODE_BY_TARGET = {
  affairs: 'master',
  classroom: 'client',
} as const satisfies Record<AppTarget, AppMode>;

/** 由固定 target 推导运行模式。 */
export function appModeForTarget(target: AppTarget): AppMode {
  return APP_MODE_BY_TARGET[target];
}

/** 端显示名（顶栏 / 侧栏 / 标题使用）。 */
export const APP_TARGET_LABEL = {
  affairs: '教务端',
  classroom: '班级端',
} as const satisfies Record<AppTarget, string>;

/** 侧栏品牌头副标题。 */
export const APP_TARGET_SUBTITLE = {
  affairs: '教务处协同端',
  classroom: '班级协同端',
} as const satisfies Record<AppTarget, string>;
