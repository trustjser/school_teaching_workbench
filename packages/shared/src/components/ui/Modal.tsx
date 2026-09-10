import { useEffect, useRef, type ReactNode } from 'react';
import { createPortal } from 'react-dom';
import { X } from 'lucide-react';
import { IconButton } from './IconButton';

export interface ModalProps {
  open: boolean;
  onClose: () => void;
  title: ReactNode;
  description?: ReactNode;
  children: ReactNode;
  footer?: ReactNode;
  /** 最大宽度类 */
  widthClass?: string;
  /** 点击遮罩关闭（默认 true） */
  closeOnBackdrop?: boolean;
}

/**
 * 模态框：Esc 关闭、焦点陷阱、大屏居中。
 * 打开时锁定 body 滚动并将焦点移入对话框。
 * 遮罩层全屏覆盖（z-index 高于一切），面板内容区独立滚动。
 */
export function Modal({
  open,
  onClose,
  title,
  description,
  children,
  footer,
  widthClass = 'max-w-3xl',
  closeOnBackdrop = true,
}: ModalProps): JSX.Element | null {
  const panelRef = useRef<HTMLDivElement>(null);
  // 用 ref 持有最新的 onClose，避免父组件每次渲染传入新函数导致 effect 反复重跑
  // （否则每次按键 setGradeDraft 都会重新调度 panel.focus()，把输入焦点从输入框抢走）。
  const onCloseRef = useRef(onClose);
  onCloseRef.current = onClose;

  useEffect(() => {
    if (!open) return;
    const onKeyDown = (e: KeyboardEvent): void => {
      if (e.key === 'Escape') {
        e.stopPropagation();
        onCloseRef.current();
        return;
      }
      if (e.key !== 'Tab') return;
      const panel = panelRef.current;
      if (!panel) return;
      const focusables = panel.querySelectorAll<HTMLElement>(
        'button:not([disabled]), [href], input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])',
      );
      if (focusables.length === 0) return;
      const first = focusables[0];
      const last = focusables[focusables.length - 1];
      if (e.shiftKey && document.activeElement === first) {
        e.preventDefault();
        last.focus();
      } else if (!e.shiftKey && document.activeElement === last) {
        e.preventDefault();
        first.focus();
      }
    };
    document.addEventListener('keydown', onKeyDown, true);
    const prevOverflow = document.body.style.overflow;
    document.body.style.overflow = 'hidden';
    // 聚焦到面板
    const timer = window.setTimeout(() => {
      panelRef.current?.focus();
    }, 30);
    return () => {
      document.removeEventListener('keydown', onKeyDown, true);
      document.body.style.overflow = prevOverflow;
      window.clearTimeout(timer);
    };
  }, [open]);

  if (!open) return null;

  return createPortal(
    <div className="fixed inset-0 z-[9999] flex items-center justify-center p-4 sm:p-6">
      {/* 全屏遮罩：半透明深色 + 背景模糊，确保覆盖所有内容 */}
      <div
        className="absolute inset-0 bg-black/55 backdrop-blur-sm"
        role="presentation"
        onClick={() => {
          if (closeOnBackdrop) onClose();
        }}
      />
      {/* 面板：限制最大高度，内容区独立滚动 */}
      <div
        ref={panelRef}
        tabIndex={-1}
        role="dialog"
        aria-modal="true"
        aria-label={typeof title === 'string' ? title : '对话框'}
        className={[
          'relative z-10 w-full animate-pop-in rounded-panel bg-surface-raised shadow-pop',
          'flex max-h-[85vh] min-w-0 flex-col outline-none overscroll-contain',
          widthClass,
        ].join(' ')}
      >
        <header className="shrink-0 flex items-start justify-between gap-4 border-b border-surface-border px-5 sm:px-6 py-4 sm:py-5">
          <div className="min-w-0 flex-1">
            <h2 className="block w-full truncate text-xl font-bold text-ink sm:text-2xl">{title}</h2>
            {description && <p className="mt-1 text-sm text-ink-muted sm:text-base">{description}</p>}
          </div>
          <IconButton icon={<X className="h-5 w-5 sm:h-6 sm:w-6" />} label="关闭" onClick={onClose} />
        </header>
        {/* 内容区：可滚动，其余区域不滚动 */}
        <div className="min-h-0 flex-1 overflow-y-auto overscroll-contain px-5 sm:px-6 py-4 sm:py-5">
          {children}
        </div>
        {footer && (
          <footer className="shrink-0 flex items-center justify-end gap-3 border-t border-surface-border px-5 sm:px-6 py-3 sm:py-4">
            {footer}
          </footer>
        )}
      </div>
    </div>,
    document.body,
  );
}
