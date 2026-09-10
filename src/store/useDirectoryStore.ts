import { create } from 'zustand';
import type { Class, Grade, SchoolYear } from '@/types/models';
import {
  classDelete,
  classList,
  classUpsert,
  gradeDelete,
  gradeList,
  gradeUpsert,
  schoolYearDelete,
  schoolYearList,
  schoolYearUpsert,
} from '@/lib/db';
import { useAppStore } from './useAppStore';

interface DirectoryState {
  grades: Grade[];
  classes: Class[];
  schoolYears: SchoolYear[];
  gradesLoading: boolean;
  classesLoading: boolean;
  schoolYearsLoading: boolean;
  /** 当前选中的年级（用于联动班级列表） */
  selectedGradeId: string | null;
  /** 当前选中的班级（用于联动名册） */
  selectedClassId: string | null;
  /** 当前选中的学年（年隔离维度，班级按 (学年, 年级) 过滤） */
  selectedSchoolYearId: string | null;

  loadGrades: () => Promise<void>;
  loadClasses: (gradeId?: string | null) => Promise<void>;
  loadSchoolYears: () => Promise<void>;
  selectGrade: (id: string | null) => void;
  selectClass: (id: string | null) => void;
  selectSchoolYear: (id: string | null) => void;

  upsertGrade: (grade: Partial<Grade> & { gradeName: string }) => Promise<Grade>;
  removeGrade: (id: string) => Promise<void>;
  upsertClass: (klass: Partial<Class> & { className: string }) => Promise<Class>;
  removeClass: (id: string) => Promise<void>;
  upsertSchoolYear: (
    year: Partial<SchoolYear> & { schoolYearName: string },
  ) => Promise<SchoolYear>;
  removeSchoolYear: (id: string) => Promise<void>;

  classesOfGrade: (gradeId: string) => Class[];
  classById: (id: string) => Class | undefined;
  currentSchoolYear: () => SchoolYear | undefined;
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

function sortSchoolYears(list: SchoolYear[]): SchoolYear[] {
  return [...list].sort((a, b) => {
    if (a.sortOrder !== b.sortOrder) return a.sortOrder - b.sortOrder;
    return a.schoolYearName.localeCompare(b.schoolYearName, 'zh');
  });
}

export const useDirectoryStore = create<DirectoryState>((set, get) => ({
  grades: [],
  classes: [],
  schoolYears: [],
  gradesLoading: false,
  classesLoading: false,
  schoolYearsLoading: false,
  selectedGradeId: null,
  selectedClassId: null,
  selectedSchoolYearId: null,

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
      // 班级按 (学年, 年级) 双重过滤；学年为空表示「全部学年」。
      const list = await classList(gid, get().selectedSchoolYearId);
      set({ classes: sortClasses(list) });
    } catch (err) {
      app.toastError(err, '加载班级失败');
    } finally {
      set({ classesLoading: false });
    }
  },

  loadSchoolYears: async () => {
    set({ schoolYearsLoading: true });
    const app = useAppStore.getState();
    try {
      const list = await schoolYearList();
      const sorted = sortSchoolYears(list);
      set({
        schoolYears: sorted,
        selectedSchoolYearId: get().selectedSchoolYearId ?? sorted[0]?.id ?? null,
      });
      // 学年列表和年级列表并行加载时，补一次联动加载，避免班级仍停留在“全部学年”或空列表。
      await get().loadClasses(get().selectedGradeId);
    } catch (err) {
      app.toastError(err, '加载学年失败');
    } finally {
      set({ schoolYearsLoading: false });
    }
  },

  selectGrade: (id) => {
    set({ selectedGradeId: id, selectedClassId: null });
    void get().loadClasses(id);
  },

  selectClass: (id) => set({ selectedClassId: id }),

  selectSchoolYear: (id) => {
    set({ selectedSchoolYearId: id, selectedClassId: null });
    void get().loadClasses(get().selectedGradeId);
  },

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

  upsertSchoolYear: async (year) => {
    const app = useAppStore.getState();
    try {
      const saved = await schoolYearUpsert(year);
      await get().loadSchoolYears();
      set({ selectedSchoolYearId: get().selectedSchoolYearId ?? saved.id });
      app.pushToast({ kind: 'success', title: '已保存学年' });
      return saved;
    } catch (err) {
      app.toastError(err, '保存学年失败');
      throw err;
    }
  },

  removeSchoolYear: async (id) => {
    const app = useAppStore.getState();
    try {
      await schoolYearDelete(id);
      await get().loadSchoolYears();
      if (get().selectedSchoolYearId === id) set({ selectedSchoolYearId: null, selectedClassId: null });
      app.pushToast({ kind: 'success', title: '已删除学年' });
    } catch (err) {
      app.toastError(err, '删除学年失败');
      throw err;
    }
  },

  classesOfGrade: (gradeId) => get().classes.filter((c) => c.gradeId === gradeId),
  classById: (id) => get().classes.find((c) => c.id === id),
  currentSchoolYear: () =>
    get().schoolYears.find((y) => y.id === get().selectedSchoolYearId) ?? undefined,
}));
