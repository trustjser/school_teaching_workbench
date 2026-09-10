import { Building2, GraduationCap } from 'lucide-react';
import type { AppMode } from '@shared/types/enums';

export interface ModeBadgeProps {
  mode: AppMode;
  size?: 'sm' | 'md' | 'lg';
  /** 附带模式说明文案 */
  withLabel?: boolean;
}

/** 模式徽标：班级端 / 教务处端 视觉强区分（颜色 + 图标 + 文字） */
export function ModeBadge({ mode, size = 'md', withLabel = true }: ModeBadgeProps): JSX.Element {
  const isMaster = mode === 'master';
  const sizeClass =
    size === 'lg' ? 'px-4 py-2 text-lg gap-2' : size === 'sm' ? 'px-2 py-0.5 text-sm gap-1' : 'px-3 py-1 text-base gap-1.5';
  const iconSize = size === 'lg' ? 26 : size === 'sm' ? 16 : 20;
  return (
    <span
      className={[
        'inline-flex items-center rounded-full border-2 font-bold whitespace-nowrap',
        sizeClass,
        isMaster
          ? 'border-violet-700 bg-violet-100 text-violet-900'
          : 'border-brand-700 bg-brand-100 text-brand-900',
      ].join(' ')}
      title={isMaster ? '教务处端：可监控全校节点、下发任务、导出统计' : '班级端：名册、考勤、任务矩阵'}
    >
      {isMaster ? (
        <Building2 width={iconSize} height={iconSize} aria-hidden />
      ) : (
        <GraduationCap width={iconSize} height={iconSize} aria-hidden />
      )}
      {withLabel && <span>{isMaster ? '教务处端' : '班级端'}</span>}
    </span>
  );
}
