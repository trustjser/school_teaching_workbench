import { useEffect, useMemo, useState } from 'react';
import { Plus, RefreshCw } from 'lucide-react';
import { useBroadcastStore, summarizeReceipts } from '@/store/useBroadcastStore';
import { useDeviceStore } from '@/store/useDeviceStore';
import { useAppStore } from '@/store/useAppStore';
import { Card } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';
import { Input } from '@/components/ui/Input';
import { Textarea } from '@/components/ui/Textarea';
import { Select } from '@/components/ui/Select';
import { Modal } from '@/components/ui/Modal';
import { Table, type TableColumn } from '@/components/ui/Table';
import { Badge } from '@/components/ui/Badge';
import { Toggle } from '@/components/ui/Toggle';
import { EmptyState } from '@/components/ui/EmptyState';
import { SkeletonRows } from '@/components/ui/Skeleton';
import { StatusNodeEditor, type EditableNode } from '@/components/task/StatusNodeEditor';
import { TASK_TYPE_OPTIONS, PRIORITY_OPTIONS, DEFAULT_NODE_TEMPLATES } from '@/constants/status';
import { TASK_NODE_MIN, TASK_NODE_MAX } from '@/constants/app';
import type { BroadcastPriority, BroadcastTargetType, ColorToken } from '@/types/enums';
import type { BroadcastTask } from '@/types/broadcast';
import {
  stringifyBroadcastPayload,
  resolveTargetDeviceIds,
  type BroadcastNodeTemplate,
  type BroadcastTaskTemplate,
} from '@/types/broadcast';
import type { BroadcastReceipt } from '@/types/broadcast';
import { formatDateTime } from '@/lib/format';

/** 任务下发中心（教务处端）：创建广播任务 → 选择目标 → 下发 */
export function BroadcastCenter(): JSX.Element {
  const outbox = useBroadcastStore((s) => s.outbox);
  const receipts = useBroadcastStore((s) => s.receipts);
  const loadOutbox = useBroadcastStore((s) => s.loadOutbox);
  const create = useBroadcastStore((s) => s.create);
  const send = useBroadcastStore((s) => s.send);
  const loadReceipts = useBroadcastStore((s) => s.loadReceipts);

  const devices = useDeviceStore((s) => s.devices);
  const loadDevices = useDeviceStore((s) => s.load);

  const [createOpen, setCreateOpen] = useState(false);
  const [booting, setBooting] = useState(true);

  useEffect(() => {
    void loadOutbox();
    void loadDevices();
    const t = setTimeout(() => setBooting(false), 450);
    return () => clearTimeout(t);
  }, [loadOutbox, loadDevices]);

  const handleCreate = async (p: CreatePayload): Promise<void> => {
    // Rust 侧只认 device_id 数组，选择器在这里展开。
    const targetDeviceIds = resolveTargetDeviceIds(
      { targetType: p.targetType, values: p.targetValues },
      devices,
    );
    if (targetDeviceIds.length === 0) {
      useAppStore.getState().pushToast({
        kind: 'warning',
        title: '没有匹配到目标设备',
        description: '请先刷新设备列表，或调整下发范围',
      });
      return;
    }

    const template: BroadcastTaskTemplate = {
      title: p.title,
      description: p.description || null,
      taskType: p.taskType,
      dueAt: p.dueAt,
      scoreEnabled: p.scoreEnabled,
      noteEnabled: p.noteEnabled,
      viewMode: p.viewMode,
      statusNodes: p.nodes,
    };
    const created = await create({
      title: p.title,
      payload: stringifyBroadcastPayload(template),
      description: p.description || null,
      priority: p.priority as BroadcastPriority,
      targetType: p.targetType,
      dueAt: p.dueAt,
    });
    await send(created.id, targetDeviceIds);
    await loadOutbox();
  };

  const columns: TableColumn<BroadcastTask>[] = [
    { key: 'title', header: '任务', accessor: (t) => t.title },
    {
      key: 'priority',
      header: '优先级',
      render: (t) => (
        <Badge tone={t.priority === 'urgent' ? 'danger' : t.priority === 'high' ? 'warning' : 'brand'}>
          {PRIORITY_OPTIONS.find((o) => o.value === t.priority)?.label ?? t.priority}
        </Badge>
      ),
    },
    {
      key: 'status',
      header: '状态',
      render: (t) => <Badge tone={t.status === 'sent' ? 'success' : 'neutral'}>{t.status}</Badge>,
    },
    {
      key: 'sentAt',
      header: '下发时间',
      accessor: (t) => (t.sentAt ? formatDateTime(t.sentAt) : '—'),
    },
    {
      key: 'expect',
      header: '目标数',
      accessor: (t) => t.expectCount,
      align: 'center',
    },
    {
      key: 'receipt',
      header: '回执',
      align: 'center',
      render: (t) => {
        const list: BroadcastReceipt[] = receipts[t.id] ?? [];
        if (list.length === 0) return <span className="text-ink-muted">—</span>;
        const s = summarizeReceipts(list);
        return (
          <span className="text-sm">
            收 {s.received} · 接受 {s.accepted} · 完成 {s.done}
          </span>
        );
      },
    },
    {
      key: 'actions',
      header: '操作',
      align: 'right',
      render: (t) => (
        <Button size="md" variant="secondary" icon={<RefreshCw className="h-4 w-4" />} onClick={() => void loadReceipts(t.id)}>
          回执
        </Button>
      ),
    },
  ];

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <h1 className="text-3xl font-bold text-ink">任务下发</h1>
        <Button icon={<Plus className="h-5 w-5" />} onClick={() => setCreateOpen(true)}>
          新建并下发
        </Button>
      </div>

      <Card>
        {booting && outbox.length === 0 ? (
          <SkeletonRows rows={5} />
        ) : outbox.length === 0 ? (
          <EmptyState title="尚未下发任务" description="点击「新建并下发」向班级 / 年级 / 全校推送任务。" />
        ) : (
          <Table columns={columns} data={outbox} rowKey={(t) => t.id} />
        )}
      </Card>

      <CreateBroadcastModal
        open={createOpen}
        onClose={() => setCreateOpen(false)}
        devices={devices}
        onSubmit={handleCreate}
      />
    </div>
  );
}

