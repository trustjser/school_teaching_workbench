import { useEffect, useState } from 'react';
import { Link } from 'react-router-dom';
import { Plus, LayoutGrid } from 'lucide-react';
import { useTaskStore } from '@/store/useTaskStore';
import { Card } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';
import { Input } from '@/components/ui/Input';
import { Select } from '@/components/ui/Select';
import { Textarea } from '@/components/ui/Textarea';
import { Toggle } from '@/components/ui/Toggle';
import { Modal } from '@/components/ui/Modal';
import { Table, type TableColumn } from '@/components/ui/Table';
import { SkeletonRows } from '@/components/ui/Skeleton';
import {
  TASK_TYPE_OPTIONS,
  TASK_STATUS_OPTIONS,
  DEFAULT_NODE_TEMPLATES,
} from '@/constants/status';
import { TASK_NODE_MIN, TASK_NODE_MAX } from '@/constants/app';
import type { TaskScope, TaskType, ColorToken } from '@/types/enums';
import type { CustomTask, TaskStatusNode } from '@/types/models';
import { uuidV4 } from '@/lib/crypto';
import { StatusNodeEditor, type EditableNode } from '@/components/task/StatusNodeEditor';
import { useAppStore } from '@/store/useAppStore';

/** 任务中心：统一管理班级自建任务与教务下发任务 */
export function TaskManage(): JSX.Element {
  const tasks = useTaskStore((s) => s.tasks);
  const currentTaskId = useTaskStore((s) => s.currentTaskId);
  const nodes = useTaskStore((s) => (currentTaskId ? s.nodes[currentTaskId] ?? [] : []));
  const loadTaskPage = useTaskStore((s) => s.loadTaskPage);
  const taskPage = useTaskStore((s) => s.taskPage);
  const loadMatrix = useTaskStore((s) => s.loadMatrix);
  const setCurrentTask = useTaskStore((s) => s.setCurrentTask);
  const upsertTask = useTaskStore((s) => s.upsertTask);
  const removeTask = useTaskStore((s) => s.removeTask);
  const saveNodes = useTaskStore((s) => s.saveNodes);
  const deleteNode = useTaskStore((s) => s.deleteNode);
  const pushToast = useAppStore((s) => s.pushToast);

  const [createOpen, setCreateOpen] = useState(false);
  const [editNodesOpen, setEditNodesOpen] = useState(false);
  const [booting, setBooting] = useState(true);
  const [keyword, setKeyword] = useState('');
  const [statusFilter, setStatusFilter] = useState('');
  const [page, setPage] = useState(1);
  const pageSize = 10;

  useEffect(() => {
    void loadTaskPage(page, pageSize, keyword, statusFilter);
    const t = setTimeout(() => setBooting(false), 450);
    return () => clearTimeout(t);
  }, [loadTaskPage, page, pageSize, keyword, statusFilter]);

  useEffect(() => {
    if (currentTaskId && nodes.length === 0) void loadMatrix(currentTaskId);
  }, [currentTaskId, nodes.length, loadMatrix]);

  const handleCreate = async (data: NewTaskInput): Promise<void> => {
    const runtimeSettings = useAppStore.getState().settings;
    const saved = await upsertTask({
      title: data.title,
      description: data.description || null,
      taskType: data.taskType as TaskType,
      scope: 'class' as TaskScope,
      dueAt: data.dueAt,
      viewMode: data.viewMode,
      scoreEnabled: data.scoreEnabled,
      noteEnabled: data.noteEnabled,
      status: 'active',
      grade: runtimeSettings.grade,
      className: runtimeSettings.className,
    });
    const now = Date.now();
    const initNodes: TaskStatusNode[] = data.nodes.map((n, i) => ({
      id: uuidV4(),
      nodeKey: n.nodeKey,
      label: n.label,
      colorToken: n.colorToken as ColorToken,
      iconName: n.iconName,
      nodeOrder: i,
      isFinal: n.isFinal,
      isDefault: n.isDefault,
      taskId: saved.id,
      createdAt: now,
      updatedAt: now,
      deletedAt: null,
      syncState: 'local',
      dirty: false,
    }));
    await saveNodes(saved.id, initNodes);
    await loadTaskPage(page, pageSize, keyword, statusFilter);
    await loadMatrix(saved.id);
    pushToast({ kind: 'success', title: '任务已创建', description: `已生成 ${initNodes.length} 个状态节点` });
  };

  const columns: TableColumn<CustomTask>[] = [
    { key: 'title', header: '任务', accessor: (t) => t.title },
    {
      key: 'type',
      header: '类型',
      accessor: (t) => TASK_TYPE_OPTIONS.find((o) => o.value === t.taskType)?.label ?? t.taskType,
    },
    {
      key: 'scope',
      header: '范围',
      accessor: (t) =>
        ({ class: '班级', grade: '年级', school: '全校' } as Record<string, string>)[t.scope] ?? t.scope,
    },
    {
      key: 'status',
      header: '状态',
      accessor: (t) => TASK_STATUS_OPTIONS.find((o) => o.value === t.status)?.label ?? t.status,
    },
    {
      key: 'actions',
      header: '操作',
      align: 'right',
      render: (t) => (
        <div className="flex justify-end gap-2">
          <Button
            size="md"
            variant="secondary"
            onClick={() => {
              setCurrentTask(t.id);
              setEditNodesOpen(true);
            }}
          >
            状态节点
          </Button>
          <Link to={`/client/matrix?task=${encodeURIComponent(t.id)}`}><Button size="md" variant="secondary">打开看板</Button></Link>
          {t.source === 'broadcast' ? (
            <span className="px-2 py-2 text-sm font-semibold text-ink-muted">教务下发 · 不可删除</span>
          ) : (
            <Button size="md" variant="danger" onClick={() => void removeTask(t.id)}>删除</Button>
          )}
        </div>
      ),
    },
  ];

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <h1 className="text-3xl font-bold text-ink">任务中心</h1>
          <div className="flex gap-2"><Link to="/client/matrix"><Button variant="secondary" icon={<LayoutGrid className="h-5 w-5" />}>打开任务看板</Button></Link><Button icon={<Plus className="h-5 w-5" />} onClick={() => setCreateOpen(true)}>新建任务</Button></div>
      </div>

      <Card>
        <div className="mb-4 grid grid-cols-1 gap-3 sm:grid-cols-[minmax(240px,1fr)_minmax(180px,220px)_auto]">
          <Input label="搜索任务" value={keyword} onChange={(event) => { setKeyword(event.target.value); setPage(1); }} placeholder="按标题搜索" />
          <Select label="状态" options={TASK_STATUS_OPTIONS} value={statusFilter} onChange={(event) => { setStatusFilter(event.target.value); setPage(1); }} placeholder="全部状态" />
          <Button variant="secondary" className="self-end" onClick={() => void loadTaskPage(page, pageSize, keyword, statusFilter)}>搜索</Button>
        </div>
        {booting && tasks.length === 0 ? (
          <SkeletonRows rows={5} />
        ) : (
          <Table
            columns={columns}
            data={tasks}
            rowKey={(t) => t.id}
            empty={<span>还没有任务，点击「新建任务」创建第一个自定义任务。</span>}
          />
        )}
        {taskPage && taskPage.total > pageSize && (
          <div className="mt-4 flex items-center justify-between gap-3 text-sm text-ink-muted">
            <span>共 {taskPage.total} 个任务，第 {taskPage.page} / {Math.ceil(taskPage.total / pageSize)} 页</span>
            <div className="flex gap-2">
              <Button size="md" variant="secondary" disabled={page <= 1} onClick={() => setPage((value) => value - 1)}>上一页</Button>
              <Button size="md" variant="secondary" disabled={page >= Math.ceil(taskPage.total / pageSize)} onClick={() => setPage((value) => value + 1)}>下一页</Button>
            </div>
          </div>
        )}
      </Card>

      <CreateTaskModal
        open={createOpen}
        onClose={() => setCreateOpen(false)}
        onSubmit={handleCreate}
      />
      <NodeEditorModal
        open={editNodesOpen}
        onClose={() => setEditNodesOpen(false)}
        taskId={currentTaskId ?? ''}
        nodes={nodes}
        onSave={async (updated) => {
          if (!currentTaskId) return;
          const converted: TaskStatusNode[] = updated.map((n) => {
            const now = Date.now();
            return {
              id: n.id ?? uuidV4(),
              nodeKey: n.nodeKey,
              label: n.label,
              colorToken: n.colorToken as ColorToken,
              iconName: n.iconName,
              nodeOrder: n.nodeOrder,
              isFinal: n.isFinal,
              isDefault: n.isDefault,
              taskId: currentTaskId,
              createdAt: now,
              updatedAt: now,
              deletedAt: null,
              syncState: 'local',
              dirty: false,
            };
          });
          await saveNodes(currentTaskId, converted);
          await loadMatrix(currentTaskId);
        }}
        onDeleteNode={async (id) => {
          if (currentTaskId) await deleteNode(currentTaskId, id);
        }}
      />
    </div>
  );
}

