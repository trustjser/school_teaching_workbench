import { RefreshCw, Wifi, WifiOff } from 'lucide-react';
import { useQueueStore } from '@/store/useQueueStore';
import { useAppStore } from '@/store/useAppStore';
import { useDeviceStore } from '@/store/useDeviceStore';
import { Tooltip } from '@/components/ui/Tooltip';

export interface SyncIndicatorProps {
  compact?: boolean;
}

/**
 * 同步指示器：待发数量 + 补发动画 + 在线/离线灯。
 * 数据来源：useQueueStore（pending_queue）+ useDeviceStore（对端在线数）。
 */
export function SyncIndicator({ compact = false }: SyncIndicatorProps): JSX.Element {
  const pending = useQueueStore((s) => s.pendingCount());
  const dead = useQueueStore((s) => s.deadCount());
  const flushing = useQueueStore((s) => s.flushing);
  const flush = useQueueStore((s) => s.flush);
  const online = useDeviceStore((s) => s.onlineCount());
  const pushToast = useAppStore((s) => s.pushToast);

  const isOnline = online > 0;
  const dotColor = isOnline ? 'bg-green-500' : 'bg-surface-muted';
  const label = isOnline ? `在线 ${online} 个节点` : '未发现节点';

  return (
    <div className="flex items-center gap-3">
      <Tooltip content={label}>
        <span className="inline-flex items-center gap-2">
          <span className={['relative inline-flex h-3.5 w-3.5 rounded-full', dotColor].join(' ')}>
            {isOnline && (
              <span
                className={[
                  'absolute inline-flex h-full w-full rounded-full opacity-75',
                  'animate-ping',
                  dotColor,
                ].join(' ')}
              />
            )}
          </span>
          {isOnline ? (
            <Wifi className="h-6 w-6 text-green-700" aria-hidden />
          ) : (
            <WifiOff className="h-6 w-6 text-ink-muted" aria-hidden />
          )}
          {!compact && <span className="text-base font-semibold text-ink-soft">{label}</span>}
        </span>
      </Tooltip>

      <button
        type="button"
        onClick={() => {
          void flush().then(() => {
            void pushToast({ kind: 'info', title: '已触发一次补发' });
          });
        }}
        className={[
          'inline-flex min-h-touch items-center gap-2 rounded-lg border px-3 font-semibold transition-colors',
          'focus-visible:outline-none focus-visible:ring-4 focus-visible:ring-brand-400',
          pending > 0
            ? 'border-amber-600 bg-amber-50 text-amber-800'
            : 'border-surface-border bg-surface-raised text-ink-soft hover:bg-surface-muted',
        ].join(' ')}
        title="点击立即补发一次"
      >
        <RefreshCw
          className={['h-5 w-5', flushing ? 'animate-spin text-brand-600' : ''].join(' ')}
          aria-hidden
        />
        <span className="text-base">
          {flushing ? '补发中' : pending > 0 ? `待发 ${pending}` : '已同步'}
        </span>
        {dead > 0 && (
          <span className="rounded-full bg-red-100 px-2 py-0.5 text-xs font-bold text-red-800">
            死信 {dead}
          </span>
        )}
      </button>
    </div>
  );
}
