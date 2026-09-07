import { useEffect, useState } from 'react';
import { Plus } from 'lucide-react';
import { useTaskStore } from '@/store/useTaskStore';
import { Card } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';
import { Input } from '@/components/ui/Input';
import { Select } from '@/components/ui/Select';
import { Textarea } from '@/components/ui/Textarea';
import { Toggle } from '@/components/ui/Toggle';
import { Modal } from '@/components/ui/Modal';
import { Table, type TableColumn } from '@/components/ui/Table';
import {
  TASK_TYPE_OPTIONS,
  TASK_STATUS_OPTIONS,
  COLOR_TOKENS,
  COLOR_TOKEN_LABEL,
  NODE_ICON_OPTIONS,
  DEFAULT_NODE_TEMPLATES,
} from '@/constants/status';
import { TASK_NODE_MIN, TASK_NODE_MAX } from '@/constants/app';
import type { ColorToken, TaskScope, TaskType } from '@/types/enums';
import type { CustomTask, TaskStatusNode } from '@/types/models';
import { uuidV4 } from '@/lib/crypto';
import { useAppStore } from '@/store/useAppStore';

/** 任务管理页：任务 CRUD + 状态节点（2~4）编辑 */
export function TaskManage(): JSX.Element {
  const tasks = useTaskStore((s) => s.tasks);
  const currentTaskId = useTaskStore((s) => s.currentTaskId);
  const nodes = useTaskStore((s) => (currentTaskId ? s.nodes[currentTaskId] ?? [] : []));
  const loadTasks = useTaskStore((s) => s.loadTasks);
  const loadMatrix = useTaskStore((s) => s.loadMatrix);
  const setCurrentTask = useTaskStore((s) => s.setCurrentTask);
  const upsertTask = useTaskStore((s) => s.upsertTask);
  const removeTask = useTaskStore((s) => s.removeTask);
  const saveNodes = useTaskStore((s) => s.saveNodes);
  const deleteNode = useTaskStore((s) => s.deleteNode);
  const pushToast = useAppStore((s) => s.pushToast);

  const [createOpen, setCreateOpen] = useState(false);
  const [editNodesOpen, setEditNodesOpen] = useState(false);

  useEffect(() => {
    void loadTasks();
  }, [loadTasks]);

  useEffect(() => {
    if (currentTaskId && nodes.length === 0) void loadMatrix(currentTaskId);
  }, [currentTaskId, nodes.length, loadMatrix]);

  const handleCreate = async (data: NewTaskInput): Promise<void> => {
    const saved = await upsertTask({
      title: data.title,
      description: data.description || null,
      taskType: data.taskType as TaskType,
      scope: data.scope as TaskScope,
      dueAt: data.dueAt,
      viewMode: data.viewMode,
      scoreEnabled: data.scoreEnabled,
      noteEnabled: data.noteEnabled,
      status: 'active',
    });
    const now = Date.now();
    const initNodes: TaskStatusNode[] = DEFAULT_NODE_TEMPLATES.map((n, i) => ({
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
    await loadTasks();
    await loadMatrix(saved.id);
    pushToast({ kind: 'success', title: '任务已创建', description: '已生成默认状态节点' });
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
          <Button size="md" variant="danger" onClick={() => void removeTask(t.id)}>
            删除
          </Button>
        </div>
      ),
    },
  ];

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <h1 className="text-3xl font-bold text-ink">任务管理</h1>
        <Button icon={<Plus className="h-5 w-5" />} onClick={() => setCreateOpen(true)}>
          新建任务
        </Button>
      </div>

      <Card>
        <Table
          columns={columns}
          data={tasks}
          rowKey={(t) => t.id}
          empty={<span>还没有任务，点击「新建任务」创建第一个自定义任务。</span>}
        />
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
              colorToken: n.colorToken,
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

interface EditableNode {
  id?: string;
  nodeKey: string;
  label: string;
  colorToken: ColorToken;
  iconName: string | null;
  nodeOrder: number;
  isFinal: boolean;
  isDefault: boolean;
}

interface NewTaskInput {
  title: string;
  description: string;
  taskType: string;
  scope: string;
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
  const [scope, setScope] = useState('class');
  const [viewMode, setViewMode] = useState<'grid' | 'table'>('grid');
  const [scoreEnabled, setScoreEnabled] = useState(false);
  const [noteEnabled, setNoteEnabled] = useState(false);
  const [saving, setSaving] = useState(false);

  const reset = (): void => {
    setTitle('');
    setDescription('');
    setTaskType('custom');
    setScope('class');
    setViewMode('grid');
    setScoreEnabled(false);
    setNoteEnabled(false);
  };

  const submit = async (): Promise<void> => {
    if (!title.trim()) return;
    setSaving(true);
    try {
      await onSubmit({
        title: title.trim(),
        description,
        taskType,
        scope,
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
          <Button onClick={() => void submit()} loading={saving} disabled={!title.trim()}>
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
            label="作用范围"
            options={[
              { value: 'class', label: '班级' },
              { value: 'grade', label: '年级' },
              { value: 'school', label: '全校' },
            ]}
            value={scope}
            onChange={(e) => setScope(e.target.value)}
          />
        </div>
        <Select
          label="默认视图"
          options={[
            { value: 'grid', label: '网格视图' },
            { value: 'table', label: '表格视图' },
          ]}
          value={viewMode}
          onChange={(e) => setViewMode(e.target.value as 'grid' | 'table')}
        />
        <Toggle label="启用评分" checked={scoreEnabled} onChange={setScoreEnabled} />
        <Toggle label="启用备注" checked={noteEnabled} onChange={setNoteEnabled} />
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
          colorToken: n.colorToken,
          iconName: n.iconName,
          nodeOrder: n.nodeOrder,
          isFinal: n.isFinal,
          isDefault: n.isDefault,
        })),
      );
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open, taskId]);

  const patch = (idx: number, patch: Partial<EditableNode>): void => {
    setItems((prev) => prev.map((n, i) => (i === idx ? { ...n, ...patch } : n)));
  };

  const addNode = (): void => {
    if (items.length >= TASK_NODE_MAX) return;
    setItems((prev) => [
      ...prev,
      {
        id: undefined,
        nodeKey: `node-${uuidV4().slice(0, 4)}`,
        label: '新节点',
        colorToken: (COLOR_TOKENS[prev.length % COLOR_TOKENS.length] as ColorToken) ?? 'blue',
        iconName: 'circle',
        nodeOrder: prev.length,
        isFinal: false,
        isDefault: false,
      },
    ]);
  };

  const removeNode = async (idx: number): Promise<void> => {
    const target = items[idx];
    if (items.length <= TASK_NODE_MIN) return;
    if (target.id) await onDeleteNode(target.id);
    setItems((prev) => prev.filter((_, i) => i !== idx).map((n, i) => ({ ...n, nodeOrder: i })));
  };

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
      <div className="space-y-3">
        {items.map((n, idx) => (
          <div key={n.id ?? n.nodeKey} className="flex flex-wrap items-end gap-3 rounded-lg border border-surface-border p-3">
            <Input
              label="节点名称"
              value={n.label}
              onChange={(e) => patch(idx, { label: e.target.value })}
              className="w-40"
            />
            <Select
              label="配色"
              options={COLOR_TOKENS.map((c) => ({ value: c, label: COLOR_TOKEN_LABEL[c] ?? c }))}
              value={n.colorToken}
              onChange={(e) => patch(idx, { colorToken: e.target.value as ColorToken })}
              className="w-36"
            />
            <Select
              label="图标"
              options={NODE_ICON_OPTIONS}
              value={n.iconName ?? 'circle'}
              onChange={(e) => patch(idx, { iconName: e.target.value })}
              className="w-36"
            />
            <Toggle
              label="终态"
              checked={n.isFinal}
              onChange={(v) => patch(idx, { isFinal: v })}
            />
            <Button
              variant="danger"
              size="md"
              disabled={items.length <= TASK_NODE_MIN}
              onClick={() => void removeNode(idx)}
            >
              删除
            </Button>
          </div>
        ))}
        <Button variant="secondary" onClick={addNode} disabled={items.length >= TASK_NODE_MAX}>
          添加节点
        </Button>
      </div>
    </Modal>
  );
}
