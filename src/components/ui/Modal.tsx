import { useEffect, useRef, type ReactNode } from 'react';
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
 * 模态框：Esc 关闭、焦点陷阱、大屏居中放大。
 * 打开时锁定 body 滚动并将焦点移入对话框。
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

  useEffect(() => {
    if (!open) return;
    const onKeyDown = (e: KeyboardEvent): void => {
      if (e.key === 'Escape') {
        e.stopPropagation();
        onClose();
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
  }, [open, onClose]);

  if (!open) return null;

  return (
    <div className="fixed inset-0 z-modal flex items-center justify-center p-6">
      <div
        className="absolute inset-0 bg-slate-900/50"
        role="presentation"
        onClick={() => {
          if (closeOnBackdrop) onClose();
        }}
      />
      <div
        ref={panelRef}
        tabIndex={-1}
        role="dialog"
        aria-modal="true"
        aria-label={typeof title === 'string' ? title : '对话框'}
        className={[
          'relative z-10 w-full animate-pop-in rounded-panel bg-white shadow-pop',
          'flex max-h-[88vh] flex-col outline-none',
          widthClass,
        ].join(' ')}
      >
        <header className="flex items-start justify-between gap-4 border-b border-slate-200 px-6 py-5">
          <div className="min-w-0">
            <h2 className="text-2xl font-bold text-ink">{title}</h2>
            {description && <p className="mt-1 text-base text-ink-muted">{description}</p>}
          </div>
          <IconButton icon={<X className="h-6 w-6" />} label="关闭" onClick={onClose} />
        </header>
        <div className="min-h-0 flex-1 overflow-y-auto px-6 py-5">{children}</div>
        {footer && (
          <footer className="flex items-center justify-end gap-3 border-t border-slate-200 px-6 py-4">
            {footer}
          </footer>
        )}
      </div>
    </div>
  );
}
