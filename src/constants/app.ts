/**
 * 核心常量。
 * 取值必须与 src-tauri/src/config/constants.rs 保持一致（docs/03-tasks.md §4.5）。
 */

/** P2P API 监听端口（被占用时 Rust 侧向后探测 5179–5188） */
export const API_PORT = 5178;

/** 端口探测上限 */
export const API_PORT_MAX = 5188;

/** mDNS 服务类型 */
export const MDNS_SERVICE_TYPE = '_schworkbench._tcp.local.';

/** mDNS 标准端口（UDP） */
export const MDNS_PORT = 5353;

/** API 版本前缀 */
export const API_VERSION = 'v1';

/** API 路径前缀 */
export const API_PREFIX = `/api/${API_VERSION}`;

/** 心跳周期（秒） */
export const HEARTBEAT_INTERVAL_SEC = 15;

/** 设备离线判定阈值（秒） */
export const OFFLINE_TTL_SEC = 45;

/** HMAC 时间戳容差窗口（秒） */
export const HMAC_TS_WINDOW_SEC = 300;

/** nonce TTL（秒）= 2 × 窗口 */
export const NONCE_TTL_SEC = 600;

/** 离线队列最大重试次数 */
export const QUEUE_MAX_ATTEMPTS = 5;

/** 退避基数（毫秒） */
export const BACKOFF_BASE_MS = 2000;

/** 退避上限（毫秒） */
export const BACKOFF_CAP_MS = 300000;

/** Vite dev server 端口 */
export const VITE_DEV_PORT = 1420;

/** 应用显示名 */
export const APP_NAME = '教务与班级协同工作台';

/** 应用版本 */
export const APP_VERSION = '1.0.0';

/** 自动补发触发周期（毫秒）——useAutoSync 定时 flush */
export const AUTO_SYNC_INTERVAL_MS = 15000;

/** 设备列表自动刷新周期（毫秒） */
export const DEVICE_REFRESH_INTERVAL_MS = 20000;

/** 大屏 refresh：教务处大屏轮询周期（毫秒） */
export const BOARD_REFRESH_INTERVAL_MS = 20000;

/** 评分范围 */
export const SCORE_MIN = 0;
export const SCORE_MAX = 100;

/** 备注最大长度 */
export const NOTE_MAX_LENGTH = 500;

/** 任务状态节点数量约束 */
export const TASK_NODE_MIN = 2;
export const TASK_NODE_MAX = 4;

/** 单页/单批渲染阈值：超过则启用简易窗口渲染（避免 1200 格卡顿） */
export const VIRTUAL_RENDER_THRESHOLD = 400;

/** 队列优先级（数值越小越优先，与 DDL priority 语义一致） */
export const QUEUE_PRIORITY = {
  checkin: 1,
  broadcast: 2,
  task: 3,
  student: 5,
} as const;

/** 本地存储键（仅 UI 偏好，业务数据一律落 SQLite） */
export const LS_KEYS = {
  uiScale: 'lan-workbench:ui-scale',
  theme: 'lan-workbench:theme',
  lastCheckinDate: 'lan-workbench:last-checkin-date',
  lastMatrixView: 'lan-workbench:last-matrix-view',
} as const;

/** mDNS TXT Record 键 */
export const MDNS_TXT_KEYS = {
  version: 'v',
  role: 'role',
  grade: 'grade',
  className: 'class',
  apiVersion: 'api',
  keyId: 'kid',
} as const;

/** 默认 app_settings 值（首次启动、DB 未就绪时的兜底） */
export const DEFAULT_SETTINGS = {
  appMode: 'client',
  apiPort: API_PORT,
  mdnsServiceType: MDNS_SERVICE_TYPE,
  uiScale: 1.25,
  theme: 'light',
  heartbeatInterval: HEARTBEAT_INTERVAL_SEC,
  offlineTtlSec: OFFLINE_TTL_SEC,
  hmacTsWindowSec: HMAC_TS_WINDOW_SEC,
  queueMaxAttempts: QUEUE_MAX_ATTEMPTS,
} as const;
