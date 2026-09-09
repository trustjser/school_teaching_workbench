import { useEffect } from 'react';
import { ArrowLeft, Clock3, Users } from 'lucide-react';
import { useNavigate, useParams, useSearchParams } from 'react-router-dom';
import { useStudentStore } from '@/store/useStudentStore';
import { useTaskStore } from '@/store/useTaskStore';
import { Button } from '@/components/ui/Button';
import { Card } from '@/components/ui/Card';
import { Badge } from '@/components/ui/Badge';
import { ProgressBar } from '@/components/ui/ProgressBar';
import { EmptyState } from '@/components/ui/EmptyState';
import { TaskMatrixView } from '@/components/task/TaskMatrixView';
import { formatDateTime, formatPercent } from '@/lib/format';

/** 教务端单班级任务详情：学生状态矩阵 + 评分/备注编辑。 */
export function TaskDetail(): JSX.Element {
  const navigate = useNavigate();
  const { taskId = '' } = useParams();
  const [params] = useSearchParams();
  const className = params.get('class') ?? '';
  const returnTo = params.get('return') || '/master/tasks';
  const tasks = useTaskStore((s) => s.tasks);
  const progress = useTaskStore((s) => s.progress);
  const loadTasks = useTaskStore((s) => s.loadTasks);
  const loadClassMatrix = useTaskStore((s) => s.loadClassMatrix);
  const loadProgress = useTaskStore((s) => s.loadProgress);
  const setCurrentTask = useTaskStore((s) => s.setCurrentTask);
  const loadStudents = useStudentStore((s) => s.load);
  const task = tasks.find((item) => item.id === taskId);
  const summary = progress.find((item) => item.taskId === taskId && item.className === className);

  useEffect(() => {
    void loadStudents();
    void loadTasks();
    setCurrentTask(taskId || null);
  }, [loadStudents, loadTasks, setCurrentTask, taskId]);

  useEffect(() => {
    if (!taskId || !className) return;
    void loadProgress(taskId, null, className);
    void loadClassMatrix(taskId, className);
  }, [className, loadClassMatrix, loadProgress, taskId]);

  if (!task || !className) {
    return <EmptyState title="找不到任务班级" description="请从任务看板选择一个班级进入详情。" action={<Button onClick={() => navigate(returnTo)}>返回任务看板</Button>} />;
  }

  return (
    <div className="space-y-5">
      <div className="flex flex-wrap items-center gap-3">
        <Button variant="ghost" icon={<ArrowLeft className="h-5 w-5" />} onClick={() => navigate(returnTo)}>返回看板</Button>
        <div>
          <h1 className="text-3xl font-bold text-ink">{task.title} · {className}</h1>
          <p className="mt-1 text-base text-ink-muted">学生级别处理明细，可直接编辑状态、评分和备注。</p>
        </div>
      </div>

      <div className="grid gap-4 board:grid-cols-[1.4fr_1fr]">
        <Card title="班级处理摘要">
          <div className="grid grid-cols-2 gap-3 board:grid-cols-4">
            <div><p className="text-sm text-ink-muted">班级人数</p><p className="mt-1 text-2xl font-bold text-ink">{summary?.total ?? 0}</p></div>
            <div><p className="text-sm text-ink-muted">已完成</p><p className="mt-1 text-2xl font-bold text-green-700">{summary?.finalCount ?? 0}</p></div>
            <div><p className="text-sm text-ink-muted">处理中</p><p className="mt-1 text-2xl font-bold text-brand-700">{summary?.processingCount ?? 0}</p></div>
            <div><p className="text-sm text-ink-muted">平均分</p><p className="mt-1 text-2xl font-bold text-violet-700">{summary?.avgScore == null ? '—' : summary.avgScore.toFixed(1)}</p></div>
          </div>
          <div className="mt-4"><ProgressBar value={(summary?.completionRate ?? 0) * 100} label={`${formatPercent((summary?.completionRate ?? 0) * 100)} 完成`} tone="success" /></div>
        </Card>
        <Card title="任务配置">
          <div className="flex flex-wrap gap-2">
            <Badge tone="neutral"><Users className="mr-1 h-4 w-4" />{className}</Badge>
            {task.dueAt && <Badge tone="neutral"><Clock3 className="mr-1 h-4 w-4" />截止 {formatDateTime(task.dueAt)}</Badge>}
            <Badge tone={task.scoreEnabled ? 'brand' : 'neutral'}>{task.scoreEnabled ? '已开启评分' : '未开启评分'}</Badge>
            <Badge tone={task.noteEnabled ? 'brand' : 'neutral'}>{task.noteEnabled ? '已开启备注' : '未开启备注'}</Badge>
          </div>
        </Card>
      </div>

      <Card title="学生处理明细" description="点击学生卡片可编辑状态、评分和备注；表格视图可快速切换状态。">
        <TaskMatrixView className={className} hideTaskSelect />
      </Card>
    </div>
  );
}
