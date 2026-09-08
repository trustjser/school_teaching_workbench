import { create } from 'zustand';
import type { Class, Grade } from '@/types/models';
import {
  classDelete,
  classList,
  classUpsert,
  gradeDelete,
  gradeList,
  gradeUpsert,
} from '@/lib/db';
import { useAppStore } from './useAppStore';

interface DirectoryState {
  grades: Grade[];
  classes: Class[];
  gradesLoading: boolean;
  classesLoading: boolean;
  /** 当前选中的年级（用于联动班级列表） */
  selectedGradeId: string | null;
  /** 当前选中的班级（用于联动名册） */
  selectedClassId: string | null;

  loadGrades: () => Promise<void>;
  loadClasses: (gradeId?: string | null) => Promise<void>;
  selectGrade: (id: string | null) => void;
  selectClass: (id: string | null) => void;

  upsertGrade: (grade: Partial<Grade> & { gradeName: string }) => Promise<Grade>;
  removeGrade: (id: string) => Promise<void>;
  upsertClass: (klass: Partial<Class> & { className: string }) => Promise<Class>;
  removeClass: (id: string) => Promise<void>;

  classesOfGrade: (gradeId: string) => Class[];
  classById: (id: string) => Class | undefined;
}

/** 按排序、名称升序 */
function sortGrades(list: Grade[]): Grade[] {
  return [...list].sort((a, b) => {
    if (a.sortOrder !== b.sortOrder) return a.sortOrder - b.sortOrder;
    return a.gradeName.localeCompare(b.gradeName, 'zh');
  });
}

function sortClasses(list: Class[]): Class[] {
  return [...list].sort((a, b) => {
    if (a.sortOrder !== b.sortOrder) return a.sortOrder - b.sortOrder;
    return a.className.localeCompare(b.className, 'zh');
  });
}

export const useDirectoryStore = create<DirectoryState>((set, get) => ({
  grades: [],
  classes: [],
  gradesLoading: false,
  classesLoading: false,
  selectedGradeId: null,
  selectedClassId: null,

  loadGrades: async () => {
    set({ gradesLoading: true });
    const app = useAppStore.getState();
    try {
      const list = await gradeList();
      const sorted = sortGrades(list);
      set({ grades: sorted, selectedGradeId: get().selectedGradeId ?? sorted[0]?.id ?? null });
    } catch (err) {
      app.toastError(err, '加载年级失败');
    } finally {
      set({ gradesLoading: false });
    }
  },

  loadClasses: async (gradeId) => {
    set({ classesLoading: true });
    const app = useAppStore.getState();
    try {
      const gid = gradeId === undefined ? get().selectedGradeId : gradeId;
      const list = await classList(gid);
      set({ classes: sortClasses(list) });
    } catch (err) {
      app.toastError(err, '加载班级失败');
    } finally {
      set({ classesLoading: false });
    }
  },

  selectGrade: (id) => {
    set({ selectedGradeId: id, selectedClassId: null });
    void get().loadClasses(id);
  },

  selectClass: (id) => set({ selectedClassId: id }),

  upsertGrade: async (grade) => {
    const app = useAppStore.getState();
    try {
      const saved = await gradeUpsert(grade);
      await get().loadGrades();
      set({ selectedGradeId: get().selectedGradeId ?? saved.id });
      app.pushToast({ kind: 'success', title: '已保存年级' });
      return saved;
    } catch (err) {
      app.toastError(err, '保存年级失败');
      throw err;
    }
  },

  removeGrade: async (id) => {
    const app = useAppStore.getState();
    try {
      await gradeDelete(id);
      await get().loadGrades();
      if (get().selectedGradeId === id) set({ selectedGradeId: null, selectedClassId: null });
      app.pushToast({ kind: 'success', title: '已删除年级' });
    } catch (err) {
      app.toastError(err, '删除年级失败');
      throw err;
    }
  },

  upsertClass: async (klass) => {
    const app = useAppStore.getState();
    try {
      const saved = await classUpsert(klass);
      await get().loadClasses(get().selectedGradeId);
      set({ selectedClassId: get().selectedClassId ?? saved.id });
      app.pushToast({ kind: 'success', title: '已保存班级' });
      return saved;
    } catch (err) {
      app.toastError(err, '保存班级失败');
      throw err;
    }
  },

  removeClass: async (id) => {
    const app = useAppStore.getState();
    try {
      await classDelete(id);
      await get().loadClasses(get().selectedGradeId);
      if (get().selectedClassId === id) set({ selectedClassId: null });
      app.pushToast({ kind: 'success', title: '已删除班级' });
    } catch (err) {
      app.toastError(err, '删除班级失败');
      throw err;
    }
  },

  classesOfGrade: (gradeId) => get().classes.filter((c) => c.gradeId === gradeId),
  classById: (id) => get().classes.find((c) => c.id === id),
}));
