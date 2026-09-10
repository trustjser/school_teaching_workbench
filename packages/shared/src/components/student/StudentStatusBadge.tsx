import { Badge, type BadgeTone } from '@shared/components/ui/Badge';
import { STUDENT_STATUS_META } from '@shared/constants/status';
import type { StudentStatus } from '@shared/types/enums';

const TONE: Record<StudentStatus, BadgeTone> = {
  active: 'success',
  leave: 'warning',
  transferred: 'neutral',
};

export interface StudentStatusBadgeProps {
  status: StudentStatus;
  size?: 'sm' | 'md' | 'lg';
}

/** 学生状态徽标：在读 / 请假 / 已转出（颜色 + emoji + 文字三重编码） */
export function StudentStatusBadge({
  status,
  size = 'md',
}: StudentStatusBadgeProps): JSX.Element {
  const meta = STUDENT_STATUS_META[status];
  return (
    <Badge tone={TONE[status]} emoji={meta.emoji} size={size}>
      {meta.label}
    </Badge>
  );
}
