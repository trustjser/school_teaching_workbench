import { useEffect, useState } from 'react';
import { Button } from '@/components/ui/Button';
import { Modal } from '@/components/ui/Modal';
import { Select } from '@/components/ui/Select';
import { Textarea } from '@/components/ui/Textarea';
import { CHECKIN_STATUS_META, CHECKIN_STATES_ALL } from '@/constants/status';
import type { CheckinRecord, Student } from '@/types/models';
import type { CheckinState } from '@/types/enums';

export interface CheckinRecordEditorProps {
  open: boolean;
  student: Student;
  record: CheckinRecord | null;
  saving?: boolean;
  onClose: () => void;
  onSave: (state: CheckinState, note: string | null) => Promise<void>;
}

/** 考勤详情编辑：补充迟到状态和备注，时段由系统固定为全天。 */
export function CheckinRecordEditor({
  open,
  student,
  record,
  saving = false,
  onClose,
  onSave,
}: CheckinRecordEditorProps): JSX.Element | null {
  const [state, setState] = useState<CheckinState>('present');
  const [note, setNote] = useState('');

  useEffect(() => {
    if (!open) return;
    setState((record?.state ?? 'present') as CheckinState);
    setNote(record?.note ?? '');
  }, [open, record?.note, record?.state]);

  if (!open) return null;
  return (
    <Modal
      open={open}
      onClose={onClose}
      title={`编辑考勤 · ${student.name}`}
      description="考勤按全天记录，状态和备注会进入同步队列。"
      footer={<><Button variant="secondary" onClick={onClose}>取消</Button><Button loading={saving} onClick={() => void onSave(state, note.trim() || null)}>保存</Button></>}
    >
      <div className="space-y-4">
        <Select
          label="考勤状态"
          options={CHECKIN_STATES_ALL.map((item) => ({ value: item, label: CHECKIN_STATUS_META[item].label }))}
          value={state}
          onChange={(event) => setState(event.target.value as CheckinState)}
        />
        <Textarea label="备注" value={note} onChange={(event) => setNote(event.target.value)} maxLength={500} showCount placeholder="记录请假、迟到或跟进说明" />
      </div>
    </Modal>
  );
}
