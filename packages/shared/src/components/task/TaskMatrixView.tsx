import { useEffect, useMemo, useState } from 'react';
import { useStudentStore } from '@shared/store/useStudentStore';
import { useTaskStore } from '@shared/store/useTaskStore';
import { Tabs } from '@shared/components/ui/Tabs';
import { Badge } from '@shared/components/ui/Badge';
import { EmptyState } from '@shared/components/ui/EmptyState';
import { Select } from '@shared/components/ui/Select';
import { SearchableSelect } from '@shared/components/ui/SearchableSelect';
import { Button } from '@shared/components/ui/Button';
import { COLOR_TOKEN_HEX } from '@shared/constants/status';
import { resolveIcon } from '@shared/components/ui/IconButton';
import { TaskRecordEditor } from './TaskRecordEditor';

export interface TaskMatrixViewProps {
  className?: string;
  hideTaskSelect?: boolean;
}

/**
 * 任务矩阵视图（学生 × 状态节点，2~4 节点）。
 * 提供「网格视图 / 表格视图」双视图：
 *   - 网格：每个学生一张卡片，点击打开记录编辑器；
 *   - 表格：首列学生点击打开编辑器，每个节点单元格可快速设置状态。
 * 数据来自 useTaskStore（nodes / records / viewMode）。
 */
