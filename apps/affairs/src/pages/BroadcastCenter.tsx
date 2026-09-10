import { useEffect, useMemo, useState } from 'react';
import { Plus, RefreshCw } from 'lucide-react';
import { useBroadcastStore, summarizeReceipts } from '@shared/store/useBroadcastStore';
import { useDeviceStore } from '@shared/store/useDeviceStore';
import { useAppStore } from '@shared/store/useAppStore';
import { Card } from '@shared/components/ui/Card';
import { Button } from '@shared/components/ui/Button';
import { Input } from '@shared/components/ui/Input';
import { Textarea } from '@shared/components/ui/Textarea';
import { Select } from '@shared/components/ui/Select';
import { Modal } from '@shared/components/ui/Modal';
import { Table, type TableColumn } from '@shared/components/ui/Table';
import { Badge } from '@shared/components/ui/Badge';
import { Toggle } from '@shared/components/ui/Toggle';
import { EmptyState } from '@shared/components/ui/EmptyState';
import { ConfirmDialog } from '@shared/components/ui/ConfirmDialog';
import { SkeletonRows } from '@shared/components/ui/Skeleton';
import { StatusNodeEditor, type EditableNode } from '@shared/components/task/StatusNodeEditor';
import { TASK_TYPE_OPTIONS, BROADCAST_STATUS_OPTIONS, PRIORITY_OPTIONS, DEFAULT_NODE_TEMPLATES } from '@shared/constants/status';
import { TASK_NODE_MIN, TASK_NODE_MAX } from '@shared/constants/app';
import type { BroadcastPriority, BroadcastTargetType, ColorToken } from '@shared/types/enums';
import type { BroadcastTask } from '@shared/types/broadcast';
import {
  stringifyBroadcastPayload,
  resolveTargetDeviceIds,
  type BroadcastNodeTemplate,
  type BroadcastTaskTemplate,
} from '@shared/types/broadcast';
import type { BroadcastReceipt } from '@shared/types/broadcast';
import { formatDateTime } from '@shared/lib/format';

