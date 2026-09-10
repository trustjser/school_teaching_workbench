import { useEffect } from 'react';
import { Link } from 'react-router-dom';
import { CalendarCheck, ClipboardList, GraduationCap, Inbox, Users } from 'lucide-react';
import { useStudentStore } from '@shared/store/useStudentStore';
import { useCheckinStore } from '@shared/store/useCheckinStore';
import { useTaskStore } from '@shared/store/useTaskStore';
import { useBroadcastStore } from '@shared/store/useBroadcastStore';
import { Card } from '@shared/components/ui/Card';
import { Button } from '@shared/components/ui/Button';
import { Badge } from '@shared/components/ui/Badge';
import { StatCard } from '@shared/components/ui/StatCard';
import { Stagger } from '@shared/components/motion/Reveal';

/** 班级首页：今日工作摘要 + 在读学生 / 出勤率 / 进行中任务 / 教务指令 + 任务矩阵与通知资料入口 */
export function ClientHome(): JSX.Element {
  const students = useStudentStore((s) => s.students);
  const tasks = useTaskStore((s) => s.tasks);
  const loadTasks = useTaskStore((s) => s.loadTasks);
  const inbox = useBroadcastStore((s) => s.inbox);
  const unread = useBroadcastStore((s) => s.unreadCount);
  const records = useCheckinStore((s) => s.records);

  // 本页计数依赖任务列表：其它页面的加载不能保证这里是最新的，
  // 因此进入首页时主动拉一次（否则刚标记为已结束的任务仍会被算进去）。
  useEffect(() => {
    void loadTasks();
  }, [loadTasks]);

  const rosterCount = students.filter((s) => s.status !== 'transferred').length;
  const present = Object.values(records).filter(
    (r) => r.state === 'present' || r.state === 'late',
  ).length;
  const rate = rosterCount > 0 ? Math.round((present / rosterCount) * 1000) / 10 : 0;

  // 首页只关心「还要做多少」：已结束的任务不算进行中。
  const activeTasks = tasks.filter((t) => t.status === 'active');
  const closedCount = tasks.length - activeTasks.length;

  return (
    <div className="space-y-6">
      {/* 今日工作摘要 */}
      <section className="card flex flex-wrap items-center justify-between gap-4 p-6">
        <div className="flex items-center gap-4">
          <div className="flex h-14 w-14 items-center justify-center rounded-2xl bg-brand-gradient text-white shadow-glow">
            <GraduationCap className="h-7 w-7" />
          </div>
          <div>
            <p className="text-sm font-medium text-ink-muted">今日工作</p>
            <h1 className="text-3xl font-bold text-ink">班级工作台</h1>
            <p className="mt-0.5 text-ink-soft">先完成考勤，再处理待办任务和通知资料。</p>
          </div>
        </div>
      </section>

      <Stagger className="grid grid-cols-2 gap-4 board:grid-cols-4" step={70}>
        <StatCard icon={Users} label="在读学生" value={rosterCount} tone="brand" />
        <StatCard icon={CalendarCheck} label="今日出勤率" value={rate} decimals={1} suffix="%" tone="success" accent />
        <StatCard
          icon={ClipboardList}
          label="进行中任务"
          value={activeTasks.length}
          tone="violet"
          sub={closedCount > 0 ? `另有 ${closedCount} 个已结束` : undefined}
        />
        <StatCard icon={Inbox} label="教务指令" value={inbox.length} tone="warning" badge={unread} />
      </Stagger>

      <Stagger className="grid gap-4 board:grid-cols-2" step={80}>
        <Card
          className="card-interactive"
          title="任务矩阵"
          description="点学生姓名逐人标记状态，网格 / 表格两种视图可切换"
          actions={
            <Link to="/matrix">
              <Button variant="ghost" size="md">
                打开矩阵
              </Button>
            </Link>
          }
        >
          <p className="text-ink-soft">
            {activeTasks.length > 0
              ? `当前有 ${activeTasks.length} 个进行中的任务，可以逐人标记完成情况。`
              : '暂无进行中的任务。可在「任务中心」新建，或等待教务处下发。'}
          </p>
          <Link to="/tasks">
            <Button variant="secondary" size="md" className="mt-3">
              进入任务中心
            </Button>
          </Link>
        </Card>

        <Card
          className="card-interactive"
          title="通知资料"
          description="查看教务处下发的通知与资料"
          actions={unread > 0 ? <Badge tone="warning">🔔 {unread} 条未读</Badge> : undefined}
        >
          <p className="text-ink-soft">
            {inbox.length > 0
              ? `收到 ${inbox.length} 条通知或资料，请及时查看并确认。`
              : '暂无新的通知或资料。'}
          </p>
          <Link to="/inbox">
            <Button variant="secondary" size="md" className="mt-3">
              查看通知资料
            </Button>
          </Link>
        </Card>
      </Stagger>
    </div>
  );
}
