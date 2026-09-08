import { useCallback, useEffect, useState, type ReactNode } from 'react';
import { Link } from 'react-router-dom';
import { BarChart3, CalendarCheck, Monitor, Radio, RefreshCw } from 'lucide-react';
import { useDeviceStore } from '@/store/useDeviceStore';
import { useBroadcastStore } from '@/store/useBroadcastStore';
import { Card } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';
import { Badge } from '@/components/ui/Badge';
import { Spinner } from '@/components/ui/Spinner';
import { StatCard } from '@/components/ui/StatCard';
import { SkeletonStatCard } from '@/components/ui/Skeleton';
import { Table, type TableColumn } from '@/components/ui/Table';
import { Stagger } from '@/components/motion/Reveal';
import { checkinSchoolSummary, checkinClassAttendance } from '@/lib/db';
import { toDateKey, formatPercent } from '@/lib/format';
import type { SchoolSummary, ClassAttendanceRow } from '@/types/api';

/** 教务处首页：节点监控概览 + 全校考勤大屏 + 任务下发 + 统计导出入口 */
export function MasterHome(): JSX.Element {
  const devices = useDeviceStore((s) => s.devices);
  const loadDevices = useDeviceStore((s) => s.load);
  const outbox = useBroadcastStore((s) => s.outbox);
  const loadOutbox = useBroadcastStore((s) => s.loadOutbox);
  const [summary, setSummary] = useState<SchoolSummary | null>(null);
  const [classes, setClasses] = useState<ClassAttendanceRow[]>([]);
  const [loading, setLoading] = useState(false);

  const refresh = useCallback(async (): Promise<void> => {
    setLoading(true);
    try {
      const date = toDateKey(Date.now());
      const [s, c] = await Promise.all([checkinSchoolSummary(date), checkinClassAttendance(date)]);
      setSummary(s);
      setClasses(c);
    } catch {
      /* 教务处端统计为可选能力，失败时静默 */
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void loadDevices();
    void loadOutbox();
    void refresh();
  }, [loadDevices, loadOutbox, refresh]);

  const online = devices.filter((d) => d.status === 'online').length;

  const classColumns: TableColumn<ClassAttendanceRow>[] = [
    { key: 'className', header: '班级', accessor: (c) => c.className },
    { key: 'grade', header: '年级', accessor: (c) => c.grade ?? '—' },
    { key: 'total', header: '应到', accessor: (c) => c.total, align: 'center' },
    { key: 'present', header: '出勤', accessor: (c) => c.present, align: 'center' },
    { key: 'absent', header: '缺勤', accessor: (c) => c.absent, align: 'center' },
    {
      key: 'rate',
      header: '出勤率',
      accessor: (c) => formatPercent(c.attendanceRate),
      align: 'center',
    },
    {
      key: 'submitted',
      header: '已提交',
      align: 'center',
      render: (c) => (c.submitted ? <Badge tone="success">是</Badge> : <Badge tone="neutral">否</Badge>),
    },
  ];

  return (
    <div className="space-y-6">
      {/* 欢迎条 */}
      <section className="card flex flex-wrap items-center justify-between gap-4 p-6">
        <div className="flex items-center gap-4">
          <div className="flex h-14 w-14 items-center justify-center rounded-2xl bg-brand-gradient text-white shadow-glow">
            <Monitor className="h-7 w-7" />
          </div>
          <div>
            <p className="text-sm font-medium text-ink-muted">教务处协同端</p>
            <h1 className="text-3xl font-bold text-ink">全校总览</h1>
            <p className="mt-0.5 text-ink-soft">节点状态、全校考勤与任务下发，一屏掌握。</p>
          </div>
        </div>
        <Button
          variant="secondary"
          size="md"
          icon={<RefreshCw className="h-4 w-4" />}
          loading={loading}
          onClick={() => void refresh()}
        >
          刷新
        </Button>
      </section>

      {loading && !summary ? (
        <div className="grid grid-cols-2 gap-4 board:grid-cols-4">
          <SkeletonStatCard />
          <SkeletonStatCard />
          <SkeletonStatCard />
          <SkeletonStatCard />
        </div>
      ) : (
        <Stagger className="grid grid-cols-2 gap-4 board:grid-cols-4" step={70}>
          <StatCard
            tone="brand"
            icon={Monitor}
            label="已发现节点"
            value={devices.length}
            sub={`在线 ${online}`}
          />
          <StatCard
            tone="success"
            icon={CalendarCheck}
            label="全校出勤率"
            value={summary ? summary.attendanceRate ?? 0 : 0}
            decimals={1}
            suffix="%"
            accent
            sub={summary ? `${summary.markedStudents ?? 0}/${summary.totalStudents ?? 0} 已标记` : '—'}
          />
          <StatCard tone="violet" icon={Radio} label="已下发任务" value={outbox.length} sub="教务处下发" />
          <StatCard
            tone="warning"
            icon={BarChart3}
            label="异常学生"
            value={summary ? (summary.absent ?? 0) + (summary.leave ?? 0) : 0}
            accent
            sub="缺勤 + 请假"
          />
        </Stagger>
      )}

      <Card
        title="各班级考勤"
        description={loading ? '加载中…' : `共 ${classes.length} 个班级`}
        actions={
          <Link to="/master/attendance">
            <Button variant="ghost" size="md">
              考勤大屏
            </Button>
          </Link>
        }
      >
        {loading && classes.length === 0 ? (
          <Spinner label="加载全校考勤…" />
        ) : (
          <Table
            columns={classColumns}
            data={classes}
            rowKey={(c) => c.className}
            empty={<span>暂无班级考勤数据。</span>}
          />
        )}
      </Card>

      <Stagger className="grid gap-4 board:grid-cols-3" step={80}>
        <QuickLink to="/master/devices" title="节点监控" desc="查看局域网内班级端与教务处端节点状态" />
        <QuickLink to="/master/broadcast" title="任务下发" desc="向指定班级 / 年级 / 全校下发任务" />
        <QuickLink to="/master/analytics" title="统计导出" desc="导出任务完成率与考勤汇总（xlsx）" />
      </Stagger>
    </div>
  );
}

function QuickLink({ to, title, desc }: { to: string; title: string; desc: string }): JSX.Element {
  return (
    <Link to={to}>
      <Card className="card-interactive group h-full">
        <div className="flex items-center justify-between gap-3">
          <h3 className="text-xl font-bold text-ink">{title}</h3>
          <span className="text-brand-600 transition-transform group-hover:translate-x-1">→</span>
        </div>
        <p className="mt-1 text-ink-soft">{desc}</p>
      </Card>
    </Link>
  );
}