/** 任务下发中心（教务处端）：创建广播任务 → 选择目标 → 下发 */
export function BroadcastCenter(): JSX.Element {
  const outbox = useBroadcastStore((s) => s.outbox);
  const receipts = useBroadcastStore((s) => s.receipts);
  const loadOutboxPage = useBroadcastStore((s) => s.loadOutboxPage);
  const outboxPage = useBroadcastStore((s) => s.outboxPage);
  const create = useBroadcastStore((s) => s.create);
  const send = useBroadcastStore((s) => s.send);
  const loadReceipts = useBroadcastStore((s) => s.loadReceipts);

  const devices = useDeviceStore((s) => s.devices);
  const loadDevices = useDeviceStore((s) => s.load);

  const [createOpen, setCreateOpen] = useState(false);
  const [booting, setBooting] = useState(true);
  const [keyword, setKeyword] = useState('');
  const [statusFilter, setStatusFilter] = useState('');
  /** 是否处于筛选态：决定空状态是「没有数据」还是「没有匹配」 */
  const hasFilter = keyword.trim() !== '' || statusFilter !== '';
  /** 待执行的取消 / 关闭操作：非空即弹出二次确认 */
  const [actionTarget, setActionTarget] = useState<{ task: BroadcastTask; kind: 'cancel' | 'close' } | null>(null);
  const [actionSaving, setActionSaving] = useState(false);
  const cancelOutbox = useBroadcastStore((s) => s.cancelOutbox);
  const closeOutbox = useBroadcastStore((s) => s.closeOutbox);

  const applyAction = async (): Promise<void> => {
    if (!actionTarget) return;
    setActionSaving(true);
    try {
      if (actionTarget.kind === 'cancel') await cancelOutbox(actionTarget.task.id);
      else await closeOutbox(actionTarget.task.id);
    } catch {
      // store 已 toast 后端返回的原因（例如「已有班级接收，请改用关闭」）。
    } finally {
      setActionSaving(false);
      setActionTarget(null);
    }
  };
  const [page, setPage] = useState(1);
  const pageSize = 10;

  useEffect(() => {
    void loadOutboxPage(page, pageSize, keyword, statusFilter);
    void loadDevices();
    const t = setTimeout(() => setBooting(false), 450);
    return () => clearTimeout(t);
  }, [loadOutboxPage, loadDevices, page, pageSize, keyword, statusFilter]);

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
    await loadOutboxPage(page, pageSize, keyword, statusFilter);
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
      render: (t) => <Badge tone={t.status === 'sent' ? 'success' : 'neutral'}>{BROADCAST_STATUS_OPTIONS.find((o) => o.value === t.status)?.label ?? t.status}</Badge>,
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
      render: (t) => {
        // 终态不再提供操作。
        const terminal = t.status === 'closed' || t.status === 'cancelled';
        // 已有班级接收就撤回不了，只能「关闭」；否则给「取消」。
        const canCancel = !terminal && !t.delivered;
        return (
          <div className="flex justify-end gap-2">
            <Button size="md" variant="secondary" icon={<RefreshCw className="h-4 w-4" />} onClick={() => void loadReceipts(t.id)}>
              回执
            </Button>
            {canCancel && (
              <Button size="md" variant="ghost" onClick={() => setActionTarget({ task: t, kind: 'cancel' })}>
                取消
              </Button>
            )}
            {!terminal && (
              <Button size="md" variant="secondary" onClick={() => setActionTarget({ task: t, kind: 'close' })}>
                关闭
              </Button>
            )}
            {terminal && <span className="px-2 py-2 text-sm font-semibold text-ink-muted">已结束</span>}
          </div>
        );
      },
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
        <div className="mb-4 grid grid-cols-1 gap-3 sm:grid-cols-[minmax(240px,1fr)_minmax(180px,220px)_auto]">
          <Input label="搜索任务" value={keyword} onChange={(event) => { setKeyword(event.target.value); setPage(1); }} placeholder="按标题搜索" />
          <Select label="状态" options={BROADCAST_STATUS_OPTIONS} value={statusFilter} onChange={(event) => { setStatusFilter(event.target.value); setPage(1); }} placeholder="全部状态" />
          <Button variant="secondary" className="self-end" onClick={() => void loadOutboxPage(page, pageSize, keyword, statusFilter)}>搜索</Button>
        </div>
        {booting && outbox.length === 0 ? (
          <SkeletonRows rows={5} />
        ) : outbox.length === 0 ? (
          hasFilter ? (
            <EmptyState
              title="没有符合筛选条件的任务"
              description="当前的关键词或状态筛选没有匹配到下发任务，可以清空条件后重新查看。"
              action={
                <Button
                  variant="secondary"
                  onClick={() => {
                    setKeyword('');
                    setStatusFilter('');
                    setPage(1);
                  }}
                >
                  清空筛选
                </Button>
              }
            />
          ) : (
            <EmptyState
              title="还没有下发过任务"
              description="点击「新建并下发」，选择班级、年级或全校后推送任务；班级端收到会自动生成待办，处理进度通过回执回流到这里。"
              action={
                <Button icon={<Plus className="h-5 w-5" />} onClick={() => setCreateOpen(true)}>
                  新建并下发
                </Button>
              }
            />
          )
        ) : (
          <Table columns={columns} data={outbox} rowKey={(t) => t.id} />
        )}
        {outboxPage && outboxPage.total > pageSize && (
          <div className="mt-4 flex items-center justify-between gap-3 text-sm text-ink-muted">
            <span>共 {outboxPage.total} 个任务，第 {outboxPage.page} / {Math.ceil(outboxPage.total / pageSize)} 页</span>
            <div className="flex gap-2">
              <Button size="md" variant="secondary" disabled={page <= 1} onClick={() => setPage((value) => value - 1)}>上一页</Button>
              <Button size="md" variant="secondary" disabled={page >= Math.ceil(outboxPage.total / pageSize)} onClick={() => setPage((value) => value + 1)}>下一页</Button>
            </div>
          </div>
        )}
      </Card>

      <CreateBroadcastModal
        open={createOpen}
        onClose={() => setCreateOpen(false)}
        devices={devices}
        onSubmit={handleCreate}
      />

      <ConfirmDialog
        open={actionTarget !== null}
        title={actionTarget?.kind === 'cancel' ? '取消下发' : '关闭下发'}
        message={
          actionTarget?.kind === 'cancel'
            ? `确定要取消「${actionTarget?.task.title ?? ''}」吗？`
            : `确定要关闭「${actionTarget?.task.title ?? ''}」吗？`
        }
        detail={
          actionTarget?.kind === 'cancel'
            ? '尚未投递的目标会被撤回，不会再送达班级端；已经收到的班级不受影响。取消不会记录发送时间。'
            : '关闭后该下发不再出现在待处理列表里。这不会改动班级端已经收到的任务，班级端仍可继续标记。'
        }
        confirmText={actionTarget?.kind === 'cancel' ? '取消下发' : '关闭下发'}
        cancelText={actionTarget?.kind === 'cancel' ? '再想想' : '取消'}
        danger={actionTarget?.kind === 'cancel'}
        loading={actionSaving}
        onConfirm={() => void applyAction()}
        onCancel={() => setActionTarget(null)}
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
