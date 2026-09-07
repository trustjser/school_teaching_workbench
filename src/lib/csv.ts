/**
 * CSV 解析与生成。
 * 兼容：UTF-8 / UTF-8 BOM / GBK（通过 TextDecoder('gbk') 回退，浏览器与 WebView 均内置）。
 */

/** 常见编码探测顺序 */
const ENCODINGS = ['utf-8', 'gbk', 'gb18030', 'big5'] as const;

/** 探测并解码字节流为字符串 */
export function decodeBytes(bytes: Uint8Array): { text: string; encoding: string } {
  // 1. UTF-8 BOM
  if (bytes.length >= 3 && bytes[0] === 0xef && bytes[1] === 0xbb && bytes[2] === 0xbf) {
    return { text: new TextDecoder('utf-8').decode(bytes.subarray(3)), encoding: 'utf-8-bom' };
  }
  // 2. UTF-16 LE/BE BOM
  if (bytes.length >= 2 && bytes[0] === 0xff && bytes[1] === 0xfe) {
    return { text: new TextDecoder('utf-16le').decode(bytes), encoding: 'utf-16le' };
  }
  if (bytes.length >= 2 && bytes[0] === 0xfe && bytes[1] === 0xff) {
    return { text: new TextDecoder('utf-16be').decode(bytes), encoding: 'utf-16be' };
  }
  // 3. 先试 UTF-8 严格解码，失败（出现替换字符）再试 GBK
  try {
    const utf8 = new TextDecoder('utf-8', { fatal: true }).decode(bytes);
    return { text: utf8, encoding: 'utf-8' };
  } catch {
    // 落到下面的循环探测
  }
  for (const enc of ENCODINGS) {
    try {
      const text = new TextDecoder(enc).decode(bytes);
      if (text && !text.includes('\uFFFD')) {
        return { text, encoding: enc };
      }
    } catch {
      /* 不支持的编码，继续下一个 */
    }
  }
  // 最终兜底：非严格 UTF-8
  return { text: new TextDecoder('utf-8').decode(bytes), encoding: 'utf-8' };
}

/** 简单 CSV 行解析（支持双引号包裹、转义双引号、\r\n / \n） */
export function parseCsvText(text: string): string[][] {
  const rows: string[][] = [];
  let row: string[] = [];
  let field = '';
  let inQuotes = false;
  let i = 0;
  while (i < text.length) {
    const ch = text[i];
    if (inQuotes) {
      if (ch === '"') {
        if (text[i + 1] === '"') {
          field += '"';
          i += 2;
          continue;
        }
        inQuotes = false;
        i += 1;
        continue;
      }
      field += ch;
      i += 1;
      continue;
    }
    if (ch === '"') {
      inQuotes = true;
      i += 1;
      continue;
    }
    if (ch === ',') {
      row.push(field);
      field = '';
      i += 1;
      continue;
    }
    if (ch === '\r') {
      i += 1;
      continue;
    }
    if (ch === '\n') {
      row.push(field);
      rows.push(row);
      row = [];
      field = '';
      i += 1;
      continue;
    }
    field += ch;
    i += 1;
  }
  if (field.length > 0 || row.length > 0) {
    row.push(field);
    rows.push(row);
  }
  // 去掉尾部空行
  while (rows.length > 0 && rows[rows.length - 1].every((c) => c.trim() === '')) {
    rows.pop();
  }
  return rows;
}

/** 解析 CSV 字节流 */
export function parseCsvBytes(bytes: Uint8Array): { rows: string[][]; encoding: string } {
  const { text, encoding } = decodeBytes(bytes);
  return { rows: parseCsvText(text), encoding };
}

/** 转义 CSV 单元格 */
export function escapeCsvCell(value: string | number | null | undefined): string {
  if (value == null) return '';
  const s = String(value);
  if (/[",\n\r]/.test(s)) {
    return `"${s.replace(/"/g, '""')}"`;
  }
  return s;
}

/** 生成 CSV 文本（带 UTF-8 BOM，保证 Excel 正确识别中文） */
export function toCsvText(rows: (string | number | null)[][], withBom = true): string {
  const body = rows.map((r) => r.map((c) => escapeCsvCell(c)).join(',')).join('\r\n');
  return withBom ? `\uFEFF${body}` : body;
}
