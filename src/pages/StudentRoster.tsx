import { useEffect, useState } from 'react';
import { useStudentStore } from '@/store/useStudentStore';
import { StudentTable } from '@/components/student/StudentTable';
import { StudentImportDialog } from '@/components/student/StudentImportDialog';
import { StudentEditDrawer } from '@/components/student/StudentEditDrawer';
import type { Student } from '@/types/models';

/** 学生名册页：导入 / 编辑 / 删除 / 状态变更 */
export function StudentRoster(): JSX.Element {
  const load = useStudentStore((s) => s.load);
  const [importOpen, setImportOpen] = useState(false);
  const [editTarget, setEditTarget] = useState<Student | null>(null);

  useEffect(() => {
    void load();
  }, [load]);

  return (
    <div className="space-y-4">
      <h1 className="text-3xl font-bold text-ink">学生名册</h1>
      <div className="animate-rise-in">
        <StudentTable onEdit={setEditTarget} onImport={() => setImportOpen(true)} />
      </div>
      <StudentImportDialog open={importOpen} onClose={() => setImportOpen(false)} />
      <StudentEditDrawer
        open={editTarget !== null}
        student={editTarget}
        onClose={() => setEditTarget(null)}
      />
    </div>
  );
}
