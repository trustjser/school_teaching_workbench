import type { ErrorCode } from '@/types/enums';

/** 错误码元信息：中文提示 + 是否可重试 + 建议操作 */
export interface ErrorMeta {
  code: ErrorCode | string;
  message: string;
  retryable: boolean;
  suggestion: string;
}

/**
 * 错误码表（与 docs/03-tasks.md §4.3 完全对齐）。
 * Toast 直接查表展示。
 */
export const ERROR_META: Record<string, ErrorMeta> = {
  ERR_DB: {
    code: 'ERR_DB',
    message: '数据库操作失败，请重试',
    retryable: true,
    suggestion: '稍后重试；若持续失败，请在设置页导出 .sch 包备份后重启应用。',
  },
  ERR_NET: {
    code: 'ERR_NET',
    message: '网络不可达，已加入待发队列',
    retryable: true,
    suggestion: '数据已保存在本机，恢复网络后会自动补发，无需重做。',
  },
  ERR_SIGN: {
    code: 'ERR_SIGN',
    message: '签名校验失败，请检查共享密钥',
    retryable: false,
    suggestion: '请确认各设备使用同一份共享密钥（设置页可查看密钥指纹）。',
  },
  ERR_CRYPTO: {
    code: 'ERR_CRYPTO',
    message: '报文解密失败，数据可能已损坏',
    retryable: false,
    suggestion: '请检查密钥是否一致，或重新导出 / 导入离线包。',
  },
  ERR_TS_WINDOW: {
    code: 'ERR_TS_WINDOW',
    message: '时间偏差过大，请校准本机时间',
    retryable: false,
    suggestion: '请将本机时间与教务处端对齐（偏差需小于 5 分钟）。',
  },
  ERR_NONCE_REPLAY: {
    code: 'ERR_NONCE_REPLAY',
    message: '检测到重复请求，已拒绝',
    retryable: false,
    suggestion: '该请求已被安全策略拦截，无需处理；若频繁出现请联系管理员。',
  },
  ERR_VALIDATION: {
    code: 'ERR_VALIDATION',
    message: '参数校验失败',
    retryable: false,
    suggestion: '请检查填写内容：评分为 0–100 整数，状态节点为 2–4 个。',
  },
  ERR_NOT_FOUND: {
    code: 'ERR_NOT_FOUND',
    message: '记录不存在或已删除',
    retryable: false,
    suggestion: '请刷新列表后重试。',
  },
  ERR_MODE: {
    code: 'ERR_MODE',
    message: '当前运行模式不支持该操作',
    retryable: false,
    suggestion: '该操作仅教务处端可用，请在设置页切换运行模式。',
  },
  ERR_PERMISSION: {
    code: 'ERR_PERMISSION',
    message: '无文件或网络权限',
    retryable: false,
    suggestion: '请允许应用访问网络（5353/UDP、5178/TCP）与目标文件夹。',
  },
  ERR_IMPORT: {
    code: 'ERR_IMPORT',
    message: '导入数据有误，请查看错误报告',
    retryable: false,
    suggestion: '请按错误报告修正表格后重新导入。',
  },
  ERR_UNKNOWN: {
    code: 'ERR_UNKNOWN',
    message: '发生未知错误',
    retryable: false,
    suggestion: '请重试；若持续失败请记录操作路径并联系管理员。',
  },
  ERR_TAURI: {
    code: 'ERR_TAURI',
    message: '未能连接到本地服务（Tauri 运行时不可用）',
    retryable: true,
    suggestion: '请通过 tauri dev / 安装包启动应用，而不是直接用浏览器打开。',
  },
};

/** 取错误元信息，未知错误码兜底为 ERR_UNKNOWN */
export function getErrorMeta(code: string | undefined | null): ErrorMeta {
  if (code && ERROR_META[code]) return ERROR_META[code];
  return ERROR_META.ERR_UNKNOWN;
}

/** 取中文提示文案 */
export function getErrorMessage(code: string | undefined | null, fallback?: string): string {
  if (!code) return fallback || ERROR_META.ERR_UNKNOWN.message;
  return getErrorMeta(code).message;
}
