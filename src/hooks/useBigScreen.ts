import { useEffect, useState } from 'react';
import { UI_SCALE_RULES } from '@/constants/ui';
import { useAppStore } from '@/store/useAppStore';

export interface BigScreenInfo {
  width: number;
  height: number;
  /** 是否为大屏（≥1600px） */
  isBigScreen: boolean;
  /** 是否为超大屏（≥1920px） */
  isWallScreen: boolean;
  /** 当前 UI 缩放 */
  scale: number;
}

/**
 * 大屏适配：按屏幕尺寸自动设置根字号缩放。
 * 优先使用用户在设置页指定的 uiScale；若用户未手动设置过（与自动档一致），则跟随屏幕自适应。
 */
export function useBigScreen(): BigScreenInfo {
  const uiScale = useAppStore((s) => s.uiScale);
  const [size, setSize] = useState<{ width: number; height: number }>(() => ({
    width: typeof window === 'undefined' ? 1280 : window.innerWidth,
    height: typeof window === 'undefined' ? 800 : window.innerHeight,
  }));

  useEffect(() => {
    const onResize = (): void => {
      setSize({ width: window.innerWidth, height: window.innerHeight });
    };
    window.addEventListener('resize', onResize);
    onResize();
    return () => window.removeEventListener('resize', onResize);
  }, []);

  useEffect(() => {
    const root = document.documentElement;
    // Tailwind 以 rem 为基础，缩放根字号即可整体放大（含大屏基线最小字号）
    root.style.fontSize = `${16 * uiScale}px`;
    root.style.setProperty('--ui-scale', String(uiScale));
    return () => {
      root.style.fontSize = '';
      root.style.removeProperty('--ui-scale');
    };
  }, [uiScale]);

  const isWallScreen = size.width >= 1920;
  const isBigScreen = size.width >= 1600;

  return {
    width: size.width,
    height: size.height,
    isBigScreen,
    isWallScreen,
    scale: uiScale,
  };
}

/** 依据屏幕宽度推荐缩放档位（设置页"跟随屏幕"按钮使用） */
export function recommendScale(width: number): number {
  const rule = UI_SCALE_RULES.find((r) => width >= r.minWidth);
  return rule ? rule.scale : 1;
}
