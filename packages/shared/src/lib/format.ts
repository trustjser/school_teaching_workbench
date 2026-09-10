import type { CheckinState } from '@/types/enums';
import { CHECKIN_STATUS_META } from '@/constants/status';

/** 毫秒时间戳 → 'YYYY-MM-DD'（本地日历日，不做时区转换） */
export function toDateKey(ts: number | null | undefined): string {
  const d = ts == null ? new Date() : new Date(ts);
  const y = d.getFullYear();
  const m = `${d.getMonth() + 1}`.padStart(2, '0');
  const day = `${d.getDate()}`.padStart(2, '0');
  return `${y}-${m}-${day}`;
}

/** 'YYYY-MM-DD' → 毫秒时间戳（本地 00:00:00） */
export function fromDateKey(dateKey: string): number {
  const [y, m, d] = dateKey.split('-').map((v) => Number.parseInt(v, 10));
  return new Date(y || 1970, (m || 1) - 1, d || 1, 0, 0, 0, 0).getTime();
}

/** 今天 00:00 的时间戳 */
export function startOfToday(): number {
  const now = new Date();
  return new Date(now.getFullYear(), now.getMonth(), now.getDate(), 0, 0, 0, 0).getTime();
}

/** 毫秒时间戳 → 'HH:mm' */
export function formatTime(ts: number | null | undefined): string {
  if (ts == null) return '--:--';
  const d = new Date(ts);
  return `${`${d.getHours()}`.padStart(2, '0')}:${`${d.getMinutes()}`.padStart(2, '0')}`;
}

/** 毫秒时间戳 → 'YYYY-MM-DD HH:mm' */
export function formatDateTime(ts: number | null | undefined): string {
  if (ts == null) return '--';
  return `${toDateKey(ts)} ${formatTime(ts)}`;
}

/** 'YYYY-MM-DD' → 中文日期，如「2026年9月8日 周二」 */
export function formatDateCN(dateKey: string): string {
  const ts = fromDateKey(dateKey);
  const d = new Date(ts);
  const week = ['周日', '周一', '周二', '周三', '周四', '周五', '周六'][d.getDay()];
  return `${d.getFullYear()}年${d.getMonth() + 1}月${d.getDate()}日 ${week}`;
}

/** 相对时间：刚刚 / N 分钟前 / N 小时前 / 日期 */
export function formatRelative(ts: number | null | undefined): string {
  if (ts == null) return '从未';
  const diff = Date.now() - ts;
  if (diff < 0) return '刚刚';
  if (diff < 60_000) return '刚刚';
  if (diff < 3_600_000) return `${Math.floor(diff / 60_000)} 分钟前`;
  if (diff < 86_400_000) return `${Math.floor(diff / 3_600_000)} 小时前`;
  if (diff < 7 * 86_400_000) return `${Math.floor(diff / 86_400_000)} 天前`;
  return toDateKey(ts);
}

/** 出勤率：保留 1 位小数的百分比数字（不附带 %） */
export function attendanceRate(present: number, total: number): number {
  if (total <= 0) return 0;
  return Math.round((present / total) * 1000) / 10;
}

/** 百分比格式化：91.7 → '91.7%' */
export function formatPercent(value: number, digits = 1): string {
  return `${value.toFixed(digits)}%`;
}

/** 姓名脱敏：张三 → 张*，欧阳娜娜 → 欧**娜 */
export function maskName(name: string): string {
  if (!name) return '';
  if (name.length <= 1) return name;
  if (name.length === 2) return `${name[0]}*`;
  const head = name[0];
  const tail = name[name.length - 1];
  return `${head}${'*'.repeat(name.length - 2)}${tail}`;
}

/** 手机号脱敏：13800138000 → 138****8000 */
export function maskPhone(phone: string | null | undefined): string {
  if (!phone) return '';
  if (phone.length <= 7) return phone;
  return `${phone.slice(0, 3)}****${phone.slice(-4)}`;
}

/** 考勤状态 → 中文文案 */
export function checkinStateLabel(state: CheckinState): string {
  return CHECKIN_STATUS_META[state]?.label ?? state;
}

/** 考勤状态 → emoji */
export function checkinStateEmoji(state: CheckinState): string {
  return CHECKIN_STATUS_META[state]?.emoji ?? '⚪';
}

/** 字节数 → 人类可读 */
export function formatBytes(bytes: number): string {
  if (!bytes || bytes < 0) return '0 B';
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / 1024 / 1024).toFixed(2)} MB`;
}

/** 数字千分位 */
export function formatNumber(value: number): string {
  return `${value}`.replace(/\B(?=(\d{3})+(?!\d))/g, ',');
}

/** 安全 JSON 解析 */
export function safeJsonParse<T>(raw: string | null | undefined, fallback: T): T {
  if (!raw) return fallback;
  try {
    return JSON.parse(raw) as T;
  } catch {
    return fallback;
  }
}

/** 截断文本 */
export function truncate(text: string, max: number): string {
  if (!text) return '';
  return text.length > max ? `${text.slice(0, max)}…` : text;
}

/** 生成排序用拼音/数字占位（中文按 Unicode 排序，保证稳定） */
export function compareText(a: string, b: string): number {
  return (a || '').localeCompare(b || '', 'zh-Hans-CN');
}

/** 学号排序：能转数字则按数字，否则按文本 */
export function compareStudentNo(a: string, b: string): number {
  const na = Number.parseInt(a, 10);
  const nb = Number.parseInt(b, 10);
  if (!Number.isNaN(na) && !Number.isNaN(nb)) return na - nb;
  return compareText(a, b);
}
