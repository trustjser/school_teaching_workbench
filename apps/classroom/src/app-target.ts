import type { AppTarget } from '@shared/app-target';

/**
 * 班级端入口的固定 target。
 *
 * 这是编译期常量：班级端安装包永远以 `client` 角色运行，不从 URL、localStorage
 * 或数据库读取，也不提供任何切换入口。
 */
export const APP_TARGET: AppTarget = 'classroom';
