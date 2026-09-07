import { Modal } from './Modal';
import { Button } from './Button';
import { AlertTriangle } from 'lucide-react';

export interface ConfirmDialogProps {
  open: boolean;
  title: string;
  message: string;
  /** 危险操作的补充说明 */
  detail?: string;
  confirmText?: string;
  cancelText?: string;
  /** 危险操作（红色确认按钮） */
  danger?: boolean;
  loading?: boolean;
  onConfirm: () => void;
  onCancel: () => void;
}

/** 二次确认：删除、覆盖导入等高风险操作 */
export function ConfirmDialog({
  open,
  title,
  message,
  detail,
  confirmText = '确认',
  cancelText = '取消',
  danger = false,
  loading = false,
  onConfirm,
  onCancel,
}: ConfirmDialogProps): JSX.Element {
  return (
    <Modal
      open={open}
      onClose={onCancel}
      title={title}
      widthClass="max-w-xl"
      footer={
        <>
          <Button variant="secondary" onClick={onCancel} disabled={loading}>
            {cancelText}
          </Button>
          <Button variant={danger ? 'danger' : 'primary'} onClick={onConfirm} loading={loading}>
            {confirmText}
          </Button>
        </>
      }
    >
      <div className="flex items-start gap-4">
        <span
          className={[
            'mt-0.5 inline-flex h-12 w-12 shrink-0 items-center justify-center rounded-full',
            danger ? 'bg-red-50 text-red-700' : 'bg-brand-50 text-brand-700',
          ].join(' ')}
        >
          <AlertTriangle className="h-7 w-7" aria-hidden />
        </span>
        <div>
          <p className="text-lg text-ink">{message}</p>
          {detail && <p className="mt-2 text-base text-ink-muted">{detail}</p>}
        </div>
      </div>
    </Modal>
  );
}