/* -------------------------------------------------------------------------- */
/* 类型与子组件                                                                */
/* -------------------------------------------------------------------------- */

interface NewTaskInput {
  title: string;
  description: string;
  taskType: string;
  nodes: EditableNode[];
  dueAt: number | null;
  viewMode: 'grid' | 'table';
  scoreEnabled: boolean;
  noteEnabled: boolean;
}

function CreateTaskModal({
  open,
  onClose,
  onSubmit,
}: {
  open: boolean;
  onClose: () => void;
  onSubmit: (data: NewTaskInput) => Promise<void>;
}): JSX.Element {
  const [title, setTitle] = useState('');
  const [description, setDescription] = useState('');
  const [taskType, setTaskType] = useState('custom');
  const [viewMode, setViewMode] = useState<'grid' | 'table'>('grid');
  const [scoreEnabled, setScoreEnabled] = useState(false);
  const [noteEnabled, setNoteEnabled] = useState(false);
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

  const reset = (): void => {
    setTitle('');
    setDescription('');
    setTaskType('custom');
    setViewMode('grid');
    setScoreEnabled(false);
    setNoteEnabled(false);
    setNodes(
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
  };

  const submit = async (): Promise<void> => {
    if (!title.trim()) return;
    if (nodes.length < TASK_NODE_MIN || nodes.length > TASK_NODE_MAX) return;
    setSaving(true);
    try {
      await onSubmit({
        title: title.trim(),
        description,
        taskType,
        nodes,
        dueAt: null,
        viewMode,
        scoreEnabled,
        noteEnabled,
      });
      reset();
      onClose();
    } catch {
      /* store 已 toast */
    } finally {
      setSaving(false);
    }
  };

  return (
    <Modal
      open={open}
      onClose={onClose}
      title="新建自定义任务"
      footer={
        <>
          <Button variant="secondary" onClick={onClose}>
            取消
          </Button>
          <Button
            onClick={() => void submit()}
            loading={saving}
            disabled={!title.trim() || nodes.length < TASK_NODE_MIN || nodes.length > TASK_NODE_MAX}
          >
            创建
          </Button>
        </>
      }
    >
      <div className="space-y-4">
        <Input label="任务标题" value={title} onChange={(e) => setTitle(e.target.value)} placeholder="如：古诗背诵" />
        <Textarea label="描述" value={description} onChange={(e) => setDescription(e.target.value)} />
        <div className="grid grid-cols-2 gap-4">
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
        <div className="rounded-lg border border-surface-border bg-surface-muted px-3 py-2 text-sm text-ink-soft">
          作用范围固定为「本班级」，班级内创建的任务仅对本班学生生效，无法选择年级或全校。
        </div>
        <Toggle label="启用评分" checked={scoreEnabled} onChange={setScoreEnabled} />
        <Toggle label="启用备注" checked={noteEnabled} onChange={setNoteEnabled} />
        <StatusNodeEditor nodes={nodes} onChange={setNodes} />
      </div>
    </Modal>
  );
}

function NodeEditorModal({
  open,
  onClose,
  taskId,
  nodes,
  onSave,
  onDeleteNode,
}: {
  open: boolean;
  onClose: () => void;
  taskId: string;
  nodes: TaskStatusNode[];
  onSave: (items: EditableNode[]) => Promise<void>;
  onDeleteNode: (id: string) => Promise<void>;
}): JSX.Element {
  const [items, setItems] = useState<EditableNode[]>([]);

  useEffect(() => {
    if (open && taskId) {
      setItems(
        nodes.map((n) => ({
          id: n.id,
          nodeKey: n.nodeKey,
          label: n.label,
          colorToken: n.colorToken as ColorToken,
          iconName: n.iconName,
          nodeOrder: n.nodeOrder,
          isFinal: n.isFinal,
          isDefault: n.isDefault,
        })),
      );
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open, taskId]);

  const save = async (): Promise<void> => {
    if (items.length < TASK_NODE_MIN || items.length > TASK_NODE_MAX) return;
    await onSave(items);
    onClose();
  };

  return (
    <Modal
      open={open}
      onClose={onClose}
      title="编辑状态节点"
      description={`每个任务 2~4 个状态节点（当前 ${items.length} 个）`}
      widthClass="max-w-3xl"
      footer={
        <>
          <Button variant="secondary" onClick={onClose}>
            取消
          </Button>
          <Button onClick={() => void save()} disabled={items.length < TASK_NODE_MIN || items.length > TASK_NODE_MAX}>
            保存节点
          </Button>
        </>
      }
    >
      <StatusNodeEditor
        nodes={items}
        onChange={setItems}
        onRemoveNode={async (n) => {
          if (n.id) await onDeleteNode(n.id);
        }}
      />
    </Modal>
  );
}
