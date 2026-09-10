import type { AppTarget } from '@shared/app-target';

/**
 * 教务端入口的固定 target。
 *
 * 这是编译期常量：教务端安装包永远以 `master` 角色运行，不从 URL、localStorage
 * 或数据库读取，也不提供任何切换入口。
 */
export const APP_TARGET: AppTarget = 'affairs';
