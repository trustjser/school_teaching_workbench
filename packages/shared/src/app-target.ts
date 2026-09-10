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

/**
 * 运行期 app target 持有者。
 *
 * 每个 app 入口在挂载前调用 `setAppTarget(APP_TARGET)` 注入自己的固定常量；
 * shared 层（bootstrap、布局等）通过 `getAppTarget()` 只读消费。这样 shared
 * 既不需要反向依赖某个 app，也不会在 target 缺失时静默默认成任一端。
 */
let runtimeTarget: AppTarget | null = null;

/** 由入口注入固定 target；重复注入不同值直接抛错（说明打包配置串了）。 */
export function setAppTarget(target: AppTarget): void {
  if (runtimeTarget !== null && runtimeTarget !== target) {
    throw new Error(`app target 已固定为 ${runtimeTarget}，不能再设为 ${target}`);
  }
  runtimeTarget = target;
}

/** 读取固定 target。入口尚未注入时抛错，绝不回退默认值。 */
export function getAppTarget(): AppTarget {
  if (runtimeTarget === null) {
    throw new Error('app target 尚未注入：入口必须先调用 setAppTarget(APP_TARGET)');
  }
  return runtimeTarget;
}
