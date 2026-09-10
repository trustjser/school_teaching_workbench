import { useEffect, useMemo, useState } from 'react';
import { Button } from '@shared/components/ui/Button';
import { Input } from '@shared/components/ui/Input';
import { Modal } from '@shared/components/ui/Modal';
import { Select } from '@shared/components/ui/Select';
import { Textarea } from '@shared/components/ui/Textarea';
import type { CustomTask, Student, TaskRecord, TaskStatusNode } from '@shared/types/models';

export interface TaskRecordPatch {
  nodeKey: string;
  score?: number | null;
  note?: string | null;
}

export interface TaskRecordEditorProps {
  task: CustomTask;
  student: Student;
  record: TaskRecord | null;
  nodes: TaskStatusNode[];
  open: boolean;
  saving?: boolean;
  onClose: () => void;
  onSave: (patch: TaskRecordPatch) => Promise<void>;
}

/** 学生任务记录编辑面板：状态始终可编辑，评分/备注按任务配置显示。 */
export function TaskRecordEditor({
  task,
  student,
  record,
  nodes,
  open,
  saving = false,
  onClose,
  onSave,
}: TaskRecordEditorProps): JSX.Element | null {
  const defaultNode = useMemo(
    () => nodes.find((node) => node.nodeKey === record?.nodeKey) ?? nodes.find((node) => node.isDefault) ?? nodes[0],
    [nodes, record?.nodeKey],
  );
  const [nodeKey, setNodeKey] = useState('');
  const [score, setScore] = useState('');
  const [note, setNote] = useState('');
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!open) return;
    setNodeKey(defaultNode?.nodeKey ?? 'todo');
    setScore(record?.score == null ? '' : String(record.score));
    setNote(record?.note ?? '');
    setError(null);
  }, [defaultNode?.nodeKey, open, record?.note, record?.score]);

  if (!open) return null;

  const submit = async (): Promise<void> => {
    const parsedScore = score.trim() === '' ? null : Number(score);
    if (task.scoreEnabled && parsedScore != null && (!Number.isInteger(parsedScore) || parsedScore < 0 || parsedScore > 100)) {
      setError('评分必须是 0–100 的整数');
      return;
    }
    if (task.noteEnabled && note.length > 500) {
      setError('备注不能超过 500 字');
      return;
    }
    setError(null);
    await onSave({
      nodeKey,
      ...(task.scoreEnabled ? { score: parsedScore } : {}),
      ...(task.noteEnabled ? { note: note.trim() || null } : {}),
    });
  };

  return (
    <Modal
      open={open}
      onClose={onClose}
      title={`编辑任务 · ${student.name}`}
      description={`${task.title}${student.className ? ` · ${student.className}` : ''}`}
      widthClass="max-w-xl"
      footer={<><Button variant="secondary" onClick={onClose}>取消</Button><Button loading={saving} onClick={() => void submit()}>保存记录</Button></>}
    >
      <div className="space-y-4">
        <Select
          label="处理状态"
          options={nodes.map((node) => ({ value: node.nodeKey, label: node.label }))}
          value={nodeKey}
          onChange={(e) => setNodeKey(e.target.value)}
        />
        {task.scoreEnabled && <Input label="评分" type="number" min={0} max={100} step={1} value={score} onChange={(e) => setScore(e.target.value)} suffix="分" hint="请输入 0–100 的整数" />}
        {task.noteEnabled && <Textarea label="备注" value={note} onChange={(e) => setNote(e.target.value)} showCount maxLength={500} placeholder="记录处理说明或跟进情况" />}
        {error && <p className="rounded-lg bg-red-50 px-3 py-2 text-sm font-semibold text-red-700">{error}</p>}
      </div>
    </Modal>
  );
}

