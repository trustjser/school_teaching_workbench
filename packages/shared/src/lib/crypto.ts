/**
 * 前端侧轻量工具：UUID / Base64 / 校验和。
 * 注意：本文件**不参与** HMAC 签名与 AES 加解密（均在 Rust 侧完成）。
 */

/**
 * 生成 UUID v4（小写带连字符）。
 * 优先使用 crypto.randomUUID，回退到 getRandomValues，最后回退到 Math.random。
 */
export function uuidV4(): string {
  const g = globalThis.crypto as Crypto | undefined;
  if (g && typeof g.randomUUID === 'function') {
    return g.randomUUID();
  }
  if (g && typeof g.getRandomValues === 'function') {
    const bytes = new Uint8Array(16);
    g.getRandomValues(bytes);
    // 设置 version(4) 与 variant(10xx)
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    const hex = Array.from(bytes, (b) => b.toString(16).padStart(2, '0')).join('');
    return `${hex.slice(0, 8)}-${hex.slice(8, 12)}-${hex.slice(12, 16)}-${hex.slice(16, 20)}-${hex.slice(20)}`;
  }
  // 极端兜底（不应发生）
  return 'xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx'.replace(/[xy]/g, (c) => {
    const r = (Math.random() * 16) | 0;
    const v = c === 'x' ? r : (r & 0x3) | 0x8;
    return v.toString(16);
  });
}

/** 生成 32 字节随机密钥并返回 Base64 字符串（用于首次启动向导生成共享密钥） */
export function generateSecretBase64(): string {
  const bytes = new Uint8Array(32);
  const g = globalThis.crypto;
  if (g && typeof g.getRandomValues === 'function') {
    g.getRandomValues(bytes);
  } else {
    for (let i = 0; i < bytes.length; i += 1) {
      bytes[i] = Math.floor(Math.random() * 256);
    }
  }
  return bytesToBase64(bytes);
}

/** Uint8Array → Base64 */
export function bytesToBase64(bytes: Uint8Array): string {
  let binary = '';
  for (let i = 0; i < bytes.length; i += 1) {
    binary += String.fromCharCode(bytes[i]);
  }
  return btoa(binary);
}

/** Base64 → Uint8Array */
export function base64ToBytes(base64: string): Uint8Array {
  const clean = base64.trim();
  const binary = atob(clean);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i += 1) {
    bytes[i] = binary.charCodeAt(i);
  }
  return bytes;
}

/** Uint8Array → hex 字符串 */
export function bytesToHex(bytes: Uint8Array): string {
  return Array.from(bytes, (b) => b.toString(16).padStart(2, '0')).join('');
}

/**
 * SHA-256（hex）。用于计算密钥指纹等信息展示。
 * 不可用（非安全上下文）时回退到简单哈希，仅用于 UI 展示。
 */
export async function sha256Hex(input: string): Promise<string> {
  const subtle = globalThis.crypto?.subtle;
  if (subtle && typeof subtle.digest === 'function') {
    try {
      const data = new TextEncoder().encode(input);
      const digest = await subtle.digest('SHA-256', data);
      return bytesToHex(new Uint8Array(digest));
    } catch {
      // 落到回退实现
    }
  }
  return fallbackHashHex(input);
}

/** 同步回退哈希（FNV-1a 变体，仅用于 UI 展示，不具备密码学强度） */
export function fallbackHashHex(input: string): string {
  let h1 = 0x811c9dc5;
  let h2 = 0x01000193;
  for (let i = 0; i < input.length; i += 1) {
    const c = input.charCodeAt(i);
    h1 ^= c;
    h1 = Math.imul(h1, 0x01000193) >>> 0;
    h2 ^= c + i;
    h2 = Math.imul(h2, 0x85ebca6b) >>> 0;
  }
  return (h1.toString(16).padStart(8, '0') + h2.toString(16).padStart(8, '0')).repeat(4).slice(0, 64);
}

/** 计算共享密钥指纹：SHA-256 前 8 字节 hex（16 字符），分组展示便于人工核对 */
export async function keyFingerprint(secretBase64: string): Promise<string> {
  const hex = await sha256Hex(secretBase64);
  return hex.slice(0, 16);
}

/** 指纹分组展示：abcdef0123456789 → abcd ef01 2345 6789 */
export function formatFingerprint(fp: string): string {
  return (fp.match(/.{1,4}/g) || [fp]).join(' ');
}

/** 简单的稳定哈希（用于给匿名数据生成稳定的伪随机色，非加密用途） */
export function stableHash(input: string): number {
  let h = 2166136261;
  for (let i = 0; i < input.length; i += 1) {
    h ^= input.charCodeAt(i);
    h = Math.imul(h, 16777619);
  }
  return h >>> 0;
}