export function TaskMatrixView({ className, hideTaskSelect = false }: TaskMatrixViewProps): JSX.Element {
  const tasks = useTaskStore((s) => s.tasks);
  const currentTaskId = useTaskStore((s) => s.currentTaskId);
  const viewMode = useTaskStore((s) => s.viewMode);
  const nodes = useTaskStore((s) => (currentTaskId ? s.nodes[currentTaskId] ?? [] : []));
  const records = useTaskStore((s) => (currentTaskId ? s.records[currentTaskId] ?? {} : {}));
  const loadTasks = useTaskStore((s) => s.loadTasks);
  const loadMatrix = useTaskStore((s) => s.loadMatrix);
  const loadClassMatrix = useTaskStore((s) => s.loadClassMatrix);
  const setCurrentTask = useTaskStore((s) => s.setCurrentTask);
  const setViewMode = useTaskStore((s) => s.setViewMode);
  const setCellNode = useTaskStore((s) => s.setCellNode);
  const saveRecordPatch = useTaskStore((s) => s.saveRecordPatch);
  const batchSetNode = useTaskStore((s) => s.batchSetNode);
  const effectiveNodeKey = useTaskStore((s) => s.effectiveNodeKey);
  const nodeDistribution = useTaskStore((s) => s.nodeDistribution);

  const students = useStudentStore((s) => s.students);
  const roster = useMemo(
    () => students.filter((s) => s.status !== 'transferred' && (!className || s.className === className)),
    [className, students],
  );
  const [editingStudentId, setEditingStudentId] = useState<string | null>(null);
  const [bulkNodeKey, setBulkNodeKey] = useState('');
  const task = tasks.find((item) => item.id === currentTaskId) ?? null;
  const editingStudent = roster.find((student) => student.id === editingStudentId) ?? null;
  const editingSaving = useTaskStore((s) =>
    task && editingStudent ? Boolean(s.saving[`${task.id}:${editingStudent.id}`]) : false,
  );

  useEffect(() => {
    if (nodes.length > 0 && !nodes.some((node) => node.nodeKey === bulkNodeKey)) {
      setBulkNodeKey(nodes.find((node) => node.isDefault)?.nodeKey ?? nodes[0].nodeKey);
    }
  }, [bulkNodeKey, nodes]);

  useEffect(() => {
    void loadTasks();
  }, [loadTasks]);

  useEffect(() => {
    if (currentTaskId && nodes.length === 0) {
      if (className) void loadClassMatrix(currentTaskId, className);
      else void loadMatrix(currentTaskId);
    }
  }, [className, currentTaskId, loadClassMatrix, loadMatrix, nodes.length]);

  if (tasks.length === 0) {
    return (
      <EmptyState
        title="还没有任务"
        description="在「任务管理」中创建自定义任务后，可在此查看学生 × 状态节点矩阵。"
      />
    );
  }

  const taskOptions = tasks.map((t) => ({ value: t.id, label: t.title }));
  const dist = nodeDistribution(currentTaskId ?? '', roster);

  return (
    <div className="space-y-4">
      <div className="flex flex-wrap items-center gap-3">
        {!hideTaskSelect && (
          <SearchableSelect
            label="选择任务"
            placeholder="请选择任务"
            options={taskOptions}
            value={currentTaskId ?? ''}
            onChange={(value) => setCurrentTask(value || null)}
          />
        )}
        <Tabs
          items={[
            { value: 'grid', label: '网格视图' },
            { value: 'table', label: '表格视图' },
          ]}
          value={viewMode}
          onChange={(v) => setViewMode(v as 'grid' | 'table')}
        />
        {currentTaskId && roster.length > 0 && nodes.length > 0 && (
          <div className="flex flex-1 flex-wrap items-end justify-end gap-2">
            <div className="min-w-[10rem]">
              <Select
                label="全班状态"
                options={nodes.map((node) => ({ value: node.nodeKey, label: node.label }))}
                value={bulkNodeKey}
                onChange={(event) => setBulkNodeKey(event.target.value)}
              />
            </div>
            <Button variant="secondary" onClick={() => void batchSetNode(currentTaskId, roster.map((student) => student.id), bulkNodeKey)}>
              一键标记全班
            </Button>
          </div>
        )}
      </div>

      <div className="flex flex-wrap gap-2">
        {nodes.map((n) => (
          <Badge key={n.id} tone="neutral">
            <span
              className="inline-block h-3 w-3 rounded-full"
              style={{ backgroundColor: COLOR_TOKEN_HEX[n.colorToken] ?? '#475569' }}
            />
            {n.label} {dist[n.nodeKey] ?? 0}
          </Badge>
        ))}
      </div>

      {viewMode === 'grid' ? (
        <div className="grid grid-cols-2 gap-3 board:grid-cols-4 wall:grid-cols-6">
          {roster.map((student) => {
            const key = effectiveNodeKey(currentTaskId ?? '', student.id);
            const node = nodes.find((n) => n.nodeKey === key);
            const color = node ? COLOR_TOKEN_HEX[node.colorToken] ?? '#475569' : '#475569';
            const Icon = node ? resolveIcon(node.iconName) : null;
            return (
              <button
                key={student.id}
                type="button"
                disabled={!currentTaskId || nodes.length === 0}
                onClick={() => currentTaskId && setEditingStudentId(student.id)}
                className="flex min-h-touch flex-col items-center justify-center gap-1 rounded-lg border-2 bg-surface-raised p-3 text-center transition-colors hover:brightness-95 disabled:opacity-60"
                style={{ borderColor: color }}
                title="点击编辑状态、评分和备注"
              >
                {Icon && <Icon className="h-6 w-6" style={{ color }} />}
                <span className="truncate text-base font-semibold text-ink">{student.name}</span>
                <span className="text-sm" style={{ color }}>
                  {node?.label ?? '未开始'}
                </span>
              </button>
            );
          })}
        </div>
      ) : (
        <div className="overflow-x-auto rounded-lg border border-surface-border">
          <table className="w-full border-collapse text-base">
            <thead className="bg-surface-muted">
              <tr>
                <th className="px-4 py-2 text-left text-ink">学生</th>
                {nodes.map((n) => (
                  <th
                    key={n.id}
                    className="px-4 py-2 text-center text-ink"
                    style={{ color: COLOR_TOKEN_HEX[n.colorToken] ?? '#475569' }}
                  >
                    {n.label}
                  </th>
                ))}
              </tr>
            </thead>
            <tbody>
              {roster.map((student) => {
                const key = effectiveNodeKey(currentTaskId ?? '', student.id);
                return (
                  <tr key={student.id} className="border-t border-surface-border">
                    <td className="px-4 py-2 font-semibold text-ink">
                      <button
                        type="button"
                        disabled={!currentTaskId || nodes.length === 0}
                        className="min-h-touch text-left hover:text-brand-700 disabled:cursor-not-allowed disabled:opacity-60"
                        onClick={() => setEditingStudentId(student.id)}
                      >
                        {student.name}
                      </button>
                    </td>
                    {nodes.map((n) => {
                      const active = n.nodeKey === key;
                      return (
                        <td key={n.id} className="px-2 py-1 text-center">
                          <button
                            type="button"
                            disabled={!currentTaskId}
                            onClick={() => currentTaskId && void setCellNode(currentTaskId, student.id, n.nodeKey)}
                            className={[
                              'mx-auto inline-flex h-10 min-w-touch items-center justify-center rounded-md px-3 text-sm font-semibold',
                              active ? 'text-white' : 'text-ink-muted hover:bg-surface-muted',
                            ].join(' ')}
                            style={active ? { backgroundColor: COLOR_TOKEN_HEX[n.colorToken] ?? '#475569' } : undefined}
                            title={n.label}
                          >
                            {active ? '●' : '○'}
                          </button>
                        </td>
                      );
                    })}
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
      )}

      {task && editingStudent && (
        <TaskRecordEditor
          open={editingStudentId !== null}
          task={task}
          student={editingStudent}
          record={records[editingStudent.id] ?? null}
          nodes={nodes}
          saving={editingSaving}
          onClose={() => setEditingStudentId(null)}
          onSave={async (patch) => {
            await saveRecordPatch(task.id, editingStudent.id, patch.nodeKey, patch.score, patch.note);
            setEditingStudentId(null);
          }}
        />
      )}
    </div>
  );
}
