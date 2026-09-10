import * as XLSX from 'xlsx';
import { parseCsvBytes } from './csv';
import type { ImportRowError, ParsedStudentRow } from '@/types/api';
import { NOTE_MAX_LENGTH } from '@/constants/app';

/**
 * 名册导入解析：.xlsx / .xls / .csv
 * 支持列别名映射、行级校验、GBK/UTF-8 BOM 兼容。
 */

/** 字段别名表（大小写不敏感，去空格后匹配） */
export const COLUMN_ALIASES: Record<keyof ParsedStudentRow | 'ignore', string[]> = {
  rowIndex: [],
  studentNo: ['学号', '学籍号', '编号', 'studentno', 'no', 'id', 'student_no', 'studentid'],
  name: ['姓名', '学生姓名', '名字', 'name', 'studentname', 'student_name'],
  gender: ['性别', 'gender', 'sex'],
  grade: ['年级', 'grade'],
  className: ['班级', '班级名称', 'class', 'classname', 'class_name'],
  seatNo: ['座位号', '座号', '序号', 'seat', 'seatno', 'seat_no'],
  phone: ['电话', '联系电话', '家长电话', '手机', '手机号', 'phone', 'tel', 'mobile'],
  note: ['备注', '说明', 'note', 'remark', 'memo'],
  errors: [],
  ignore: [],
};

/** 归一化表头：去空格、去括号内容、转小写 */
export function normalizeHeader(raw: string): string {
  return String(raw || '')
    .replace(/\s+/g, '')
    .replace(/[（(][^）)]*[）)]/g, '')
    .toLowerCase();
}

/** 依据表头推断列映射：返回 { 字段: 列索引 } */
export function inferColumnMap(headers: string[]): Record<string, number> {
  const map: Record<string, number> = {};
  headers.forEach((h, idx) => {
    const norm = normalizeHeader(h);
    if (!norm) return;
    for (const [field, aliases] of Object.entries(COLUMN_ALIASES)) {
      if (field === 'ignore') continue;
      if (map[field] !== undefined) continue;
      if (aliases.some((a) => norm === a || norm.includes(a))) {
        map[field] = idx;
        break;
      }
    }
  });
  return map;
}

/** 性别归一化 */
export function normalizeGender(raw: string): string {
  const v = String(raw || '').trim();
  if (!v) return 'unknown';
  if (['男', 'm', 'male', 'boy', '1'].includes(v.toLowerCase())) return 'male';
  if (['女', 'f', 'female', 'girl', '2'].includes(v.toLowerCase())) return 'female';
  return 'unknown';
}

/** 读取通用二维数组（自动识别 xlsx / csv） */
export async function readSheet(file: File): Promise<{
  rows: string[][];
  encoding: string;
  sheetName: string | null;
}> {
  const buffer = await file.arrayBuffer();
  const lower = file.name.toLowerCase();
  if (lower.endsWith('.csv') || lower.endsWith('.txt')) {
    const { rows, encoding } = parseCsvBytes(new Uint8Array(buffer));
    return { rows, encoding, sheetName: null };
  }
  const workbook = XLSX.read(new Uint8Array(buffer), { type: 'array', cellDates: false });
  const first = workbook.SheetNames[0];
  if (!first) return { rows: [], encoding: 'xlsx', sheetName: null };
  const sheet = workbook.Sheets[first];
  const matrix = XLSX.utils.sheet_to_json<unknown[]>(sheet, {
    header: 1,
    blankrows: false,
    defval: '',
    raw: false,
  });
  const rows = matrix.map((r) =>
    (Array.isArray(r) ? r : []).map((c) => (c == null ? '' : String(c).trim())),
  );
  return { rows, encoding: 'xlsx', sheetName: first };
}

export interface ParseStudentOptions {
  /** 手动列映射（覆盖自动推断），键为字段，值为列索引；-1 表示忽略 */
  columnMap?: Record<string, number>;
  /** 班级缺省值（导入时统一填充） */
  defaultClassName?: string | null;
  defaultGrade?: string | null;
}

/**
 * 将二维数组解析为名册行 + 行级校验报告。
 * @param rows 原始二维表（第一行视为表头）
 */
