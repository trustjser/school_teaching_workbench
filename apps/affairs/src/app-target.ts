import type { AppTarget } from '@shared/app-target';

/**
 * 教务端入口的固定 target。
 *
 * 这是编译期常量：教务端安装包永远以 `master` 角色运行，不从 URL、localStorage
 * 或数据库读取，也不提供任何切换入口。
 */
export const APP_TARGET: AppTarget = 'affairs';

// 构建期校验：产物中的 VITE_APP_TARGET 必须与本入口的常量一致，
// 防止误用班级端配置打包教务端产物（两端产物共用同一套代码，只靠 define 区分）。
if (import.meta.env.VITE_APP_TARGET !== APP_TARGET) {
  throw new Error(
    `产物端标识不匹配：期望 ${APP_TARGET}，实际 ${String(import.meta.env.VITE_APP_TARGET)}`,
  );
}
