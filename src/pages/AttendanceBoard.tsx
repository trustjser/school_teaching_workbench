import { useCallback, useEffect, useState } from 'react';
import { useDeviceStore } from '@/store/useDeviceStore';
import { Card } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';
import { Input } from '@/components/ui/Input';
import { Tabs } from '@/components/ui/Tabs';
import { Table, type TableColumn } from '@/components/ui/Table';
import { Badge } from '@/components/ui/Badge';
import { EmptyState } from '@/components/ui/EmptyState';
import { checkinSchoolSummary, checkinClassAttendance, checkinExceptionStudents } from '@/lib/db';
import { toDateKey, formatPercent } from '@/lib/format';
import type { SchoolSummary, ClassAttendanceRow, ExceptionStudentRow } from '@/types/api';

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
      render: (e) => (
        <Badge tone={e.state === 'absent' ? 'danger' : 'warning'}>
          {e.state === 'absent' ? '缺勤' : e.state === 'leave' ? '请假' : e.state}
        </Badge>
      ),
    },
    { key: 'note', header: '备注', accessor: (e) => e.note ?? '—' },
  ];

  return (
    <div className="space-y-4">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <h1 className="text-3xl font-bold text-ink">考勤大屏</h1>
        <div className="flex items-end gap-3">
          <Input type="date" label="日期" value={date} onChange={(e) => setDate(e.target.value || toDateKey(Date.now()))} />
          <Button variant="secondary" onClick={() => void refresh()} disabled={loading}>
            刷新
          </Button>
        </div>
      </div>

      {summary && (
        <div className="grid grid-cols-2 gap-4 board:grid-cols-4">
          <Card>
            <p className="text-base text-ink-muted">全校出勤率</p>
            <p className="mt-1 text-4xl font-bold text-ink">{formatPercent(summary.attendanceRate)}</p>
            <p className="mt-1 text-sm text-ink-muted">
              {summary.markedStudents}/{summary.totalStudents} 已标记
            </p>
          </Card>
          <Card>
            <p className="text-base text-ink-muted">已提交班级</p>
            <p className="mt-1 text-4xl font-bold text-ink">
              {summary.submittedClassCount}/{summary.classCount}
            </p>
          </Card>
          <Card>
            <p className="text-base text-ink-muted">缺勤</p>
            <p className="mt-1 text-4xl font-bold text-red-700">{summary.absent}</p>
          </Card>
          <Card>
            <p className="text-base text-ink-muted">请假</p>
            <p className="mt-1 text-4xl font-bold text-amber-700">{summary.leave}</p>
          </Card>
        </div>
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
