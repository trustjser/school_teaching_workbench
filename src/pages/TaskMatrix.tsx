import { useEffect } from 'react';
import { useStudentStore } from '@/store/useStudentStore';
import { TaskMatrixView } from '@/components/task/TaskMatrixView';
import { Card } from '@/components/ui/Card';

/** 任务矩阵页：学生 × 状态节点双视图 */
export function TaskMatrix(): JSX.Element {
  const load = useStudentStore((s) => s.load);

  useEffect(() => {
    void load();
  }, [load]);

  return (
    <div className="space-y-4">
      <h1 className="text-3xl font-bold text-ink">任务矩阵</h1>
      <Card className="animate-rise-in">
        <TaskMatrixView />
      </Card>
    </div>
  );
}
