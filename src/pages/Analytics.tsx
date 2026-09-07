import { useEffect, useState } from 'react';
import { Download } from 'lucide-react';
import { useTaskStore } from '@/store/useTaskStore';
import { Card } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';
import { Table, type TableColumn } from '@/components/ui/Table';
import { Badge } from '@/components/ui/Badge';
import { Spinner } from '@/components/ui/Spinner';
import { checkinClassAttendance } from '@/lib/db';
import {
  exportSheetsToXlsx,
  buildTaskCompletionSheet,
  buildClassSummarySheet,
  defaultExportName,
} from '@/lib/exporter';
import { toDateKey, formatPercent } from '@/lib/format';
import type { TaskCompletionRow } from '@/types/api';
import type { ClassSummaryExportRow } from '@/lib/exporter';

/** 统计导出页（教务处端）：任务完成率 + 班级考勤汇总，导出 xlsx */
export function Analytics(): JSX.Element {
  const completion = useTaskStore((s) => s.completionStats);
  const loadCompletion = useTaskStore((s) => s.loadCompletionStats);
  const [loading, setLoading] = useState(false);
  const [exporting, setExporting] = useState(false);

  useEffect(() => {
    void loadCompletion();
  }, [loadCompletion]);

  const exportCompletion = async (): Promise<void> => {
    if (completion.length === 0) return;
    setExporting(true);
    try {
      const sheet = buildTaskCompletionSheet(completion);
      await exportSheetsToXlsx(defaultExportName('任务完成率'), [sheet]);
    } finally {
      setExporting(false);
    }
  };

  const exportClassSummary = async (): Promise<void> => {
    setExporting(true);
    try {
      const date = toDateKey(Date.now());
      const rows = await checkinClassAttendance(date);
      const mapped: ClassSummaryExportRow[] = rows.map((r) => ({
        className: r.className,
        grade: r.grade,
        total: r.total,
        present: r.present,
        leave: r.leave,
        absent: r.absent,
        late: r.late,
        attendanceRate: r.attendanceRate,
        submitted: r.submitted,
      }));
      const sheet = buildClassSummarySheet(mapped);
      await exportSheetsToXlsx(defaultExportName('班级考勤汇总'), [sheet]);
    } finally {
      setExporting(false);
    }
  };

  const columns: TableColumn<TaskCompletionRow>[] = [
    { key: 'title', header: '任务', accessor: (r) => r.title },
    { key: 'className', header: '班级', accessor: (r) => r.className ?? '—' },
    { key: 'total', header: '人数', accessor: (r) => r.total, align: 'center' },
    { key: 'final', header: '完成', accessor: (r) => r.finalCount, align: 'center' },
    {
      key: 'rate',
      header: '完成率',
      accessor: (r) => formatPercent(r.completionRate),
      align: 'center',
    },
    {
      key: 'avg',
      header: '平均分',
      accessor: (r) => (r.avgScore != null ? r.avgScore.toFixed(1) : '—'),
      align: 'center',
    },
  ];

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <h1 className="text-3xl font-bold text-ink">统计导出</h1>
        <div className="flex gap-3">
          <Button
            variant="secondary"
            icon={<Download className="h-5 w-5" />}
            loading={exporting}
            onClick={() => void exportCompletion()}
            disabled={completion.length === 0}
          >
            导出任务完成率
          </Button>
          <Button
            icon={<Download className="h-5 w-5" />}
            loading={exporting}
            onClick={() => void exportClassSummary()}
          >
            导出班级考勤汇总
          </Button>
        </div>
      </div>

      <Card title="任务完成率统计" description="各任务的最终状态完成率与平均评分">
        {loading ? (
          <Spinner label="加载统计…" />
        ) : completion.length === 0 ? (
          <p className="py-6 text-center text-ink-muted">暂无任务完成率统计数据。</p>
        ) : (
          <Table columns={columns} data={completion} rowKey={(r) => r.taskId} />
        )}
      </Card>

      <Card title="说明">
        <p className="text-ink-soft">
          导出为 <Badge tone="brand">.xlsx</Badge> 文件，可直接用 Excel / WPS 打开。任务完成率基于各任务的「终态」节点统计；
          班级考勤汇总基于当日各班级的考勤记录聚合。
        </p>
      </Card>
    </div>
  );
}
