import { create } from 'zustand';
import type { CustomTask, TaskRecord, TaskStatusNode } from '@/types/models';
import type { Page, TaskCompletionRow, TaskProgressRow } from '@/types/api';
import {
  taskCompletionStats,
  taskDelete,
  taskList,
  taskPage,
  taskMatrixQuery,
  taskClassMatrixQuery,
  taskNodeDelete,
  taskNodeList,
  taskNodeUpsert,
  taskProgressList,
  taskRecordUpsert,
  taskRecordsBatchUpsert,
  taskUpsert,
} from '@/lib/db';
import { useAppStore } from './useAppStore';
import { useStudentStore } from './useStudentStore';
import { TASK_NODE_MAX, TASK_NODE_MIN } from '@/constants/app';

/**
 * 自定义任务引擎状态。
 * 矩阵 = 学生 × 任务节点；单元格点击循环切换到下一节点（末尾回到第一个）。
 */

interface TaskState {
  tasks: CustomTask[];
  loading: boolean;
  currentTaskId: string | null;
  /** taskId -> 有序节点列表 */
  nodes: Record<string, TaskStatusNode[]>;
  /** taskId -> studentId -> 记录 */
  records: Record<string, Record<string, TaskRecord>>;
  /** 视图模式：grid（大屏卡片） / table（高密度表格） */
  viewMode: 'grid' | 'table';
  saving: Record<string, boolean>;
  completionStats: TaskCompletionRow[];
  progress: TaskProgressRow[];
  taskPage: Page<CustomTask> | null;

  loadTasks: () => Promise<void>;
  loadTaskPage: (page: number, pageSize: number, keyword?: string | null, status?: string | null) => Promise<void>;
  loadNodes: (taskId: string) => Promise<TaskStatusNode[]>;
  loadMatrix: (taskId: string) => Promise<void>;
  setCurrentTask: (taskId: string | null) => void;
  setViewMode: (mode: 'grid' | 'table') => void;

  upsertTask: (task: Partial<CustomTask> & { title: string }) => Promise<CustomTask>;
  removeTask: (taskId: string) => Promise<void>;
  saveNodes: (taskId: string, nodes: TaskStatusNode[]) => Promise<TaskStatusNode[]>;
  deleteNode: (taskId: string, nodeId: string) => Promise<void>;

  /** 单元格点击：循环到下一节点 */
  cycleCell: (taskId: string, studentId: string) => Promise<void>;
  /** 直接设置节点 */
  setCellNode: (taskId: string, studentId: string, nodeKey: string) => Promise<void>;
  /** 设置评分（0-100） */
  setCellScore: (taskId: string, studentId: string, score: number | null) => Promise<void>;
  /** 设置备注 */
  setCellNote: (taskId: string, studentId: string, note: string | null) => Promise<void>;
  saveRecordPatch: (taskId: string, studentId: string, nodeKey: string, score?: number | null, note?: string | null) => Promise<void>;
  batchSetNode: (taskId: string, studentIds: string[], nodeKey: string) => Promise<void>;

  /** 取有效节点 key */
  effectiveNodeKey: (taskId: string, studentId: string) => string;
  /** 各节点人数分布 */
  nodeDistribution: (taskId: string, students: { id: string }[]) => Record<string, number>;
  loadCompletionStats: () => Promise<void>;
  loadProgress: (taskId: string, grade?: string | null, className?: string | null) => Promise<TaskProgressRow[]>;
  loadClassMatrix: (taskId: string, className: string) => Promise<void>;
}

