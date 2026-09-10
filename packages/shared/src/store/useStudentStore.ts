import { create } from 'zustand';
import type { ClassContext, Student, StudentStatus } from '@shared/types/models';
import { studentDelete, studentList, studentUpdateStatus, studentUpsert } from '@shared/lib/db';
import { compareStudentNo } from '@shared/lib/format';
import { useAppStore } from './useAppStore';

/** 名册筛选 */
export type StudentFilter = 'all' | 'active' | 'leave' | 'transferred';

interface StudentState {
  students: Student[];
  loading: boolean;
  keyword: string;
  filter: StudentFilter;
  /** 最近一次导入批次 ID */
  lastBatchId: string | null;
  /** 当前名册作用域（教务端目录班级 / 班级端绑定班级） */
  scope: ClassContext | null;

  setKeyword: (keyword: string) => void;
  setFilter: (filter: StudentFilter) => void;
  /** 加载名册：传入 ctx 时按 classId 过滤（目录消费主路径），否则按设置 className 回退 */
  load: (ctx?: ClassContext | null) => Promise<void>;
  upsert: (student: Partial<Student> & { name: string; studentNo: string }) => Promise<Student>;
  changeStatus: (id: string, status: StudentStatus) => Promise<void>;
  remove: (id: string) => Promise<void>;
  /** 参与日常统计的学生（排除已转出） */
  activeStudents: () => Student[];
  /** 参与考勤网格的学生（在读 + 长期请假，排除已转出） */
  checkinStudents: () => Student[];
  byId: (id: string) => Student | undefined;
}

/** 排序：座位号 → 学号 */
function sortStudents(list: Student[]): Student[] {
  return [...list].sort((a, b) => {
    const sa = a.seatNo ?? Number.MAX_SAFE_INTEGER;
    const sb = b.seatNo ?? Number.MAX_SAFE_INTEGER;
    if (sa !== sb) return sa - sb;
    return compareStudentNo(a.studentNo, b.studentNo);
  });
}

export const useStudentStore = create<StudentState>((set, get) => ({
  students: [],
  loading: false,
  keyword: '',
  filter: 'all',
  lastBatchId: null,
  scope: null,

  setKeyword: (keyword) => set({ keyword }),
  setFilter: (filter) => set({ filter }),

  load: async (ctx) => {
    set({ loading: true });
    const app = useAppStore.getState();
    try {
      const scope = ctx === undefined ? null : ctx;
      const list = await studentList({
        classId: scope?.classId ?? null,
        className: scope ? scope.className : app.settings.className,
      });
      set({ students: sortStudents(list), scope });
    } catch (err) {
      app.toastError(err, '加载名册失败');
    } finally {
      set({ loading: false });
    }
  },

  upsert: async (student) => {
    const app = useAppStore.getState();
    try {
      const saved = await studentUpsert(student);
      set((s) => {
        const idx = s.students.findIndex((x) => x.id === saved.id);
        if (idx >= 0) {
          const next = [...s.students];
          next[idx] = saved;
          return { students: sortStudents(next) };
        }
        return { students: sortStudents([...s.students, saved]) };
      });
      app.pushToast({ kind: 'success', title: '已保存学生信息' });
      return saved;
    } catch (err) {
      app.toastError(err, '保存失败');
      throw err;
    }
  },

  changeStatus: async (id, status) => {
    const app = useAppStore.getState();
    const prev = get().students.find((s) => s.id === id);
    if (!prev) return;
    // 乐观更新
    set((s) => ({
      students: s.students.map((x) =>
        x.id === id ? { ...x, status, statusSince: Date.now() } : x,
      ),
    }));
    try {
      const saved = await studentUpdateStatus(id, status);
      set((s) => ({ students: s.students.map((x) => (x.id === id ? saved : x)) }));
      app.pushToast({
        kind: 'success',
        title: status === 'transferred' ? '已标记为转出' : '状态已更新',
        description: `${saved.name} 将${status === 'transferred' ? '不再出现在考勤网格' : '按新状态参与统计'}`,
      });
    } catch (err) {
      // 回滚
      set((s) => ({ students: s.students.map((x) => (x.id === id ? prev : x)) }));
      app.toastError(err, '状态更新失败');
      throw err;
    }
  },

  remove: async (id) => {
    const app = useAppStore.getState();
    const prev = get().students;
    set((s) => ({ students: s.students.filter((x) => x.id !== id) }));
    try {
      await studentDelete(id);
      app.pushToast({ kind: 'success', title: '已删除学生' });
    } catch (err) {
      set({ students: prev });
      app.toastError(err, '删除失败');
      throw err;
    }
  },

  activeStudents: () => get().students.filter((s) => s.status !== 'transferred'),
  checkinStudents: () => get().students.filter((s) => s.status !== 'transferred'),
  byId: (id) => get().students.find((s) => s.id === id),
}));

/** 依据筛选与关键字过滤 */
export function filterStudents(
  students: Student[],
  filter: StudentFilter,
  keyword: string,
): Student[] {
  const kw = keyword.trim().toLowerCase();
  return students.filter((s) => {
    if (filter !== 'all' && s.status !== filter) return false;
    if (!kw) return true;
    return (
      s.name.toLowerCase().includes(kw) ||
      s.studentNo.toLowerCase().includes(kw) ||
      (s.phone ?? '').toLowerCase().includes(kw)
    );
  });
}
