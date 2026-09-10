import { save } from '@tauri-apps/plugin-dialog';
import { writeFile } from '@tauri-apps/plugin-fs';
import { isTauriRuntime } from './tauri';
import { AppError } from './tauri';

/**
 * 文件保存：Tauri 环境走「另存为对话框 + fs 写盘」，
 * 浏览器环境回退到 Blob + <a download>。
 */

export interface SaveFileOptions {
  /** 建议文件名 */
  defaultPath?: string;
  /** 对话框标题 */
  title?: string;
  /** 扩展名过滤器，如 [{ name: 'Excel', extensions: ['xlsx'] }] */
  filters?: { name: string; extensions: string[] }[];
}

/** 保存二进制内容 */
export async function saveBinary(
  data: Uint8Array,
  options: SaveFileOptions = {},
): Promise<string | null> {
  if (isTauriRuntime()) {
    const path = await save({
      defaultPath: options.defaultPath,
      title: options.title,
      filters: options.filters,
    });
    if (!path) return null;
    await writeFile(path, data);
    return path;
  }
  // 浏览器回退
  const blob = new Blob([data as unknown as BlobPart], {
    type: 'application/octet-stream',
  });
  triggerBrowserDownload(blob, options.defaultPath || 'download.bin');
  return options.defaultPath || 'download.bin';
}

/** 保存文本（自动加 UTF-8 BOM 以便 Excel 正确识别中文） */
export async function saveText(
  text: string,
  options: SaveFileOptions = {},
  withBom = true,
): Promise<string | null> {
  const body = withBom ? `\uFEFF${text}` : text;
  const bytes = new TextEncoder().encode(body);
  return saveBinary(bytes, options);
}

/** 浏览器端下载触发 */
export function triggerBrowserDownload(blob: Blob, fileName: string): void {
  const url = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url;
  a.download = fileName;
  a.style.display = 'none';
  document.body.appendChild(a);
  a.click();
  document.body.removeChild(a);
  setTimeout(() => URL.revokeObjectURL(url), 2000);
}

/** 打开文件选择（单文件） */
export function pickFile(accept: string): Promise<File | null> {
  return new Promise((resolve) => {
    const input = document.createElement('input');
    input.type = 'file';
    input.accept = accept;
    input.style.display = 'none';
    input.onchange = () => {
      resolve(input.files && input.files.length > 0 ? input.files[0] : null);
      document.body.removeChild(input);
    };
    document.body.appendChild(input);
    input.click();
  });
}

/** 统一的保存异常处理：用户取消不视为错误 */
export function isUserCancel(err: unknown): boolean {
  if (err instanceof AppError) return false;
  const msg = String((err as Error)?.message ?? err ?? '').toLowerCase();
  return msg.includes('cancel') || msg.includes('取消');
}