export const useTaskStore = create<TaskState>((set, get) => ({
  tasks: [],
  loading: false,
  currentTaskId: null,
  nodes: {},
  records: {},
  viewMode: 'grid',
  saving: {},
  completionStats: [],
  progress: [],
  taskPage: null,

  loadTasks: async () => {
    const app = useAppStore.getState();
    set({ loading: true });
    try {
      const list = await taskList();
      set({ tasks: list });
      if (!get().currentTaskId && list.length > 0) {
        set({ currentTaskId: list[0].id });
      }
    } catch (err) {
      app.toastError(err, '加载任务失败');
    } finally {
      set({ loading: false });
    }
  },

  loadTaskPage: async (page, pageSize, keyword, status) => {
    const app = useAppStore.getState();
    set({ loading: true });
    try {
      const result = await taskPage(page, pageSize, keyword, status);
      set({ taskPage: result, tasks: result.items });
      if (!get().currentTaskId && result.items.length > 0) set({ currentTaskId: result.items[0].id });
    } catch (err) {
      app.toastError(err, '加载任务列表失败');
    } finally {
      set({ loading: false });
    }
  },

  loadNodes: async (taskId) => {
    const app = useAppStore.getState();
    try {
      const list = await taskNodeList(taskId);
      const ordered = [...list].sort((a, b) => a.nodeOrder - b.nodeOrder);
      set((s) => ({ nodes: { ...s.nodes, [taskId]: ordered } }));
      return ordered;
    } catch (err) {
      app.toastError(err, '加载状态节点失败');
      return get().nodes[taskId] ?? [];
    }
  },

  loadMatrix: async (taskId) => {
    const app = useAppStore.getState();
    set({ loading: true });
    try {
      const matrix = await taskMatrixQuery(taskId);
      const recordMap: Record<string, TaskRecord> = {};
      matrix.records.forEach((r) => {
        recordMap[r.studentId] = r;
      });
      const orderedNodes = [...matrix.nodes].sort((a, b) => a.nodeOrder - b.nodeOrder);
      set((s) => ({
        nodes: { ...s.nodes, [taskId]: orderedNodes },
        records: { ...s.records, [taskId]: recordMap },
        tasks: s.tasks.some((t) => t.id === taskId)
          ? s.tasks.map((t) => (t.id === taskId ? matrix.task : t))
          : [...s.tasks, matrix.task],
      }));
    } catch (err) {
      app.toastError(err, '加载任务矩阵失败');
    } finally {
      set({ loading: false });
    }
  },

  setCurrentTask: (taskId) => set({ currentTaskId: taskId }),
  setViewMode: (mode) => set({ viewMode: mode }),

  upsertTask: async (task) => {
    const app = useAppStore.getState();
    try {
      const saved = await taskUpsert(task);
      set((s) => {
        const exists = s.tasks.some((t) => t.id === saved.id);
        return {
          tasks: exists ? s.tasks.map((t) => (t.id === saved.id ? saved : t)) : [...s.tasks, saved],
          currentTaskId: saved.id,
        };
      });
      app.pushToast({ kind: 'success', title: '任务已保存' });
      return saved;
    } catch (err) {
      app.toastError(err, '保存任务失败');
      throw err;
    }
  },

  removeTask: async (taskId) => {
    const app = useAppStore.getState();
    const snapshot = get().tasks;
    set((s) => ({ tasks: s.tasks.filter((t) => t.id !== taskId) }));
    try {
      await taskDelete(taskId);
      app.pushToast({ kind: 'success', title: '任务已删除' });
    } catch (err) {
      set({ tasks: snapshot });
      app.toastError(err, '删除任务失败');
      throw err;
    }
  },

  saveNodes: async (taskId, nodes) => {
    const app = useAppStore.getState();
    if (nodes.length < TASK_NODE_MIN) {
      app.pushToast({
        kind: 'warning',
        title: `至少需要 ${TASK_NODE_MIN} 个状态节点`,
      });
      throw new Error('TASK_NODE_MIN');
    }
    if (nodes.length > TASK_NODE_MAX) {
      app.pushToast({
        kind: 'warning',
        title: `最多只能有 ${TASK_NODE_MAX} 个状态节点`,
        description: '请删除多余节点后再保存',
      });
      throw new Error('TASK_NODE_MAX');
    }
    const ordered = nodes.map((n, i) => ({ ...n, nodeOrder: i, taskId }));
    const saved: TaskStatusNode[] = [];
    try {
      for (const node of ordered) {
        saved.push(await taskNodeUpsert(node));
      }
      set((s) => ({ nodes: { ...s.nodes, [taskId]: saved } }));
      app.pushToast({ kind: 'success', title: '状态节点已保存' });
      return saved;
    } catch (err) {
      app.toastError(err, '保存状态节点失败');
      throw err;
    }
  },

  deleteNode: async (taskId, nodeId) => {
    const app = useAppStore.getState();
    const prev = get().nodes[taskId] ?? [];
    if (prev.length <= TASK_NODE_MIN) {
      app.pushToast({
        kind: 'warning',
        title: `至少需要保留 ${TASK_NODE_MIN} 个状态节点`,
      });
      return;
    }
    set((s) => ({
      nodes: { ...s.nodes, [taskId]: prev.filter((n) => n.id !== nodeId) },
    }));
    try {
      await taskNodeDelete(nodeId);
      app.pushToast({ kind: 'success', title: '节点已删除' });
    } catch (err) {
      set((s) => ({ nodes: { ...s.nodes, [taskId]: prev } }));
      app.toastError(err, '删除节点失败');
      throw err;
    }
  },

  cycleCell: async (taskId, studentId) => {
    const nodes = get().nodes[taskId] ?? [];
    if (nodes.length === 0) return;
    const currentKey = get().effectiveNodeKey(taskId, studentId);
    const idx = nodes.findIndex((n) => n.nodeKey === currentKey);
    const nextIdx = (idx < 0 ? 0 : idx + 1) % nodes.length;
    await get().setCellNode(taskId, studentId, nodes[nextIdx].nodeKey);
  },

  setCellNode: async (taskId, studentId, nodeKey) => {
    const app = useAppStore.getState();
    const nodes = get().nodes[taskId] ?? [];
    const node = nodes.find((n) => n.nodeKey === nodeKey);
    if (!node) return;
    const prev = get().records[taskId]?.[studentId] ?? null;
    if (prev && prev.nodeKey === nodeKey) return;

    const now = Date.now();
    const optimistic: TaskRecord = {
      id: prev?.id ?? `tmp-${taskId}-${studentId}`,
      taskId,
      studentId,
      nodeId: node.id,
      nodeKey,
      score: prev?.score ?? null,
      note: prev?.note ?? null,
      completedAt: node.isFinal ? now : (prev?.completedAt ?? null),
      evaluatedBy: prev?.evaluatedBy ?? app.settings.deviceName ?? null,
      createdAt: prev?.createdAt ?? now,
      updatedAt: now,
      deletedAt: null,
      syncState: 'pending',
      dirty: true,
    };

    set((s) => ({
      records: {
        ...s.records,
        [taskId]: { ...(s.records[taskId] ?? {}), [studentId]: optimistic },
      },
      saving: { ...s.saving, [`${taskId}:${studentId}`]: true },
    }));

    try {
      const saved = await taskRecordUpsert({
        id: prev?.id && !prev.id.startsWith('tmp-') ? prev.id : undefined,
        taskId,
        studentId,
        nodeId: node.id,
        nodeKey,
        score: prev?.score ?? null,
        note: prev?.note ?? null,
        completedAt: optimistic.completedAt,
      });
      set((s) => ({
        records: {
          ...s.records,
          [taskId]: { ...(s.records[taskId] ?? {}), [studentId]: saved },
        },
        saving: { ...s.saving, [`${taskId}:${studentId}`]: false },
      }));
    } catch (err) {
      // 回滚
      set((s) => {
        const taskRecords = { ...(s.records[taskId] ?? {}) };
        if (prev) taskRecords[studentId] = prev;
        else delete taskRecords[studentId];
        const nextSaving = { ...s.saving };
        delete nextSaving[`${taskId}:${studentId}`];
        return { records: { ...s.records, [taskId]: taskRecords }, saving: nextSaving };
      });
      app.toastError(err, '更新任务状态失败，已恢复');
    }
  },

  setCellScore: async (taskId, studentId, score) => {
    const app = useAppStore.getState();
    const prev = get().records[taskId]?.[studentId] ?? null;
    const currentKey = get().effectiveNodeKey(taskId, studentId);
    const nodes = get().nodes[taskId] ?? [];
    const node = nodes.find((n) => n.nodeKey === currentKey);
    const now = Date.now();
    const optimistic: TaskRecord = {
      id: prev?.id ?? `tmp-${taskId}-${studentId}`,
      taskId,
      studentId,
      nodeId: prev?.nodeId ?? node?.id ?? null,
      nodeKey: currentKey,
      score,
      note: prev?.note ?? null,
      completedAt: prev?.completedAt ?? null,
      evaluatedBy: prev?.evaluatedBy ?? app.settings.deviceName ?? null,
      createdAt: prev?.createdAt ?? now,
      updatedAt: now,
      deletedAt: null,
      syncState: 'pending',
      dirty: true,
    };
    set((s) => ({
      records: {
        ...s.records,
        [taskId]: { ...(s.records[taskId] ?? {}), [studentId]: optimistic },
      },
    }));
    try {
      const saved = await taskRecordUpsert({
        id: prev?.id && !prev.id.startsWith('tmp-') ? prev.id : undefined,
        taskId,
        studentId,
        nodeId: optimistic.nodeId ?? undefined,
        nodeKey: currentKey,
        score,
        note: prev?.note ?? null,
        completedAt: prev?.completedAt ?? null,
      });
      set((s) => ({
        records: {
          ...s.records,
          [taskId]: { ...(s.records[taskId] ?? {}), [studentId]: saved },
        },
      }));
    } catch (err) {
      set((s) => {
        const taskRecords = { ...(s.records[taskId] ?? {}) };
        if (prev) taskRecords[studentId] = prev;
        else delete taskRecords[studentId];
        return { records: { ...s.records, [taskId]: taskRecords } };
      });
      app.toastError(err, '保存评分失败，已恢复');
    }
  },

  setCellNote: async (taskId, studentId, note) => {
    const app = useAppStore.getState();
    const prev = get().records[taskId]?.[studentId] ?? null;
    const currentKey = get().effectiveNodeKey(taskId, studentId);
    const nodes = get().nodes[taskId] ?? [];
    const node = nodes.find((n) => n.nodeKey === currentKey);
    const now = Date.now();
    const optimistic: TaskRecord = {
      id: prev?.id ?? `tmp-${taskId}-${studentId}`,
      taskId,
      studentId,
      nodeId: prev?.nodeId ?? node?.id ?? null,
      nodeKey: currentKey,
      score: prev?.score ?? null,
      note,
      completedAt: prev?.completedAt ?? null,
      evaluatedBy: prev?.evaluatedBy ?? app.settings.deviceName ?? null,
      createdAt: prev?.createdAt ?? now,
      updatedAt: now,
      deletedAt: null,
      syncState: 'pending',
      dirty: true,
    };
    set((s) => ({
      records: {
        ...s.records,
        [taskId]: { ...(s.records[taskId] ?? {}), [studentId]: optimistic },
      },
    }));
    try {
      const saved = await taskRecordUpsert({
        id: prev?.id && !prev.id.startsWith('tmp-') ? prev.id : undefined,
        taskId,
        studentId,
        nodeId: optimistic.nodeId ?? undefined,
        nodeKey: currentKey,
        score: prev?.score ?? null,
        note,
        completedAt: prev?.completedAt ?? null,
      });
      set((s) => ({
        records: {
          ...s.records,
          [taskId]: { ...(s.records[taskId] ?? {}), [studentId]: saved },
        },
      }));
    } catch (err) {
      set((s) => {
        const taskRecords = { ...(s.records[taskId] ?? {}) };
        if (prev) taskRecords[studentId] = prev;
        else delete taskRecords[studentId];
        return { records: { ...s.records, [taskId]: taskRecords } };
      });
      app.toastError(err, '保存备注失败，已恢复');
    }
  },

  saveRecordPatch: async (taskId, studentId, nodeKey, score, note) => {
    const app = useAppStore.getState();
    const nodes = get().nodes[taskId] ?? [];
    const node = nodes.find((n) => n.nodeKey === nodeKey);
    if (!node) return;
    const prev = get().records[taskId]?.[studentId] ?? null;
    const now = Date.now();
    const optimistic: TaskRecord = {
      id: prev?.id ?? `tmp-${taskId}-${studentId}`,
      taskId,
      studentId,
      nodeId: node.id,
      nodeKey,
      score: score === undefined ? (prev?.score ?? null) : score,
      note: note === undefined ? (prev?.note ?? null) : note,
      completedAt: node.isFinal ? (prev?.completedAt ?? now) : null,
      evaluatedBy: prev?.evaluatedBy ?? app.settings.deviceName ?? null,
      createdAt: prev?.createdAt ?? now,
      updatedAt: now,
      deletedAt: null,
      syncState: 'pending',
      dirty: true,
    };
    set((s) => ({
      records: { ...s.records, [taskId]: { ...(s.records[taskId] ?? {}), [studentId]: optimistic } },
      saving: { ...s.saving, [`${taskId}:${studentId}`]: true },
    }));
    try {
      const saved = await taskRecordUpsert({
        id: prev?.id && !prev.id.startsWith('tmp-') ? prev.id : undefined,
        taskId,
        studentId,
        nodeId: node.id,
        nodeKey,
        score: optimistic.score,
        note: optimistic.note,
        completedAt: optimistic.completedAt,
      });
      set((s) => ({
        records: { ...s.records, [taskId]: { ...(s.records[taskId] ?? {}), [studentId]: saved } },
        saving: { ...s.saving, [`${taskId}:${studentId}`]: false },
      }));
    } catch (err) {
      set((s) => {
        const taskRecords = { ...(s.records[taskId] ?? {}) };
        if (prev) taskRecords[studentId] = prev;
        else delete taskRecords[studentId];
        const saving = { ...s.saving };
        delete saving[`${taskId}:${studentId}`];
        return { records: { ...s.records, [taskId]: taskRecords }, saving };
      });
      app.toastError(err, '保存任务记录失败');
      throw err;
    }
  },

  batchSetNode: async (taskId, studentIds, nodeKey) => {
    const app = useAppStore.getState();
    const node = (get().nodes[taskId] ?? []).find((item) => item.nodeKey === nodeKey);
    if (!node || studentIds.length === 0) return;
    const now = Date.now();
    const previous = get().records[taskId] ?? {};
    const records = studentIds.map((studentId) => {
      const prev = previous[studentId];
      return {
        id: prev?.id?.startsWith('tmp-') ? '' : (prev?.id ?? ''),
        taskId,
        studentId,
        nodeId: node.id,
        nodeKey,
        score: prev?.score ?? null,
        note: prev?.note ?? null,
        completedAt: node.isFinal ? (prev?.completedAt ?? now) : null,
        evaluatedBy: prev?.evaluatedBy ?? app.settings.deviceName ?? null,
        createdAt: prev?.createdAt ?? now,
        updatedAt: now,
        deletedAt: null,
        syncState: 'pending' as const,
        dirty: true,
      } satisfies TaskRecord;
    });
    set((s) => {
      const next = { ...(s.records[taskId] ?? {}) };
      records.forEach((record) => { next[record.studentId] = record; });
      return { records: { ...s.records, [taskId]: next } };
    });
    try {
      const saved = await taskRecordsBatchUpsert(records);
      set((s) => {
        const next = { ...(s.records[taskId] ?? {}) };
        saved.forEach((record) => { next[record.studentId] = record; });
        return { records: { ...s.records, [taskId]: next } };
      });
      app.pushToast({ kind: 'success', title: `已将 ${saved.length} 人标记为「${node.label}」` });
    } catch (err) {
      set((s) => ({ records: { ...s.records, [taskId]: previous } }));
      app.toastError(err, '批量更新任务状态失败，已恢复原状态');
      throw err;
    }
  },

  effectiveNodeKey: (taskId, studentId) => {
    const rec = get().records[taskId]?.[studentId];
    if (rec) return rec.nodeKey;
    const task = get().tasks.find((t) => t.id === taskId);
    const nodes = get().nodes[taskId] ?? [];
    if (task?.defaultNodeId) {
      const dn = nodes.find((n) => n.id === task.defaultNodeId);
      if (dn) return dn.nodeKey;
    }
    const defaultNode = nodes.find((n) => n.isDefault);
    if (defaultNode) return defaultNode.nodeKey;
    return nodes[0]?.nodeKey ?? 'todo';
  },

  nodeDistribution: (taskId, students) => {
    const nodes = get().nodes[taskId] ?? [];
    const dist: Record<string, number> = {};
    nodes.forEach((n) => {
      dist[n.nodeKey] = 0;
    });
    students.forEach((s) => {
      const key = get().effectiveNodeKey(taskId, s.id);
      dist[key] = (dist[key] ?? 0) + 1;
    });
    return dist;
  },

  loadCompletionStats: async () => {
    try {
      const rows = await taskCompletionStats();
      set({ completionStats: rows });
    } catch {
      // 教务处端统计可选能力，失败时静默
      set({ completionStats: [] });
    }
  },

  loadProgress: async (taskId, grade, className) => {
    const app = useAppStore.getState();
    try {
      const rows = await taskProgressList(taskId, grade, className);
      set({ progress: rows });
      return rows;
    } catch (err) {
      app.toastError(err, '加载任务进度失败');
      set({ progress: [] });
      return [];
    }
  },

  loadClassMatrix: async (taskId, className) => {
    const app = useAppStore.getState();
    set({ loading: true });
    try {
      const matrix = await taskClassMatrixQuery(taskId, className);
      const recordMap: Record<string, TaskRecord> = {};
      matrix.records.forEach((r) => {
        recordMap[r.studentId] = r;
      });
      const orderedNodes = [...matrix.nodes].sort((a, b) => a.nodeOrder - b.nodeOrder);
      set((s) => ({
        nodes: { ...s.nodes, [taskId]: orderedNodes },
        records: { ...s.records, [taskId]: recordMap },
        tasks: s.tasks.some((t) => t.id === taskId)
          ? s.tasks.map((t) => (t.id === taskId ? matrix.task : t))
          : [...s.tasks, matrix.task],
      }));
    } catch (err) {
      app.toastError(err, '加载班级任务明细失败');
    } finally {
      set({ loading: false });
    }
  },
}));

/** 便捷：确保任务节点已加载 */
export async function ensureNodesLoaded(taskId: string): Promise<TaskStatusNode[]> {
  const store = useTaskStore.getState();
  const existing = store.nodes[taskId];
  if (existing && existing.length > 0) return existing;
  return store.loadNodes(taskId);
}

/** 便捷：确保矩阵已加载（含节点与记录） */
export async function ensureMatrixLoaded(taskId: string): Promise<void> {
  const store = useTaskStore.getState();
  const hasRecords = Boolean(store.records[taskId]);
  const hasNodes = (store.nodes[taskId] ?? []).length > 0;
  if (hasRecords && hasNodes) return;
  await store.loadMatrix(taskId);
}

/** 学生名册（任务矩阵使用，排除已转出） */
export function matrixStudents(): { id: string; name: string; studentNo: string }[] {
  return useStudentStore.getState().checkinStudents();
}
