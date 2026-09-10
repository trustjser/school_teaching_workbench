import { useEffect } from 'react';
import { AlertTriangle, CheckCircle2, Info, Loader2, X, XCircle } from 'lucide-react';
import { useAppStore, type ToastItem } from '@shared/store/useAppStore';

const KIND_STYLE: Record<
  ToastItem['kind'],
  { border: string; bg: string; text: string; icon: JSX.Element }
> = {
  success: {
    border: 'border-green-600',
    bg: 'bg-surface-raised',
    text: 'text-green-800',
    icon: <CheckCircle2 className="h-7 w-7 text-green-600" aria-hidden />,
  },
  error: {
    border: 'border-red-600',
    bg: 'bg-surface-raised',
    text: 'text-red-800',
    icon: <XCircle className="h-7 w-7 text-red-600" aria-hidden />,
  },
  warning: {
    border: 'border-amber-600',
    bg: 'bg-surface-raised',
    text: 'text-amber-800',
    icon: <AlertTriangle className="h-7 w-7 text-amber-600" aria-hidden />,
  },
  info: {
    border: 'border-brand-600',
    bg: 'bg-surface-raised',
    text: 'text-brand-800',
    icon: <Info className="h-7 w-7 text-brand-600" aria-hidden />,
  },
  pending: {
    border: 'border-surface-border',
    bg: 'bg-surface-raised',
    text: 'text-ink-soft',
    icon: <Loader2 className="h-6 w-6 animate-spin text-brand-600" aria-hidden />,
  },
};

/** 单条 Toast */
function ToastRow({ item }: { item: ToastItem }): JSX.Element {
  const dismiss = useAppStore((s) => s.dismissToast);
  const style = KIND_STYLE[item.kind];

  useEffect(() => {
    const timer = window.setTimeout(() => dismiss(item.id), item.duration);
    return () => window.clearTimeout(timer);
  }, [dismiss, item.duration, item.id]);

  return (
    <div
      role="status"
      aria-live="polite"
      className={[
        'pointer-events-auto flex w-[22rem] animate-toast-in items-start gap-3 rounded-lg border-l-4',
        'border border-surface-border px-4 py-3 shadow-pop',
        style.border,
        style.bg,
      ].join(' ')}
    >
      <span className="mt-0.5 shrink-0">{style.icon}</span>
      <div className="min-w-0 flex-1">
        <p className={['text-base font-bold', style.text].join(' ')}>{item.title}</p>
        {item.description && (
          <p className="mt-0.5 break-words text-sm text-ink-muted">{item.description}</p>
        )}
      </div>
      <button
        type="button"
        onClick={() => dismiss(item.id)}
        aria-label="关闭提示"
        className="shrink-0 rounded p-1 text-ink-muted transition-colors hover:bg-surface-muted hover:text-ink"
      >
        <X className="h-5 w-5" aria-hidden />
      </button>
    </div>
  );
}

/** 全局 Toast 容器（在 App.tsx 挂载一次） */
export function ToastHost(): JSX.Element {
  const toasts = useAppStore((s) => s.toasts);
  return (
    <div className="pointer-events-none fixed right-6 top-6 z-toast flex flex-col gap-3">
      {toasts.slice(-5).map((t) => (
        <ToastRow key={t.id} item={t} />
      ))}
    </div>
  );
}
