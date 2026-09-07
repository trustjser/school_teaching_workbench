import { useEffect, useMemo } from 'react';
import { useStudentStore } from '@/store/useStudentStore';
import { useTaskStore } from '@/store/useTaskStore';
import { Tabs } from '@/components/ui/Tabs';
import { Badge } from '@/components/ui/Badge';
import { EmptyState } from '@/components/ui/EmptyState';
import { Select } from '@/components/ui/Select';
import { COLOR_TOKEN_HEX } from '@/constants/status';
import { resolveIcon } from '@/components/ui/IconButton';

/**
 * 任务矩阵视图（学生 × 状态节点，2~4 节点）。
 * 提供「网格视图 / 表格视图」双视图：
 *   - 网格：每个学生一张卡片，点击循环到下一节点；
 *   - 表格：首列为学生，每个节点一列，点击单元格直接设置节点。
 * 数据来自 useTaskStore（nodes / records / viewMode）。
 */
export function TaskMatrixView(): JSX.Element {
  const tasks = useTaskStore((s) => s.tasks);
  const currentTaskId = useTaskStore((s) => s.currentTaskId);
  const viewMode = useTaskStore((s) => s.viewMode);
  const nodes = useTaskStore((s) => (currentTaskId ? s.nodes[currentTaskId] ?? [] : []));
  const records = useTaskStore((s) => (currentTaskId ? s.records[currentTaskId] ?? {} : {}));
  const loadTasks = useTaskStore((s) => s.loadTasks);
  const loadMatrix = useTaskStore((s) => s.loadMatrix);
  const setCurrentTask = useTaskStore((s) => s.setCurrentTask);
  const setViewMode = useTaskStore((s) => s.setViewMode);
  const cycleCell = useTaskStore((s) => s.cycleCell);
  const setCellNode = useTaskStore((s) => s.setCellNode);
  const effectiveNodeKey = useTaskStore((s) => s.effectiveNodeKey);
  const nodeDistribution = useTaskStore((s) => s.nodeDistribution);

  const students = useStudentStore((s) => s.students);
  const roster = useMemo(
    () => students.filter((s) => s.status !== 'transferred'),
    [students],
  );

  useEffect(() => {
    void loadTasks();
  }, [loadTasks]);

  useEffect(() => {
    if (currentTaskId && nodes.length === 0) {
      void loadMatrix(currentTaskId);
    }
  }, [currentTaskId, nodes.length, loadMatrix]);

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
        <Select
          label="选择任务"
          placeholder="请选择任务"
          options={taskOptions}
          value={currentTaskId ?? ''}
          onChange={(e) => setCurrentTask(e.target.value || null)}
        />
        <Tabs
          items={[
            { value: 'grid', label: '网格视图' },
            { value: 'table', label: '表格视图' },
          ]}
          value={viewMode}
          onChange={(v) => setViewMode(v as 'grid' | 'table')}
        />
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
                disabled={!currentTaskId}
                onClick={() => currentTaskId && void cycleCell(currentTaskId, student.id)}
                className="flex min-h-touch flex-col items-center justify-center gap-1 rounded-lg border-2 bg-white p-3 text-center transition-colors hover:brightness-95 disabled:opacity-60"
                style={{ borderColor: color }}
                title="点击切换到下一状态节点"
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
            <thead className="bg-slate-100">
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
                  <tr key={student.id} className="border-t border-slate-200">
                    <td className="px-4 py-2 font-semibold text-ink">{student.name}</td>
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
                              active ? 'text-white' : 'text-ink-muted hover:bg-slate-100',
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
    </div>
  );
}
