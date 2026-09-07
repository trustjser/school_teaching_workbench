import { useState } from 'react';
import { Link } from 'react-router-dom';
import { UserPlus } from 'lucide-react';
import { useStudentStore } from '@/store/useStudentStore';
import { useCheckinStore } from '@/store/useCheckinStore';
import { useTaskStore } from '@/store/useTaskStore';
import { useBroadcastStore } from '@/store/useBroadcastStore';
import { Card } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';
import { Badge } from '@/components/ui/Badge';
import { AttendanceGrid } from '@/components/checkin/AttendanceGrid';
import { StudentImportDialog } from '@/components/student/StudentImportDialog';

/** 班级首页：名册导入入口 + 反向标记考勤 + 任务矩阵概览 + 教务指令收件箱 */
export function ClientHome(): JSX.Element {
  const students = useStudentStore((s) => s.students);
  const tasks = useTaskStore((s) => s.tasks);
  const inbox = useBroadcastStore((s) => s.inbox);
  const unread = useBroadcastStore((s) => s.unreadCount);
  const records = useCheckinStore((s) => s.records);
  const [importOpen, setImportOpen] = useState(false);

  const rosterCount = students.filter((s) => s.status !== 'transferred').length;
  const present = Object.values(records).filter(
    (r) => r.state === 'present' || r.state === 'late',
  ).length;
  const rate = rosterCount > 0 ? Math.round((present / rosterCount) * 1000) / 10 : 0;

  return (
    <div className="space-y-6">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <h1 className="text-3xl font-bold text-ink">班级首页</h1>
        <Button icon={<UserPlus className="h-5 w-5" />} onClick={() => setImportOpen(true)}>
          导入名册
        </Button>
      </div>

      <div className="grid grid-cols-2 gap-4 board:grid-cols-4">
        <StatCard label="在读学生" value={rosterCount} tone="brand" />
        <StatCard label="今日出勤率" value={`${rate}%`} tone="success" />
        <StatCard label="自定义任务" value={tasks.length} tone="violet" />
        <StatCard label="教务指令" value={inbox.length} tone="warning" badge={unread} />
      </div>

      <Card
        title="快捷考勤（反向标记）"
        description="点击卡片循环：出勤 → 请假 → 缺勤 → 出勤"
        actions={
          <Link to="/client/checkin">
            <Button variant="ghost" size="md">
              全屏考勤
            </Button>
          </Link>
        }
      >
        <AttendanceGrid compact />
      </Card>

      <div className="grid gap-4 board:grid-cols-2">
        <Card
          title="任务矩阵"
          description="学生 × 状态节点，2~4 个节点双视图"
          actions={
            <Link to="/client/matrix">
              <Button variant="ghost" size="md">
                打开矩阵
              </Button>
            </Link>
          }
        >
          <p className="text-ink-soft">
            {tasks.length > 0
              ? `当前 ${tasks.length} 个任务，可进入「任务矩阵」逐人标记状态节点。`
              : '尚未创建自定义任务。'}
          </p>
          <Link to="/client/tasks">
            <Button variant="secondary" size="md" className="mt-3">
              管理任务
            </Button>
          </Link>
        </Card>

        <Card
          title="教务指令"
          description="接收教务处下发的任务与通知"
          actions={unread > 0 ? <Badge tone="warning">🔔 {unread} 条未读</Badge> : undefined}
        >
          <p className="text-ink-soft">
            {inbox.length > 0
              ? `收到 ${inbox.length} 条教务指令，点击「接受」可生成本班待办。`
              : '暂无新的教务指令。'}
          </p>
          <Link to="/client/inbox">
            <Button variant="secondary" size="md" className="mt-3">
              查看收件箱
            </Button>
          </Link>
        </Card>
      </div>

      <StudentImportDialog open={importOpen} onClose={() => setImportOpen(false)} />
    </div>
  );
}

function StatCard({
  label,
  value,
  tone,
  badge,
}: {
  label: string;
  value: string | number;
  tone: 'brand' | 'success' | 'violet' | 'warning';
  badge?: number;
}): JSX.Element {
  return (
    <Card>
      <p className="text-base text-ink-muted">{label}</p>
      <p className="mt-1 text-4xl font-bold text-ink">{value}</p>
      {badge ? (
        <Badge tone={tone} className="mt-2">
          未读 {badge}
        </Badge>
      ) : null}
    </Card>
  );
}