export function parseStudentRows(
  rows: string[][],
  options: ParseStudentOptions = {},
): { parsed: ParsedStudentRow[]; headers: string[]; columnMap: Record<string, number> } {
  if (rows.length === 0) return { parsed: [], headers: [], columnMap: {} };
  const headers = rows[0].map((h) => String(h || '').trim());
  const autoMap = inferColumnMap(headers);
  const columnMap: Record<string, number> = { ...autoMap, ...(options.columnMap || {}) };

  const idx = (field: keyof ParsedStudentRow): number => {
    const v = columnMap[field as string];
    return typeof v === 'number' ? v : -1;
  };

  const parsed: ParsedStudentRow[] = [];
  const seenNo = new Map<string, number>();

  for (let r = 1; r < rows.length; r += 1) {
    const raw = rows[r];
    // 全空行跳过
    if (!raw || raw.every((c) => String(c || '').trim() === '')) continue;

    const pick = (field: keyof ParsedStudentRow): string => {
      const i = idx(field);
      if (i < 0 || i >= raw.length) return '';
      return String(raw[i] ?? '').trim();
    };

    const studentNo = pick('studentNo');
    const name = pick('name');
    const gender = normalizeGender(pick('gender'));
    const grade = pick('grade') || options.defaultGrade || '';
    const className = pick('className') || options.defaultClassName || '';
    const seatRaw = pick('seatNo');
    const seatNo = seatRaw === '' ? null : Number.parseInt(seatRaw, 10);
    const phone = pick('phone');
    const note = pick('note').slice(0, NOTE_MAX_LENGTH);

    const errors: ImportRowError[] = [];
    const rowNumber = r + 1; // Excel 行号（1-based，含表头）

    if (!name) errors.push({ row: rowNumber, field: 'name', message: '姓名不能为空' });
    if (!studentNo) {
      errors.push({ row: rowNumber, field: 'studentNo', message: '学号不能为空' });
    } else if (seenNo.has(`${grade}|${className}|${studentNo}`)) {
      errors.push({
        row: rowNumber,
        field: 'studentNo',
        message: `学号与第 ${seenNo.get(`${grade}|${className}|${studentNo}`)} 行重复`,
      });
    } else {
      seenNo.set(`${grade}|${className}|${studentNo}`, rowNumber);
    }
    if (seatRaw !== '' && (seatNo === null || Number.isNaN(seatNo))) {
      errors.push({ row: rowNumber, field: 'seatNo', message: `座位号「${seatRaw}」不是数字` });
    }
    if (name && name.length > 20) {
      errors.push({ row: rowNumber, field: 'name', message: '姓名超过 20 个字符' });
    }

    parsed.push({
      rowIndex: rowNumber,
      studentNo,
      name,
      gender,
      grade,
      className,
      seatNo: seatNo === null || Number.isNaN(seatNo) ? null : seatNo,
      phone,
      note,
      errors,
    });
  }

  return { parsed, headers, columnMap };
}

/** 一键解析文件 → 名册行（组合 readSheet + parseStudentRows） */
export async function parseStudentFile(
  file: File,
  options: ParseStudentOptions = {},
): Promise<{
  parsed: ParsedStudentRow[];
  headers: string[];
  columnMap: Record<string, number>;
  encoding: string;
}> {
  const { rows, encoding } = await readSheet(file);
  const { parsed, headers, columnMap } = parseStudentRows(rows, options);
  return { parsed, headers, columnMap, encoding };
}

/** 生成名册导入模板（供用户下载填写） */
export function buildStudentTemplate(): (string | number | null)[][] {
  return [
    ['学号', '姓名', '性别', '年级', '班级', '座位号', '家长电话', '备注'],
    ['2026030201', '张三', '男', '3', '三年级二班', 1, '13800000001', ''],
    ['2026030202', '李四', '女', '3', '三年级二班', 2, '13800000002', ''],
  ];
}

/** 名册模板列名（用于导出模板文件） */
export const STUDENT_TEMPLATE_HEADERS = [
  '学号',
  '姓名',
  '性别',
  '年级',
  '班级',
  '座位号',
  '家长电话',
  '备注',
] as const;
