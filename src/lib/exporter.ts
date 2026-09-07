import * as XLSX from 'xlsx';
import { saveBinary, saveText } from './download';
import { toCsvText } from './csv';
import { formatDateTime, toDateKey } from './format';
import type { ExportSheet } from '@/types/api';
import type { CheckinState, StudentStatus } from '@/types/enums';
import type { Student, TaskRecord, TaskStatusNode } from '@/types/models';
import { CHECKIN_STATUS_META, STUDENT_STATUS_META } from '@/constants/status';

/**
 * 导出：xlsx（SheetJS，前端生成）+ csv。
 * 每份报表一个 sheet，含标题行并冻结首行，Excel / WPS 均可正常打开。
 */

/** 将 ExportSheet[] 写为 xlsx 工作簿字节流（首行冻结 + 列宽自适应） */
export function buildWorkbookBytes(sheets: ExportSheet[]): Uint8Array {
  const wb = XLSX.utils.book_new();
  sheets.forEach((sheet) => {
    const aoa: (string | number | null)[][] = [sheet.columns, ...sheet.rows];
    const ws = XLSX.utils.aoa_to_sheet(aoa);
    // 冻结首行
    ws['!freeze'] = { xSplit: 0, ySplit: 1 };
    // 列宽自适应（按最长单元格估算，中文按 2 个字符宽度计）
    const widths = sheet.columns.map((col, i) => {
      let max = estimateWidth(col);
      sheet.rows.forEach((row) => {
        const v = row[i];
        if (v != null) max = Math.max(max, estimateWidth(String(v)));
      });
      return { wch: Math.min(Math.max(max + 2, 8), 40) };
    });
    ws['!cols'] = widths;
    XLSX.utils.book_append_sheet(wb, ws, safeSheetName(sheet.name));
  });
  const out = XLSX.write(wb, { bookType: 'xlsx', type: 'array' }) as ArrayBuffer;
  return new Uint8Array(out);
}

/** Sheet 名长度限制 31 字符且不含 []:*?/\ */
function safeSheetName(name: string): string {
  const cleaned = name.replace(/[[\]:*?/\\]/g, '-').trim();
  return (cleaned || 'Sheet').slice(0, 31);
}

/** 估算显示宽度：中文/全角按 2，其余按 1 */
function estimateWidth(text: string): number {
  let w = 0;
  for (const ch of text) {
    w += /[ᄀ-ᅟ⺀-꓏가-힣豈-﫿︰-﹏＀-｠￠-￦]/.test(ch) ? 2 : 1;
  }
  return w;
}

/** 导出 xlsx（多 sheet） */
export async function exportSheetsToXlsx(
  fileName: string,
  sheets: ExportSheet[],
): Promise<string | null> {
  const bytes = buildWorkbookBytes(sheets);
  return saveBinary(bytes, {
    defaultPath: fileName,
    title: '导出报表',
    filters: [{ name: 'Excel 工作簿', extensions: ['xlsx'] }],
  });
}

/** 导出单 sheet 为 csv */
export async function exportSheetToCsv(
  fileName: string,
  sheet: ExportSheet,
): Promise<string | null> {
  const rows: (string | number | null)[][] = [sheet.columns, ...sheet.rows];
  return saveText(toCsvText(rows), {
    defaultPath: fileName,
    title: '导出 CSV',
    filters: [{ name: 'CSV 文件', extensions: ['csv'] }],
  });
}

/* -------------------------------------------------------------------------- */
/* 具体报表构造                                                                 */
/* -------------------------------------------------------------------------- */

/** 名册表 */
export function buildRosterSheet(students: Student[]): ExportSheet {
  const columns = ['学号', '姓名', '性别', '年级', '班级', '座位号', '状态', '家长电话', '备注'];
  const rows = students.map((s) => [
    s.studentNo,
    s.name,
    s.gender === 'male' ? '男' : s.gender === 'female' ? '女' : '未知',
    s.grade ?? '',
    s.className ?? '',
    s.seatNo ?? '',
    STUDENT_STATUS_META[s.status]?.label ?? s.status,
    s.phone ?? '',
    s.note ?? '',
  ]);
  return { name: '学生名册', columns, rows };
}

