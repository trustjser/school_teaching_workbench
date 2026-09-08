import { useEffect, useState } from 'react';
import { Drawer } from '@/components/ui/Drawer';
import { Input } from '@/components/ui/Input';
import { Select } from '@/components/ui/Select';
import { Textarea } from '@/components/ui/Textarea';
import { Button } from '@/components/ui/Button';
import { useStudentStore } from '@/store/useStudentStore';
import { useAppStore } from '@/store/useAppStore';
import type { Gender, StudentStatus } from '@/types/enums';
import type { ClassContext, Student as StudentModel } from '@/types/models';
import { uuidV4 } from '@/lib/crypto';
import { formatDateTime } from '@/lib/format';

export interface StudentEditDrawerProps {
  open: boolean;
  /** 为 null 表示新增 */
  student: StudentModel | null;
  /** 班级上下文（教务端目录 / 班级端绑定班级），用于落位 classId */
  classContext?: ClassContext | null;
  onClose: () => void;
}

const GENDER_OPTIONS = [
  { value: 'unknown', label: '未知' },
  { value: 'male', label: '男' },
  { value: 'female', label: '女' },
];

const STATUS_OPTIONS = [
  { value: 'active', label: '在读' },
  { value: 'leave', label: '长期请假' },
  { value: 'transferred', label: '已转出' },
];

interface FormState {
  studentNo: string;
  name: string;
  gender: Gender;
  grade: string;
  className: string;
  seatNo: string;
  phone: string;
  note: string;
  status: StudentStatus;
}

interface EditDefaults {
  classId: string | null;
  grade: string;
  className: string;
}

function toForm(student: StudentModel | null, defaults: EditDefaults): FormState {
  if (!student) {
    return {
      studentNo: '',
      name: '',
      gender: 'unknown',
      grade: defaults.grade,
      className: defaults.className,
      seatNo: '',
      phone: '',
      note: '',
      status: 'active',
    };
  }
  return {
    studentNo: student.studentNo,
    name: student.name,
    gender: student.gender,
    grade: student.grade ?? defaults.grade,
    className: student.className ?? defaults.className,
    seatNo: student.seatNo == null ? '' : String(student.seatNo),
    phone: student.phone ?? '',
    note: student.note ?? '',
    status: student.status,
  };
}

/** 学生编辑抽屉：信息编辑、状态变更、备注 */
export function StudentEditDrawer({
  open,
  student,
  classContext,
  onClose,
}: StudentEditDrawerProps): JSX.Element {
  const settings = useAppStore((s) => s.settings);
  const upsert = useStudentStore((s) => s.upsert);
  const ctx: EditDefaults = classContext
    ? {
        classId: classContext.classId,
        grade: classContext.grade ?? settings.grade ?? '',
        className: classContext.className ?? settings.className ?? '',
      }
    : {
        classId: settings.classId,
        grade: settings.grade ?? '',
        className: settings.className ?? '',
      };
  const [form, setForm] = useState<FormState>(() => toForm(null, ctx));
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (open) {
      setForm(toForm(student, ctx));
      setError(null);
    }
  }, [open, ctx.classId, ctx.className, ctx.grade, student]);

  const handleSave = async (): Promise<void> => {
    if (!form.name.trim()) {
      setError('姓名不能为空');
      return;
    }
    if (!form.studentNo.trim()) {
      setError('学号不能为空');
      return;
    }
    const seatNo = form.seatNo.trim() === '' ? null : Number.parseInt(form.seatNo, 10);
    if (form.seatNo.trim() !== '' && (seatNo === null || Number.isNaN(seatNo))) {
      setError('座位号必须是数字');
      return;
    }
    setSaving(true);
    try {
      await upsert({
        id: student?.id ?? uuidV4(),
        studentNo: form.studentNo.trim(),
        name: form.name.trim(),
        gender: form.gender,
        grade: form.grade.trim() || null,
        className: form.className.trim() || null,
        classId: ctx.classId,
        seatNo,
        status: form.status,
        phone: form.phone.trim() || null,
        note: form.note.trim() || null,
      });
      onClose();
    } catch {
      // 错误已在 store 中 toast
    } finally {
      setSaving(false);
    }
  };

  return (
    <Drawer
      open={open}
      onClose={onClose}
      title={student ? `编辑：${student.name}` : '新增学生'}
      footer={
        <>
          <Button variant="secondary" onClick={onClose}>
            取消
          </Button>
          <Button onClick={handleSave} loading={saving}>
            保存
          </Button>
        </>
      }
    >
      <div className="space-y-4">
        <Input
          label="姓名"
          value={form.name}
          onChange={(e) => setForm((f) => ({ ...f, name: e.target.value }))}
          placeholder="请输入学生姓名"
          error={error && !form.name.trim() ? error : undefined}
        />
        <Input
          label="学号"
          value={form.studentNo}
          onChange={(e) => setForm((f) => ({ ...f, studentNo: e.target.value }))}
          placeholder="如 2026030201"
        />
        <div className="grid grid-cols-2 gap-4">
          <Select
            label="性别"
            options={GENDER_OPTIONS}
            value={form.gender}
            onChange={(e) => setForm((f) => ({ ...f, gender: e.target.value as Gender }))}
          />
          <Input
            label="座位号"
            value={form.seatNo}
            onChange={(e) => setForm((f) => ({ ...f, seatNo: e.target.value }))}
            inputMode="numeric"
          />
        </div>
        <div className="grid grid-cols-2 gap-4">
          <Input
            label="年级"
            value={form.grade}
            onChange={(e) => setForm((f) => ({ ...f, grade: e.target.value }))}
          />
          <Input
            label="班级"
            value={form.className}
            onChange={(e) => setForm((f) => ({ ...f, className: e.target.value }))}
          />
        </div>
        <Input
          label="家长电话"
          value={form.phone}
          onChange={(e) => setForm((f) => ({ ...f, phone: e.target.value }))}
          inputMode="tel"
        />
        <Select
          label="状态"
          options={STATUS_OPTIONS}
          value={form.status}
          onChange={(e) => setForm((f) => ({ ...f, status: e.target.value as StudentStatus }))}
          hint="已转出的学生不再出现在考勤网格与日常统计中"
        />
        <Textarea
          label="备注"
          value={form.note}
          onChange={(e) => setForm((f) => ({ ...f, note: e.target.value }))}
          rows={4}
          showCount
        />
        {student && (
          <p className="text-sm text-ink-muted">
            创建于 {formatDateTime(student.createdAt)} · 更新于 {formatDateTime(student.updatedAt)}
          </p>
        )}
      </div>
    </Drawer>
  );
}
