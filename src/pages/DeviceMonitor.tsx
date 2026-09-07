import { useEffect } from 'react';
import { useDeviceStore } from '@/store/useDeviceStore';
import { Card } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';
import { Table, type TableColumn } from '@/components/ui/Table';
import { Badge } from '@/components/ui/Badge';
import { DEVICE_STATUS_META } from '@/constants/status';
import { formatRelative } from '@/lib/format';
import type { Device } from '@/types/models';

/** 节点监控页：局域网内所有发现节点的状态、角色、IP、最近活跃 */
export function DeviceMonitor(): JSX.Element {
  const devices = useDeviceStore((s) => s.devices);
  const load = useDeviceStore((s) => s.load);
  const refresh = useDeviceStore((s) => s.refresh);
  const forget = useDeviceStore((s) => s.forget);

  useEffect(() => {
    void load();
  }, [load]);

  const columns: TableColumn<Device>[] = [
    { key: 'name', header: '设备名', accessor: (d) => d.deviceName },
    {
      key: 'role',
      header: '角色',
      accessor: (d) => (d.deviceRole === 'master' ? '教务处端' : '班级端'),
    },
    {
      key: 'cls',
      header: '班级',
      accessor: (d) => d.txtClassName ?? d.txtGrade ?? '—',
    },
    { key: 'ip', header: 'IP', accessor: (d) => d.ipAddress ?? '—' },
    {
      key: 'status',
      header: '状态',
      render: (d) => (
        <Badge tone={d.status === 'online' ? 'success' : 'neutral'}>
          {DEVICE_STATUS_META[d.status]?.label ?? d.status}
        </Badge>
      ),
    },
    {
      key: 'seen',
      header: '最近发现',
      accessor: (d) => (d.lastSeenAt ? formatRelative(d.lastSeenAt) : '—'),
    },
    {
      key: 'actions',
      header: '操作',
      align: 'right',
      render: (d) =>
        d.isSelf ? (
          <Badge tone="brand">本机</Badge>
        ) : (
          <Button size="md" variant="danger" onClick={() => void forget(d.deviceId)}>
            忽略
          </Button>
        ),
    },
  ];

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <h1 className="text-3xl font-bold text-ink">节点监控</h1>
        <Button variant="secondary" onClick={() => void refresh()}>
          刷新
        </Button>
      </div>
      <Card>
        <Table
          columns={columns}
          data={devices}
          rowKey={(d) => d.deviceId}
          empty={<span>未发现局域网节点，请确认各端已启动且处于同一网络。</span>}
        />
      </Card>
    </div>
  );
}
