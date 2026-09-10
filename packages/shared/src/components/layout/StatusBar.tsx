import { useAppStore } from '@shared/store/useAppStore';
import { useDeviceStore } from '@shared/store/useDeviceStore';
import { useQueueStore } from '@shared/store/useQueueStore';
import { API_PORT, MDNS_SERVICE_TYPE } from '@shared/constants/app';
import { formatRelative } from '@shared/lib/format';

/** 底部状态栏：本地库路径提示 + 端口 + mDNS 类型 + 已发现节点数 + 待发数量 */
export function StatusBar(): JSX.Element {
  const settings = useAppStore((s) => s.settings);
  const devices = useDeviceStore((s) => s.devices);
  const lastRefreshedAt = useDeviceStore((s) => s.lastRefreshedAt);
  const pending = useQueueStore((s) => s.pendingCount());
  const dead = useQueueStore((s) => s.deadCount());

  const online = devices.filter((d) => d.status === 'online').length;

  return (
    <footer className="flex flex-wrap items-center gap-x-6 gap-y-1 border-t border-surface-border bg-surface-muted px-6 py-2 text-sm text-ink-muted">
      <span>
        设备 ID：<span className="font-mono text-ink-soft">{settings.deviceId || '未初始化'}</span>
      </span>
      <span>
        API 端口：<span className="font-mono text-ink-soft">{settings.apiPort || API_PORT}</span>
      </span>
      <span>
        mDNS：<span className="font-mono text-ink-soft">{MDNS_SERVICE_TYPE}</span>
      </span>
      <span>
        已发现节点：<span className="font-semibold text-ink-soft">{devices.length}</span>（在线{' '}
        <span className="font-semibold text-state-present">{online}</span>）
      </span>
      <span>
        待发队列：<span className="font-semibold text-state-leave">{pending}</span>
        {dead > 0 && <span className="ml-1 text-state-absent">· 死信 {dead}</span>}
      </span>
      <span className="ml-auto">
        上次刷新：{lastRefreshedAt ? formatRelative(lastRefreshedAt) : '尚未刷新'}
      </span>
    </footer>
  );
}
