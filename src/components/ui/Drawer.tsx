import { useEffect, useRef, type ReactNode } from 'react';
import { X } from 'lucide-react';
import { IconButton } from './IconButton';

export interface DrawerProps {
  open: boolean;
  onClose: () => void;
  title: ReactNode;
  description?: ReactNode;
  children: ReactNode;
  footer?: ReactNode;
  /** 宽度类，大屏可放宽 */
  widthClass?: string;
  /** 停靠方向 */
  side?: 'right' | 'left';
}

/** 右侧抽屉：详情 / 编辑面板。Esc 关闭，打开时锁滚动。 */
export function Drawer({
  open,
  onClose,
  title,
  description,
  children,
  footer,
  widthClass = 'w-[560px]',
  side = 'right',
}: DrawerProps): JSX.Element | null {
  // 用 ref 持有最新的 onClose，避免父组件每次渲染传入新函数导致 effect 反复重跑
  const onCloseRef = useRef(onClose);
  onCloseRef.current = onClose;

  useEffect(() => {
    if (!open) return;
    const onKeyDown = (e: KeyboardEvent): void => {
      if (e.key === 'Escape') onCloseRef.current();
    };
    document.addEventListener('keydown', onKeyDown);
    const prev = document.body.style.overflow;
    document.body.style.overflow = 'hidden';
    return () => {
      document.removeEventListener('keydown', onKeyDown);
      document.body.style.overflow = prev;
    };
  }, [open]);

  if (!open) return null;

  return (
    <div className="fixed inset-0 z-drawer flex">
      <div
        className="flex-1 bg-ink/40"
        role="presentation"
        onClick={onClose}
      />
      <aside
        role="dialog"
        aria-modal="true"
        aria-label={typeof title === 'string' ? title : '抽屉面板'}
        className={[
          'flex h-full max-w-full flex-col bg-surface-raised shadow-pop',
          side === 'right' ? 'animate-slide-in-right' : '',
          widthClass,
        ].join(' ')}
      >
        <header className="flex items-start justify-between gap-4 border-b border-surface-border px-6 py-5">
          <div className="min-w-0">
            <h2 className="text-2xl font-bold text-ink">{title}</h2>
            {description && <p className="mt-1 text-base text-ink-muted">{description}</p>}
          </div>
          <IconButton icon={<X className="h-6 w-6" />} label="关闭" onClick={onClose} />
        </header>
        <div className="min-h-0 flex-1 overflow-y-auto px-6 py-5">{children}</div>
        {footer && (
          <footer className="flex items-center justify-end gap-3 border-t border-surface-border px-6 py-4">
            {footer}
          </footer>
        )}
      </aside>
    </div>
  );
}
