import { useMemo, useState } from 'react';
import { Search } from 'lucide-react';
import { Input } from '@shared/components/ui/Input';
import { Button } from '@shared/components/ui/Button';
import { Badge } from '@shared/components/ui/Badge';
import { StudentStatusBadge } from './StudentStatusBadge';
import type { Student } from '@shared/types/models';
import { filterStudents } from '@shared/store/useStudentStore';

export interface StudentPickerProps {
  students: Student[];
  /** 已选中的学生 ID */
  selected: string[];
  onChange: (ids: string[]) => void;
  /** 最大可选数量（0 表示不限） */
  max?: number;
  /** 是否排除已转出（默认排除） */
  excludeTransferred?: boolean;
}

/** 学生选择器：批量操作对象选择（全选 / 反选 / 搜索） */
export function StudentPicker({
  students,
  selected,
  onChange,
  max = 0,
  excludeTransferred = true,
}: StudentPickerProps): JSX.Element {
  const [keyword, setKeyword] = useState('');

  const candidates = useMemo(() => {
    const base = excludeTransferred
      ? students.filter((s) => s.status !== 'transferred')
      : students;
    return filterStudents(base, 'all', keyword);
  }, [excludeTransferred, keyword, students]);

  const selectedSet = useMemo(() => new Set(selected), [selected]);

  const toggle = (id: string): void => {
    if (selectedSet.has(id)) {
      onChange(selected.filter((x) => x !== id));
      return;
    }
    if (max > 0 && selected.length >= max) return;
    onChange([...selected, id]);
  };

  const allSelected =
    candidates.length > 0 && candidates.every((s) => selectedSet.has(s.id));

  return (
    <div className="space-y-3">
      <div className="flex flex-wrap items-center gap-3">
        <div className="min-w-[16rem] flex-1">
          <Input
            value={keyword}
            onChange={(e) => setKeyword(e.target.value)}
            placeholder="搜索姓名 / 学号"
            prefix={<Search className="h-5 w-5" aria-hidden />}
          />
        </div>
        <Button
          size="md"
          variant="secondary"
          onClick={() => {
            if (allSelected) {
              onChange(selected.filter((id) => !candidates.some((c) => c.id === id)));
            } else {
              const merged = new Set(selected);
              candidates.forEach((c) => {
                if (max > 0 && merged.size >= max) return;
                merged.add(c.id);
              });
              onChange([...merged]);
            }
          }}
        >
          {allSelected ? '取消全选' : '全选当前'}
        </Button>
        <Button size="md" variant="ghost" onClick={() => onChange([])} disabled={selected.length === 0}>
          清空
        </Button>
        <Badge tone="brand">已选 {selected.length}</Badge>
      </div>

      <ul className="grid max-h-80 grid-cols-1 gap-2 overflow-y-auto pr-1 board:grid-cols-2">
        {candidates.map((s) => {
          const checked = selectedSet.has(s.id);
          const disabled = !checked && max > 0 && selected.length >= max;
          return (
            <li key={s.id}>
              <label
                className={[
                  'flex min-h-touch items-center gap-3 rounded-lg border px-3 transition-colors',
                  checked ? 'border-brand-600 bg-brand-50' : 'border-surface-border bg-surface-raised hover:bg-surface-muted',
                  disabled ? 'opacity-50' : 'cursor-pointer',
                ].join(' ')}
              >
                <input
                  type="checkbox"
                  className="h-6 w-6 accent-brand-600"
                  checked={checked}
                  disabled={disabled}
                  onChange={() => toggle(s.id)}
                />
                <span className="min-w-0 flex-1">
                  <span className="block truncate text-base font-semibold text-ink">{s.name}</span>
                  <span className="block truncate text-sm text-ink-muted">
                    {s.studentNo}
                    {s.seatNo != null && ` · 座位 ${s.seatNo}`}
                  </span>
                </span>
                <StudentStatusBadge status={s.status} size="sm" />
              </label>
            </li>
          );
        })}
        {candidates.length === 0 && (
          <li className="py-6 text-center text-ink-muted">没有匹配的学生</li>
        )}
      </ul>
    </div>
  );
}
