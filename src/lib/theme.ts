import type { ThemeName } from '@/types/enums';

/**
 * 主题应用层：把 ThemeName 映射到 <html> 上的类（light/dark/high-contrast），
 * 并持久化到 localStorage，使刷新/重启不丢失、首帧不闪烁。
 *
 * 注意：本模块不依赖 store，避免循环引用。store.theme 的变化由 main.tsx 订阅后
 * 调用 applyThemeClass 落地到 DOM。
 */

const THEME_KEY = 'lan-workbench:theme';
const VALID: readonly ThemeName[] = ['light', 'dark', 'high-contrast'];

/** 读取本地存储的主题（首帧前调用，避免闪白） */
export function readStoredTheme(): ThemeName {
  try {
    const t = window.localStorage.getItem(THEME_KEY) as ThemeName | null;
    if (t && (VALID as readonly string[]).includes(t)) return t;
  } catch {
    /* 忽略 */
  }
  return 'light';
}

/** 持久化主题选择 */
export function persistTheme(theme: ThemeName): void {
  try {
    window.localStorage.setItem(THEME_KEY, theme);
  } catch {
    /* 忽略 */
  }
}

/**
 * 把主题落到 DOM。
 * @param animate 是否在切换瞬间加 .theme-anim（启用背景/文字/边框的平滑过渡）。
 *                 首帧与启动引导阶段应传 false，避免无意义的全局过渡。
 */
export function applyThemeClass(theme: ThemeName, animate = true): void {
  const root = document.documentElement;
  if (animate) {
    root.classList.add('theme-anim');
    window.setTimeout(() => root.classList.remove('theme-anim'), 360);
  }
  root.classList.remove('dark', 'high-contrast');
  if (theme === 'dark') root.classList.add('dark');
  else if (theme === 'high-contrast') root.classList.add('high-contrast');
  root.dataset.theme = theme;
}
