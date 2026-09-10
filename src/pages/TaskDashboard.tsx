import { useEffect, useMemo, useState } from 'react';
import { ArrowRight, ClipboardCheck, RefreshCw } from 'lucide-react';
import { useNavigate, useSearchParams } from 'react-router-dom';
import { useTaskStore } from '@shared/store/useTaskStore';
import { Card } from '@shared/components/ui/Card';
import { SearchableSelect } from '@shared/components/ui/SearchableSelect';
import { Table, type TableColumn } from '@shared/components/ui/Table';
import { Badge } from '@shared/components/ui/Badge';
import { Button } from '@shared/components/ui/Button';
import { ProgressBar } from '@shared/components/ui/ProgressBar';
import { EmptyState } from '@shared/components/ui/EmptyState';
import { formatDateTime, formatPercent } from '@shared/lib/format';
import type { TaskProgressRow } from '@shared/types/api';

/** 教务端任务全局处理看板：任务总览 → 班级进度 → 学生明细。 */
export function TaskDashboard(): JSX.Element {
  const navigate = useNavigate();
  const [searchParams, setSearchParams] = useSearchParams();
  const tasks = useTaskStore((s) => s.tasks);
  const completion = useTaskStore((s) => s.completionStats);
  const progress = useTaskStore((s) => s.progress);
  const loadTasks = useTaskStore((s) => s.loadTasks);
  const loadCompletion = useTaskStore((s) => s.loadCompletionStats);
  const loadProgress = useTaskStore((s) => s.loadProgress);
  const [selectedTaskId, setSelectedTaskId] = useState(() => searchParams.get('task') ?? '');
  const [grade, setGrade] = useState(() => searchParams.get('grade') ?? '');
  const [className, setClassName] = useState(() => searchParams.get('class') ?? '');

  useEffect(() => {
    const next = new URLSearchParams();
    if (selectedTaskId) next.set('task', selectedTaskId);
    if (grade) next.set('grade', grade);
    if (className) next.set('class', className);
    setSearchParams(next, { replace: true });
  }, [className, grade, selectedTaskId, setSearchParams]);

  const refresh = async (): Promise<void> => {
    await Promise.all([loadTasks(), loadCompletion()]);
    const availableTasks = useTaskStore.getState().tasks;
    const taskId = availableTasks.some((task) => task.id === selectedTaskId)
      ? selectedTaskId
      : availableTasks[0]?.id ?? '';
    if (taskId) {
      if (taskId !== selectedTaskId) setSelectedTaskId(taskId);
      await loadProgress(taskId, grade || null, className || null);
    }
  };

  useEffect(() => {
    void refresh();
    // 首次加载只需执行一次，筛选变化由下方 effect 触发。
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    if (!selectedTaskId) return;
    void loadProgress(selectedTaskId, grade || null, className || null);
  }, [selectedTaskId, grade, className, loadProgress]);

  const selectedSummary = completion.find((row) => row.taskId === selectedTaskId);
  const gradeOptions = useMemo(
    () => [
      { value: '', label: '全部年级' },
      ...Array.from(new Set(progress.map((row) => row.grade).filter(Boolean) as string[])).map((value) => ({ value, label: value })),
    ],
    [progress],
  );
  const classOptions = useMemo(
    () => [
      { value: '', label: '全部班级' },
      ...Array.from(new Set(progress.map((row) => row.className))).map((value) => ({ value, label: value })),
    ],
    [progress],
  );

  const taskOptions = tasks.map((task) => ({ value: task.id, label: task.title }));
  const columns: TableColumn<TaskProgressRow>[] = [
    {
      key: 'class',
      header: '班级',
      render: (row) => (
        <div>
          <p className="font-semibold text-ink">{row.className}</p>
          <p className="text-sm text-ink-muted">{row.grade ?? '未分年级'}</p>
        </div>
      ),
    },
    {
      key: 'device',
      header: '节点',
      render: (row) => (
        <div className="flex items-center gap-2">
          <span className={['h-2.5 w-2.5 rounded-full', row.deviceStatus === 'online' ? 'bg-green-500' : 'bg-surface-border'].join(' ')} />
          <span>{row.deviceName ?? '未绑定设备'}</span>
        </div>
      ),
    },
    {
      key: 'progress',
      header: '处理进度',
      widthClass: 'min-w-[220px]',
      render: (row) => (
        <ProgressBar
          value={row.completionRate * 100}
          label={`${row.finalCount}/${row.total} 已完成 · ${row.pendingCount} 待处理`}
          tone={row.completionRate >= 1 ? 'success' : 'brand'}
          heightClass="h-3"
        />
      ),
    },
    { key: 'processing', header: '处理中', accessor: (row) => row.processingCount, align: 'center' },
    { key: 'score', header: '平均分', accessor: (row) => (row.avgScore == null ? '—' : row.avgScore.toFixed(1)), align: 'center' },
    { key: 'updated', header: '最近更新', accessor: (row) => formatDateTime(row.lastUpdatedAt), align: 'center' },
    {
      key: 'action',
      header: '操作',
      align: 'center',
      render: (row) => <ArrowRight className="mx-auto h-5 w-5 text-brand-600" aria-label="查看班级详情" />,
    },
  ];

  if (tasks.length === 0 && completion.length === 0) {
    return <EmptyState title="还没有任务" description="先在任务下发中创建并发送任务，教务端会在这里显示全局处理情况。" />;
  }

  return (
    <div className="space-y-5">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <div>
          <h1 className="text-3xl font-bold text-ink">任务处理看板</h1>
          <p className="mt-1 text-base text-ink-muted">实时查看各班级任务处理进度，点击班级进入学生明细。</p>
        </div>
        <Button variant="secondary" icon={<RefreshCw className="h-5 w-5" />} onClick={() => void refresh()}>刷新</Button>
      </div>

      <Card>
        <div className="grid grid-cols-1 gap-3 md:grid-cols-[minmax(240px,1.5fr)_minmax(180px,1fr)_minmax(180px,1fr)]">
          <SearchableSelect label="任务" options={taskOptions} placeholder="选择任务" value={selectedTaskId} onChange={(value) => { setSelectedTaskId(value); setGrade(''); setClassName(''); }} />
          <SearchableSelect label="年级" options={gradeOptions} placeholder="全部年级" value={grade} onChange={(value) => { setGrade(value); setClassName(''); }} />
          <SearchableSelect label="班级" options={classOptions} placeholder="全部班级" value={className} onChange={setClassName} />
        </div>
      </Card>

      {selectedSummary && (
        <div className="grid gap-3 grid-cols-2 board:grid-cols-4">
          <Card><p className="text-sm text-ink-muted">班级人数</p><p className="mt-1 text-3xl font-bold text-ink">{selectedSummary.total}</p></Card>
          <Card><p className="text-sm text-ink-muted">已完成</p><p className="mt-1 text-3xl font-bold text-green-700">{selectedSummary.finalCount}</p></Card>
          <Card><p className="text-sm text-ink-muted">完成率</p><p className="mt-1 text-3xl font-bold text-brand-700">{formatPercent(selectedSummary.completionRate * 100)}</p></Card>
          <Card><p className="text-sm text-ink-muted">平均分</p><p className="mt-1 text-3xl font-bold text-violet-700">{selectedSummary.avgScore == null ? '—' : selectedSummary.avgScore.toFixed(1)}</p></Card>
        </div>
      )}

      <Card title={selectedSummary ? `${selectedSummary.title} · 班级进度` : '班级进度'} description="点击任意班级查看学生级别的状态、评分和备注。">
        <Table
          columns={columns}
          data={progress}
          rowKey={(row) => `${row.taskId}:${row.className}`}
          onRowClick={(row) => {
            const returnParams = new URLSearchParams();
            if (selectedTaskId) returnParams.set('task', selectedTaskId);
            if (grade) returnParams.set('grade', grade);
            if (className) returnParams.set('class', className);
            const returnTo = `/master/tasks${returnParams.toString() ? `?${returnParams.toString()}` : ''}`;
            const detailParams = new URLSearchParams({ class: row.className, return: returnTo });
            navigate(`/master/tasks/${encodeURIComponent(row.taskId)}?${detailParams.toString()}`);
          }}
          empty={<span className="text-ink-muted">暂无班级任务记录</span>}
        />
      </Card>

      {selectedSummary && (
        <Badge tone="neutral"><ClipboardCheck className="mr-1 h-4 w-4" />最后统计任务：{selectedSummary.title}</Badge>
      )}
    </div>
  );
}
