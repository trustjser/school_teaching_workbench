import { useEffect } from 'react';
import { useSearchParams } from 'react-router-dom';
import { useStudentStore } from '@shared/store/useStudentStore';
import { useTaskStore } from '@shared/store/useTaskStore';
import { TaskMatrixView } from '@shared/components/task/TaskMatrixView';
import { Card } from '@shared/components/ui/Card';

/** 任务矩阵页：学生 × 状态节点双视图 */
export function TaskMatrix(): JSX.Element {
  const load = useStudentStore((s) => s.load);
  const tasks = useTaskStore((s) => s.tasks);
  const setCurrentTask = useTaskStore((s) => s.setCurrentTask);
  const [searchParams] = useSearchParams();
  const broadcastId = searchParams.get('broadcast');
  const taskId = searchParams.get('task');

  useEffect(() => {
    void load();
  }, [load]);

  useEffect(() => {
    if (tasks.length === 0) return;
    const task = taskId
      ? tasks.find((item) => item.id === taskId)
      : broadcastId
        ? tasks.find((item) => item.broadcastTaskId === broadcastId)
        : undefined;
    if (task) setCurrentTask(task.id);
  }, [broadcastId, taskId, tasks, setCurrentTask]);

  return (
    <div className="space-y-4">
      <h1 className="text-3xl font-bold text-ink">任务矩阵</h1>
      <Card className="animate-rise-in">
        <TaskMatrixView />
      </Card>
    </div>
  );
}
