import { useCallback, useEffect, useRef, useState } from 'react';

export interface UseKeyboardCycleOptions {
  /** 可循环切换的条目 ID 列表（顺序即网格顺序） */
  ids: string[];
  /** 每行个数（用于上下方向键跨行移动） */
  columns: number;
  /** 空格 / 回车：切换到下一状态 */
  onCycle: (id: string) => void;
  /** Shift + 空格 / 反方向键：切换到上一状态（可选） */
  onReverseCycle?: (id: string) => void;
  /** 是否启用（页面聚焦时开，弹窗打开时关） */
  enabled?: boolean;
}

export interface UseKeyboardCycleResult {
  /** 当前焦点索引 */
  focusIndex: number;
  setFocusIndex: (index: number) => void;
  /** 容器键盘事件处理器 */
  onKeyDown: (e: React.KeyboardEvent<HTMLElement>) => void;
  /** 容器属性：直接展开到网格容器上 */
  containerProps: {
    tabIndex: number;
    role: string;
    'aria-activedescendant': string | undefined;
    onKeyDown: (e: React.KeyboardEvent<HTMLElement>) => void;
  };
  /** 条目属性：展开到每个卡片上 */
  itemProps: (index: number) => { id: string; 'data-focused': boolean };
  /** 当前焦点 ID */
  focusedId: string | null;
}

/**
 * 键盘循环切换：
 *  - ←/→/↑/↓ 移动焦点（跨行按 columns 计算）
 *  - 空格 / 回车 触发 onCycle（循环切换状态）
 *  - Shift + 空格 触发 onReverseCycle
 *  - Home / End 跳到首尾
 */
export function useKeyboardCycle(options: UseKeyboardCycleOptions): UseKeyboardCycleResult {
  const { ids, columns, onCycle, onReverseCycle, enabled = true } = options;
  const [focusIndex, setFocusIndex] = useState(0);
  const idsRef = useRef(ids);
  idsRef.current = ids;
  const cycleRef = useRef(onCycle);
  cycleRef.current = onCycle;
  const reverseRef = useRef(onReverseCycle);
  reverseRef.current = onReverseCycle;

  // ids 变化（切换日期/筛选）时收敛焦点，避免越界
  useEffect(() => {
    setFocusIndex((idx) => {
      if (ids.length === 0) return 0;
      return Math.min(Math.max(idx, 0), ids.length - 1);
    });
  }, [ids.length]);

  const move = useCallback(
    (delta: number) => {
      setFocusIndex((idx) => {
        const next = idx + delta;
        if (next < 0) return 0;
        if (next >= idsRef.current.length) return idsRef.current.length - 1;
        return next;
      });
    },
    [],
  );

  const onKeyDown = useCallback(
    (e: React.KeyboardEvent<HTMLElement>) => {
      if (!enabled) return;
      const cols = Math.max(1, columns);
      switch (e.key) {
        case 'ArrowRight':
          e.preventDefault();
          move(1);
          break;
        case 'ArrowLeft':
          e.preventDefault();
          move(-1);
          break;
        case 'ArrowDown':
          e.preventDefault();
          move(cols);
          break;
        case 'ArrowUp':
          e.preventDefault();
          move(-cols);
          break;
        case 'Home':
          e.preventDefault();
          setFocusIndex(0);
          break;
        case 'End':
          e.preventDefault();
          setFocusIndex(Math.max(0, idsRef.current.length - 1));
          break;
        case ' ':
        case 'Spacebar':
        case 'Enter': {
          e.preventDefault();
          const id = idsRef.current[focusIndex];
          if (!id) return;
          if (e.shiftKey && reverseRef.current) {
            reverseRef.current(id);
          } else {
            cycleRef.current(id);
          }
          break;
        }
        default:
          break;
      }
    },
    [columns, enabled, focusIndex, move],
  );

  const focusedId = ids[focusIndex] ?? null;

  return {
    focusIndex,
    setFocusIndex,
    onKeyDown,
    containerProps: {
      tabIndex: 0,
      role: 'grid',
      'aria-activedescendant': focusedId ? `cycle-item-${focusedId}` : undefined,
      onKeyDown,
    },
    itemProps: (index: number) => ({
      id: `cycle-item-${ids[index] ?? index}`,
      'data-focused': index === focusIndex,
    }),
    focusedId,
  };
}
