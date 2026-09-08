import { useEffect } from 'react';
import { useStudentStore } from '@/store/useStudentStore';
import { AttendanceGrid } from '@/components/checkin/AttendanceGrid';
import { Card } from '@/components/ui/Card';

/** 快捷考勤页：全屏反向标记考勤网格 */
export function CheckinPage(): JSX.Element {
  const load = useStudentStore((s) => s.load);

  useEffect(() => {
    void load();
  }, [load]);

  return (
    <div className="space-y-4">
      <h1 className="text-3xl font-bold text-ink">快捷考勤</h1>
      <Card className="animate-rise-in">
        <AttendanceGrid />
      </Card>
    </div>
  );
}
