import { formatDateTime } from '@shared/lib/format';
import type { SelectOption } from '@shared/components/ui/Select';
import type { CustomTask } from '@shared/types/models';

/**
 * 任务下拉选项：标题 + 备注（截断显示，hover 出完整内容）+ 创建时间。
 *
 * 多处下拉（班级端矩阵页、教务端任务看板）需要同样的信息结构，
 * 集中在这里保证一致，也避免调用方各自拼字符串。
 *
 * 备注为空时不生成 `description`，选项就退回单行，不留空行。
 */
export function taskSelectOptions(tasks: CustomTask[]): SelectOption[] {
  return tasks.map((task) => ({
    value: task.id,
    label: task.title,
    description: task.description?.trim() || undefined,
    meta: formatDateTime(task.createdAt),
  }));
}
