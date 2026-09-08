import { useEffect, useState } from 'react';
import { useStudentStore } from '@/store/useStudentStore';
import { useAppStore } from '@/store/useAppStore';
import { StudentTable } from '@/components/student/StudentTable';
import { StudentEditDrawer } from '@/components/student/StudentEditDrawer';
import type { Student } from '@/types/models';

/** 学生名册页：编辑 / 删除 / 状态变更（班级端按绑定班级消费，名册由教务处下发，本地不导入） */
export function StudentRoster(): JSX.Element {
  const load = useStudentStore((s) => s.load);
  const settings = useAppStore((s) => s.settings);
  const [editTarget, setEditTarget] = useState<Student | null>(null);

  useEffect(() => {
    void load({
      classId: settings.classId,
      className: settings.className ?? '',
      grade: settings.grade,
    });
  }, [load, settings.classId, settings.className, settings.grade]);

  const classContext = {
    classId: settings.classId,
    className: settings.className ?? '',
    grade: settings.grade,
  };

  return (
    <div className="space-y-4">
      <h1 className="text-3xl font-bold text-ink">
        学生名册{settings.className ? ` · ${settings.className}` : ''}
      </h1>
      <div className="animate-rise-in">
        <StudentTable onEdit={setEditTarget} />
      </div>
      <StudentEditDrawer
        open={editTarget !== null}
        student={editTarget}
        classContext={classContext}
        onClose={() => setEditTarget(null)}
      />
    </div>
  );
}
