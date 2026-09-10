import type { AppTarget } from '@shared/app-target';

/**
 * 班级端入口的固定 target。
 *
 * 这是编译期常量：班级端安装包永远以 `client` 角色运行，不从 URL、localStorage
 * 或数据库读取，也不提供任何切换入口。
 */
export const APP_TARGET: AppTarget = 'classroom';

// 构建期校验：产物中的 VITE_APP_TARGET 必须与本入口的常量一致，
// 防止误用教务端配置打包班级端产物（两端产物共用同一套代码，只靠 define 区分）。
if (import.meta.env.VITE_APP_TARGET !== APP_TARGET) {
  throw new Error(
    `产物端标识不匹配：期望 ${APP_TARGET}，实际 ${String(import.meta.env.VITE_APP_TARGET)}`,
  );
}