/* -------------------------------------------------------------------------- */
/* 子组件：新建广播任务弹窗                                                      */
/* -------------------------------------------------------------------------- */

export interface CreatePayload {
  title: string;
  description: string;
  priority: string;
  taskType: string;
  dueAt: number | null;
  scoreEnabled: boolean;
  noteEnabled: boolean;
  viewMode: 'grid' | 'table';
  targetType: BroadcastTargetType;
  targetValues: string[];
  nodes: BroadcastNodeTemplate[];
}

function CreateBroadcastModal({
  open,
  onClose,
  devices,
  onSubmit,
}: {
  open: boolean;
  onClose: () => void;
  devices: { deviceId: string; deviceName: string; isSelf: boolean }[];
  onSubmit: (data: CreatePayload) => Promise<void>;
}): JSX.Element {
  const [title, setTitle] = useState('');
  const [description, setDescription] = useState('');
  const [priority, setPriority] = useState('normal');
  const [taskType, setTaskType] = useState('custom');
  const [viewMode, setViewMode] = useState<'grid' | 'table'>('grid');
  const [scoreEnabled, setScoreEnabled] = useState(false);
  const [noteEnabled, setNoteEnabled] = useState(false);
  const [targetType, setTargetType] = useState<BroadcastTargetType>('school');
  const [targetText, setTargetText] = useState('');
  const [targetDevices, setTargetDevices] = useState<string[]>([]);
  const [nodes, setNodes] = useState<EditableNode[]>(() =>
    DEFAULT_NODE_TEMPLATES.map((n, i) => ({
      nodeKey: n.nodeKey,
      label: n.label,
      colorToken: n.colorToken as ColorToken,
      iconName: n.iconName,
      nodeOrder: i,
      isFinal: n.isFinal,
      isDefault: n.isDefault,
    })),
  );
  const [saving, setSaving] = useState(false);

  const resolveTargetValues = (): string[] => {
    if (targetType === 'school') return [];
    if (targetType === 'device') return targetDevices;
    return targetText
      .split(/[,，\s]+/)
      .map((s) => s.trim())
      .filter(Boolean);
  };

  const submit = async (): Promise<void> => {
    if (!title.trim()) return;
    if (nodes.length < TASK_NODE_MIN || nodes.length > TASK_NODE_MAX) return;
    if (targetType === 'device' && targetDevices.length === 0) return;
    if ((targetType === 'grade' || targetType === 'class') && resolveTargetValues().length === 0) return;

    setSaving(true);
    try {
      await onSubmit({
        title: title.trim(),
        description,
        priority,
        taskType,
        dueAt: null,
        scoreEnabled,
        noteEnabled,
        viewMode,
        targetType,
        targetValues: resolveTargetValues(),
        nodes: nodes.map((n) => ({
          nodeKey: n.nodeKey,
          label: n.label,
          colorToken: n.colorToken,
          iconName: n.iconName,
          nodeOrder: n.nodeOrder,
          isFinal: n.isFinal,
          isDefault: n.isDefault,
        })),
      });
      onClose();
    } catch {
      /* store 已 toast */
    } finally {
      setSaving(false);
    }
  };

  const deviceOptions = useMemo(
    () => devices.filter((d) => !d.isSelf),
    [devices],
  );

  return (
    <Modal
      open={open}
      onClose={onClose}
      title="新建并下发任务"
      widthClass="max-w-3xl"
      footer={
        <>
          <Button variant="secondary" onClick={onClose}>
            取消
          </Button>
          <Button onClick={() => void submit()} loading={saving} disabled={!title.trim()}>
            创建并下发
          </Button>
        </>
      }
    >
      <div className="space-y-5">
        <Input label="任务标题" value={title} onChange={(e) => setTitle(e.target.value)} placeholder="如：周末安全提醒" />
        <Textarea label="描述" value={description} onChange={(e) => setDescription(e.target.value)} />

        <div className="grid grid-cols-1 gap-4 sm:grid-cols-3">
          <Select
            label="优先级"
            options={PRIORITY_OPTIONS}
            value={priority}
            onChange={(e) => setPriority(e.target.value)}
          />
          <Select
            label="任务类型"
            options={TASK_TYPE_OPTIONS}
            value={taskType}
            onChange={(e) => setTaskType(e.target.value)}
          />
          <Select
            label="默认视图"
            options={[
              { value: 'grid', label: '网格视图' },
              { value: 'table', label: '表格视图' },
            ]}
            value={viewMode}
            onChange={(e) => setViewMode(e.target.value as 'grid' | 'table')}
          />
        </div>

        <div className="space-y-2">
          <Select
            label="下发目标"
            options={[
              { value: 'school', label: '全校' },
              { value: 'grade', label: '指定年级' },
              { value: 'class', label: '指定班级' },
              { value: 'device', label: '指定设备' },
            ]}
            value={targetType}
            onChange={(e) => setTargetType(e.target.value as BroadcastTargetType)}
          />
          {targetType === 'school' && (
            <p className="text-sm text-ink-muted">将向局域网内全部班级端下发。</p>
          )}
          {(targetType === 'grade' || targetType === 'class') && (
            <Input
              placeholder={targetType === 'grade' ? '如：三年级（逗号分隔多个）' : '如：三年级二班（逗号分隔多个）'}
              value={targetText}
              onChange={(e) => setTargetText(e.target.value)}
            />
          )}
          {targetType === 'device' && (
            <div className="grid max-h-48 grid-cols-1 gap-2 overflow-y-auto pr-1 board:grid-cols-2">
              {deviceOptions.length === 0 ? (
                <p className="text-sm text-ink-muted">未发现其他班级端节点。</p>
              ) : (
                deviceOptions.map((d) => {
                  const checked = targetDevices.includes(d.deviceId);
                  return (
                    <label
                      key={d.deviceId}
                      className={[
                        'flex min-h-touch items-center gap-3 rounded-lg border px-3',
                        checked ? 'border-brand-600 bg-brand-50' : 'border-surface-border bg-surface-raised',
                      ].join(' ')}
                    >
                      <input
                        type="checkbox"
                        className="h-5 w-5 accent-brand-600"
                        checked={checked}
                        onChange={() =>
                          setTargetDevices((prev) =>
                            checked ? prev.filter((id) => id !== d.deviceId) : [...prev, d.deviceId],
                          )
                        }
                      />
                      <span className="text-base font-semibold text-ink">{d.deviceName}</span>
                    </label>
                  );
                })
              )}
            </div>
          )}
        </div>

        <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
          <Toggle label="启用评分" checked={scoreEnabled} onChange={setScoreEnabled} />
          <Toggle label="启用备注" checked={noteEnabled} onChange={setNoteEnabled} />
        </div>

        <StatusNodeEditor nodes={nodes} onChange={setNodes} />
      </div>
    </Modal>
  );
}
