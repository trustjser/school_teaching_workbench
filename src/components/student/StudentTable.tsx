import { useMemo, useState } from 'react';
import { UserMinus, UserX } from 'lucide-react';
import { Table, type TableColumn } from '@/components/ui/Table';
import { StudentStatusBadge } from './StudentStatusBadge';
import { ConfirmDialog } from '@/components/ui/ConfirmDialog';
import { EmptyState } from '@/components/ui/EmptyState';
import { Button } from '@/components/ui/Button';
import { Tabs } from '@/components/ui/Tabs';
import type { Student } from '@/types/models';
import { filterStudents, useStudentStore, type StudentFilter } from '@/store/useStudentStore';
import { useAppStore } from '@/store/useAppStore';
import { maskPhone } from '@/lib/format';

export interface StudentTableProps {
  onEdit: (student: Student) => void;
  onImport: () => void;
}

type FilterTab = StudentFilter;

const TAB_ITEMS: { value: FilterTab; label: string; emoji: string }[] = [
  { value: 'all', label: '全部', emoji: '📋' },
  { value: 'active', label: '在读', emoji: '🟢' },
  { value: 'leave', label: '长期请假', emoji: '🟡' },
  { value: 'transferred', label: '已转出', emoji: '⚪' },
];

/** 名册表格：筛选、排序、批量改状态、编辑与删除入口 */
export function StudentTable({ onEdit, onImport }: StudentTableProps): JSX.Element {
  const students = useStudentStore((s) => s.students);
  const keyword = useStudentStore((s) => s.keyword);
  const filter = useStudentStore((s) => s.filter);
  const setFilter = useStudentStore((s) => s.setFilter);
  const changeStatus = useStudentStore((s) => s.changeStatus);
  const remove = useStudentStore((s) => s.remove);
  const pushToast = useAppStore((s) => s.pushToast);

  const [pendingAction, setPendingAction] = useState<
    { type: 'transfer' | 'delete'; student: Student } | null
  >(null);
  const [sortDesc, setSortDesc] = useState(false);

  const filtered = useMemo(() => {
    const list = filterStudents(students, filter, keyword);
    return sortDesc ? [...list].reverse() : list;
  }, [filter, keyword, sortDesc, students]);

  const columns: TableColumn<Student>[] = [
    { key: 'seatNo', header: '座位', accessor: (s) => s.seatNo ?? '-', widthClass: 'w-20', align: 'center' },
    { key: 'studentNo', header: '学号', accessor: (s) => s.studentNo, widthClass: 'w-40' },
    {
      key: 'name',
      header: '姓名',
      render: (s) => (
        <button
          type="button"
          className="min-h-touch text-left text-lg font-semibold text-brand-700 underline-offset-2 hover:underline"
          onClick={() => onEdit(s)}
        >
          {s.name}
        </button>
      ),
    },
    { key: 'gender', header: '性别', accessor: (s) => (s.gender === 'male' ? '男' : s.gender === 'female' ? '女' : '未知'), widthClass: 'w-24' },
    { key: 'className', header: '班级', accessor: (s) => s.className ?? '-', widthClass: 'w-40' },
    { key: 'phone', header: '家长电话', accessor: (s) => maskPhone(s.phone), widthClass: 'w-40' },
    {
      key: 'status',
      header: '状态',
      render: (s) => <StudentStatusBadge status={s.status} />,
      widthClass: 'w-32',
    },
    {
      key: 'actions',
      header: '操作',
      align: 'right',
      widthClass: 'w-64',
      render: (s) => (
        <div className="flex items-center justify-end gap-2">
          {s.status !== 'transferred' && (
            <Button
              size="md"
              variant="secondary"
              icon={<UserMinus className="h-4 w-4" />}
              onClick={() => setPendingAction({ type: 'transfer', student: s })}
            >
              标记转出
            </Button>
          )}
          <Button
            size="md"
            variant="danger"
            icon={<UserX className="h-4 w-4" />}
            onClick={() => setPendingAction({ type: 'delete', student: s })}
          >
            删除
          </Button>
        </div>
      ),
    },
  ];

  const activeCount = students.filter((s) => s.status === 'active').length;
  const leaveCount = students.filter((s) => s.status === 'leave').length;
  const transferredCount = students.filter((s) => s.status === 'transferred').length;
  const counts: Record<FilterTab, number> = {
    all: students.length,
    active: activeCount,
    leave: leaveCount,
    transferred: transferredCount,
  };

  const tabItems = TAB_ITEMS.map((t) => ({ ...t, count: counts[t.value] }));

  if (students.length === 0) {
    return (
      <EmptyState
        title="还没有学生名册"
        description="导入 Excel / CSV 名册后，即可开始快捷考勤与任务矩阵管理。"
        action={<Button onClick={onImport}>导入名册</Button>}
      />
    );
  }

  return (
    <div className="space-y-4">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <Tabs items={tabItems} value={filter} onChange={setFilter} />
        <Button size="md" variant="secondary" onClick={() => setSortDesc((v) => !v)}>
          {sortDesc ? '倒序' : '正序'}（按座位号）
        </Button>
      </div>

      <Table
        columns={columns}
        data={filtered}
        rowKey={(s) => s.id}
        empty={<span className="text-ink-muted">没有符合筛选条件的学生</span>}
      />

      <ConfirmDialog
        open={pendingAction !== null}
        danger={pendingAction?.type === 'delete'}
        title={pendingAction?.type === 'delete' ? '删除学生' : '标记为已转出'}
        message={
          pendingAction?.type === 'delete'
            ? `确定删除「${pendingAction?.student.name}」吗？`
            : `确定将「${pendingAction?.student.name}」标记为已转出吗？`
        }
        detail={
          pendingAction?.type === 'delete'
            ? '删除后该学生不再出现在名册与考勤中（历史考勤记录保留）。'
            : '已转出学生不再出现在考勤网格与日常统计中，但历史考勤仍可查询。'
        }
        confirmText={pendingAction?.type === 'delete' ? '删除' : '确认转出'}
        onCancel={() => setPendingAction(null)}
        onConfirm={() => {
          const action = pendingAction;
          setPendingAction(null);
          if (!action) return;
          if (action.type === 'delete') {
            void remove(action.student.id);
          } else {
            void changeStatus(action.student.id, 'transferred').then(() => {
              pushToast({
                kind: 'info',
                title: `${action.student.name} 已转出`,
                description: '该学生已从考勤网格中移除',
              });
            });
          }
        }}
      />
    </div>
  );
}
