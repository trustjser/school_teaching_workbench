import { useCallback, useEffect, useState } from 'react';
import { CalendarCheck, CheckCircle2, RefreshCw, UserMinus, UserX } from 'lucide-react';
import { useDeviceStore } from '@shared/store/useDeviceStore';
import { Card } from '@shared/components/ui/Card';
import { Button } from '@shared/components/ui/Button';
import { Input } from '@shared/components/ui/Input';
import { Tabs } from '@shared/components/ui/Tabs';
import { Table, type TableColumn } from '@shared/components/ui/Table';
import { Badge } from '@shared/components/ui/Badge';
import { EmptyState } from '@shared/components/ui/EmptyState';
import { StatCard } from '@shared/components/ui/StatCard';
import { SkeletonStatCard } from '@shared/components/ui/Skeleton';
import { Stagger } from '@shared/components/motion/Reveal';
import { CHECKIN_STATUS_META } from '@shared/constants/status';
import { checkinSchoolSummary, checkinClassAttendance, checkinExceptionStudents } from '@shared/lib/db';
import { toDateKey, formatPercent } from '@shared/lib/format';
import type { SchoolSummary, ClassAttendanceRow, ExceptionStudentRow } from '@shared/types/api';
import type { CheckinState } from '@shared/types/enums';
import { useTauriEventHandler } from '@shared/hooks/useTauriEvent';
import { TAURI_EVENTS } from '@shared/types/events';

/** 异常状态 → 徽标配色（与 CHECKIN_STATUS_META 的中文文案配套） */
const EXCEPTION_TONE: Partial<Record<CheckinState, 'danger' | 'warning' | 'orange' | 'neutral'>> = {
  absent: 'danger',
  leave: 'warning',
  late: 'orange',
};

/** 考勤大屏（教务处端）：全校汇总 + 按班级明细 + 异常学生名单 */
export function AttendanceBoard(): JSX.Element {
  const loadDevices = useDeviceStore((s) => s.load);
  const [date, setDate] = useState(toDateKey(Date.now()));
  const [summary, setSummary] = useState<SchoolSummary | null>(null);
  const [classes, setClasses] = useState<ClassAttendanceRow[]>([]);
  const [exceptions, setExceptions] = useState<ExceptionStudentRow[]>([]);
  const [tab, setTab] = useState<'class' | 'exception'>('class');
  const [loading, setLoading] = useState(false);

  const refresh = useCallback(async (): Promise<void> => {
    setLoading(true);
    try {
      const [s, c, e] = await Promise.all([
        checkinSchoolSummary(date),
        checkinClassAttendance(date),
        checkinExceptionStudents(date),
      ]);
      setSummary(s);
      setClasses(c);
      setExceptions(e);
    } catch {
      /* 教务处端统计为可选能力，失败时静默 */
    } finally {
      setLoading(false);
    }
  }, [date]);

  useEffect(() => {
    void loadDevices();
    void refresh();
  }, [loadDevices, refresh]);

  // 班级端通过同步队列提交考勤后，教务端实时刷新当前日期的大屏数据。
  useTauriEventHandler(TAURI_EVENTS.CHECKIN_UPDATED, () => {
    void refresh();
  });

  const classColumns: TableColumn<ClassAttendanceRow>[] = [
    { key: 'className', header: '班级', accessor: (c) => c.className },
    { key: 'grade', header: '年级', accessor: (c) => c.grade ?? '—' },
    { key: 'total', header: '应到', accessor: (c) => c.total, align: 'center' },
    { key: 'present', header: '出勤', accessor: (c) => c.present, align: 'center' },
    { key: 'leave', header: '请假', accessor: (c) => c.leave, align: 'center' },
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

  const exceptionColumns: TableColumn<ExceptionStudentRow>[] = [
    { key: 'className', header: '班级', accessor: (e) => e.className ?? '—' },
    { key: 'studentNo', header: '学号', accessor: (e) => e.studentNo },
    { key: 'name', header: '姓名', accessor: (e) => e.name },
    {
      key: 'state',
      header: '状态',
      render: (e) => {
        // 统一走考勤状态字典，避免 raw 值（如 late）直接透到界面上。
        const meta = CHECKIN_STATUS_META[e.state];
        return (
          <Badge tone={EXCEPTION_TONE[e.state] ?? 'neutral'} emoji={meta?.emoji}>
            {meta?.label ?? e.state}
          </Badge>
        );
      },
    },
    { key: 'period', header: '时段', accessor: (e) => e.period },
    {
      key: 'note',
      header: '备注',
      render: (e) => {
        const note = e.note?.trim();
        if (!note) return <span className="text-ink-muted">—</span>;
        return (
          <span className="block max-w-[22rem] truncate" title={note}>
            {note}
          </span>
        );
      },
    },
  ];

  return (
    <div className="space-y-4">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <h1 className="text-3xl font-bold text-ink">考勤大屏</h1>
        <div className="flex items-end gap-3">
          <Input type="date" label="日期" value={date} onChange={(e) => setDate(e.target.value || toDateKey(Date.now()))} />
          <Button
            variant="secondary"
            size="md"
            className="shrink-0"
            icon={<RefreshCw className="h-4 w-4" />}
            loading={loading}
            onClick={() => void refresh()}
          >
            刷新
          </Button>
        </div>
      </div>

      {loading && !summary ? (
        <div className="grid grid-cols-2 gap-4 board:grid-cols-4">
          <SkeletonStatCard />
          <SkeletonStatCard />
          <SkeletonStatCard />
          <SkeletonStatCard />
        </div>
      ) : (
        summary && (
          <Stagger className="grid grid-cols-2 gap-4 board:grid-cols-4" step={70}>
            <StatCard
              tone="success"
              icon={CalendarCheck}
              label="全校出勤率"
              value={summary.attendanceRate ?? 0}
              decimals={1}
              suffix="%"
              accent
              sub={`${summary.markedStudents ?? 0}/${summary.totalStudents ?? 0} 已标记`}
            />
            <StatCard
              tone="brand"
              icon={CheckCircle2}
              label="已提交班级"
              value={`${summary.submittedClassCount ?? 0}/${summary.classCount ?? 0}`}
              sub="已提交 / 总班级"
            />
            <StatCard tone="danger" icon={UserX} label="缺勤" value={summary.absent ?? 0} accent sub="人" />
            <StatCard tone="warning" icon={UserMinus} label="请假" value={summary.leave ?? 0} accent sub="人" />
          </Stagger>
        )
      )}

      <Card
        title="班级明细 / 异常名单"
        actions={
          <Tabs
            items={[
              { value: 'class', label: '班级明细', count: classes.length },
              { value: 'exception', label: '异常名单', count: exceptions.length },
            ]}
            value={tab}
            onChange={(v) => setTab(v as 'class' | 'exception')}
          />
        }
      >
        {tab === 'class' ? (
          classes.length === 0 ? (
            <EmptyState title="暂无班级考勤数据" compact />
          ) : (
            <Table columns={classColumns} data={classes} rowKey={(c) => c.className} />
          )
        ) : exceptions.length === 0 ? (
          <EmptyState title="暂无异常学生" description="当日无缺勤或请假记录。" compact />
        ) : (
          <Table columns={exceptionColumns} data={exceptions} rowKey={(e) => e.studentId} />
        )}
      </Card>
    </div>
  );
}
