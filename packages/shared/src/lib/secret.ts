/**
 * 共享密钥工具：两端首次配置与设置页共用同一份格式校验与随机生成实现。
 *
 * 密钥语义：Base64 编码的 32 字节随机串（AES-256 / HMAC 根密钥）。
 * 教务端生成后分发给所有班级端，指纹一致才能互相解密同步。
 */

/** 共享密钥必须是 base64 编码的 32 字节 */
export function isBase64Secret(s: string): boolean {
  if (!/^[A-Za-z0-9+/]+={0,2}$/.test(s)) return false;
  try {
    return atob(s).length === 32;
  } catch {
    return false;
  }
}

/** 生成 base64 编码的 32 字节随机密钥 */
export function randomSecret(): string {
  const bytes = new Uint8Array(32);
  crypto.getRandomValues(bytes);
  let bin = '';
  for (const b of bytes) bin += String.fromCharCode(b);
  return btoa(bin);
}