/** 考勤明细表 */
export interface CheckinExportRow {
  studentNo: string;
  name: string;
  className: string | null;
  state: CheckinState;
  date: string;
  period: string;
  markedAt: number | null;
  note: string | null;
}

export function buildCheckinSheet(rows: CheckinExportRow[]): ExportSheet {
  const columns = ['日期', '时段', '班级', '学号', '姓名', '状态', '标记时间', '备注'];
  const body = rows.map((r) => [
    r.date,
    r.period,
    r.className ?? '',
    r.studentNo,
    r.name,
    CHECKIN_STATUS_META[r.state]?.label ?? r.state,
    r.markedAt ? formatDateTime(r.markedAt) : '',
    r.note ?? '',
  ]);
  return { name: '考勤明细', columns, rows: body };
}

/** 班级汇总表 */
export interface ClassSummaryExportRow {
  className: string;
  grade: string | null;
  total: number;
  present: number;
  leave: number;
  absent: number;
  late: number;
  attendanceRate: number;
  submitted: boolean;
}

export function buildClassSummarySheet(rows: ClassSummaryExportRow[]): ExportSheet {
  const columns = [
    '年级',
    '班级',
    '应到',
    '出勤',
    '请假',
    '缺勤',
    '迟到',
    '出勤率(%)',
    '是否已提交',
  ];
  const body = rows.map((r) => [
    r.grade ?? '',
    r.className,
    r.total,
    r.present,
    r.leave,
    r.absent,
    r.late,
    r.attendanceRate,
    r.submitted ? '是' : '否',
  ]);
  return { name: '班级汇总', columns, rows: body };
}

/** 异常学生明细表 */
export interface ExceptionExportRow {
  className: string | null;
  studentNo: string;
  name: string;
  state: CheckinState;
  date: string;
  note: string | null;
}

export function buildExceptionSheet(rows: ExceptionExportRow[]): ExportSheet {
  const columns = ['日期', '班级', '学号', '姓名', '状态', '备注'];
  const body = rows.map((r) => [
    r.date,
    r.className ?? '',
    r.studentNo,
    r.name,
    CHECKIN_STATUS_META[r.state]?.label ?? r.state,
    r.note ?? '',
  ]);
  return { name: '异常学生明细', columns, rows: body };
}

/** 任务完成率表 */
export function buildTaskCompletionSheet(
  rows: { taskId: string; title: string; className: string | null; grade: string | null; total: number; finalCount: number; completionRate: number; avgScore: number | null }[],
): ExportSheet {
  const columns = ['年级', '班级', '任务', '总人数', '完成人数', '完成率(%)', '平均分'];
  const body = rows.map((r) => [
    r.grade ?? '',
    r.className ?? '',
    r.title,
    r.total,
    r.finalCount,
    r.completionRate,
    r.avgScore ?? '',
  ]);
  return { name: '任务完成率', columns, rows: body };
}

/** 任务矩阵导出（学生 × 节点 + 评分 + 备注） */
export function buildTaskMatrixSheet(
  taskTitle: string,
  nodes: TaskStatusNode[],
  students: Student[],
  recordMap: Record<string, TaskRecord>,
): ExportSheet {
  const columns = ['学号', '姓名', '班级', '状态节点', '评分', '备注', '完成时间'];
  const nodeLabel = (key: string): string =>
    nodes.find((n) => n.nodeKey === key)?.label ?? key;
  const rows = students.map((s) => {
    const rec = recordMap[s.id];
    return [
      s.studentNo,
      s.name,
      s.className ?? '',
      rec ? nodeLabel(rec.nodeKey) : '',
      rec?.score ?? '',
      rec?.note ?? '',
      rec?.completedAt ? formatDateTime(rec.completedAt) : '',
    ];
  });
  return { name: safeSheetName(taskTitle || '任务矩阵'), columns, rows };
}

/** 默认导出文件名：xxx_2026-09-08.xlsx */
export function defaultExportName(prefix: string, ext = 'xlsx'): string {
  return `${prefix}_${toDateKey(Date.now())}.${ext}`;
}

/** 名册状态统计（导出前概览用） */
export function countByStatus(students: Student[]): Record<StudentStatus, number> {
  const acc: Record<StudentStatus, number> = { active: 0, leave: 0, transferred: 0 };
  students.forEach((s) => {
    acc[s.status] = (acc[s.status] ?? 0) + 1;
  });
  return acc;
}
