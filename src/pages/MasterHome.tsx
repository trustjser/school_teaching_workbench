import { useCallback, useEffect, useState, type ReactNode } from 'react';
import { Link } from 'react-router-dom';
import { Monitor, CalendarCheck, Radio, BarChart3 } from 'lucide-react';
import { useDeviceStore } from '@/store/useDeviceStore';
import { useBroadcastStore } from '@/store/useBroadcastStore';
import { Card } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';
import { Badge } from '@/components/ui/Badge';
import { Spinner } from '@/components/ui/Spinner';
import { Table, type TableColumn } from '@/components/ui/Table';
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
      <div className="flex flex-wrap items-center justify-between gap-3">
        <h1 className="text-3xl font-bold text-ink">全校总览</h1>
        <Button variant="secondary" size="md" onClick={() => void refresh()} disabled={loading}>
          刷新
        </Button>
      </div>

      <div className="grid grid-cols-2 gap-4 board:grid-cols-4">
        <HomeStat icon={<Monitor className="h-6 w-6" />} label="已发现节点" value={devices.length} sub={`在线 ${online}`} />
        <HomeStat
          icon={<CalendarCheck className="h-6 w-6" />}
          label="全校出勤率"
          value={summary ? formatPercent(summary.attendanceRate) : '—'}
          sub={summary ? `${summary.markedStudents}/${summary.totalStudents} 已标记` : '加载中'}
        />
        <HomeStat
          icon={<Radio className="h-6 w-6" />}
          label="已下发任务"
          value={outbox.length}
          sub="教务处下发"
        />
        <HomeStat
          icon={<BarChart3 className="h-6 w-6" />}
          label="异常学生"
          value={summary ? summary.absent + summary.leave : '—'}
          sub="缺勤 + 请假"
        />
      </div>

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

      <div className="grid gap-4 board:grid-cols-3">
        <QuickLink to="/master/devices" title="节点监控" desc="查看局域网内班级端与教务处端节点状态" />
        <QuickLink to="/master/broadcast" title="任务下发" desc="向指定班级 / 年级 / 全校下发任务" />
        <QuickLink to="/master/analytics" title="统计导出" desc="导出任务完成率与考勤汇总（xlsx）" />
      </div>
    </div>
  );
}

function HomeStat({
  icon,
  label,
  value,
  sub,
}: {
  icon: ReactNode;
  label: string;
  value: string | number;
  sub?: string;
}): JSX.Element {
  return (
    <Card>
      <div className="flex items-center gap-2 text-ink-muted">
        <span className="text-brand-600">{icon}</span>
        <span className="text-base">{label}</span>
      </div>
      <p className="mt-2 text-4xl font-bold text-ink">{value}</p>
      {sub && <p className="mt-1 text-sm text-ink-muted">{sub}</p>}
    </Card>
  );
}

function QuickLink({ to, title, desc }: { to: string; title: string; desc: string }): JSX.Element {
  return (
    <Link to={to}>
      <Card className="h-full transition-colors hover:border-brand-500">
        <h3 className="text-xl font-bold text-ink">{title}</h3>
        <p className="mt-1 text-ink-soft">{desc}</p>
      </Card>
    </Link>
  );
}
