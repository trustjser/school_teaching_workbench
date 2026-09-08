import { Link } from 'react-router-dom';
import { CalendarCheck, ClipboardList, GraduationCap, Inbox, Users } from 'lucide-react';
import { useStudentStore } from '@/store/useStudentStore';
import { useCheckinStore } from '@/store/useCheckinStore';
import { useTaskStore } from '@/store/useTaskStore';
import { useBroadcastStore } from '@/store/useBroadcastStore';
import { Card } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';
import { Badge } from '@/components/ui/Badge';
import { StatCard } from '@/components/ui/StatCard';
import { Stagger } from '@/components/motion/Reveal';

/** 班级首页：名册导入入口 + 出勤率概览 + 任务矩阵 + 教务指令收件箱 */
export function ClientHome(): JSX.Element {
  const students = useStudentStore((s) => s.students);
  const tasks = useTaskStore((s) => s.tasks);
  const inbox = useBroadcastStore((s) => s.inbox);
  const unread = useBroadcastStore((s) => s.unreadCount);
  const records = useCheckinStore((s) => s.records);

  const rosterCount = students.filter((s) => s.status !== 'transferred').length;
  const present = Object.values(records).filter(
    (r) => r.state === 'present' || r.state === 'late',
  ).length;
  const rate = rosterCount > 0 ? Math.round((present / rosterCount) * 1000) / 10 : 0;

  return (
    <div className="space-y-6">
      {/* 欢迎条 */}
      <section className="card flex flex-wrap items-center justify-between gap-4 p-6">
        <div className="flex items-center gap-4">
          <div className="flex h-14 w-14 items-center justify-center rounded-2xl bg-brand-gradient text-white shadow-glow">
            <GraduationCap className="h-7 w-7" />
          </div>
          <div>
            <p className="text-sm font-medium text-ink-muted">欢迎回来</p>
            <h1 className="text-3xl font-bold text-ink">班级工作台</h1>
            <p className="mt-0.5 text-ink-soft">一键考勤、任务矩阵与教务指令，尽在掌握。</p>
          </div>
        </div>
        <span className="rounded-full border border-surface-border bg-surface-muted px-3 py-1.5 text-sm text-ink-muted">
          学生名册由教务处统一下发，无需导入
        </span>
      </section>

      <Stagger className="grid grid-cols-2 gap-4 board:grid-cols-4" step={70}>
        <StatCard icon={Users} label="在读学生" value={rosterCount} tone="brand" />
        <StatCard icon={CalendarCheck} label="今日出勤率" value={rate} decimals={1} suffix="%" tone="success" accent />
        <StatCard icon={ClipboardList} label="自定义任务" value={tasks.length} tone="violet" />
        <StatCard icon={Inbox} label="教务指令" value={inbox.length} tone="warning" badge={unread} />
      </Stagger>

      <Stagger className="grid gap-4 board:grid-cols-2" step={80}>
        <Card
          className="card-interactive"
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
          className="card-interactive"
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
      </Stagger>
    </div>
  );
}
