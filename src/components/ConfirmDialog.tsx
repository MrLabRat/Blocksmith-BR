import { AlertTriangle, X } from 'lucide-react';

interface ConfirmDialogProps {
  title: string;
  message: string;
  detail?: string;
  confirmLabel?: string;
  onConfirm: () => void;
  onCancel: () => void;
}

export function ConfirmDialog({
  title,
  message,
  detail,
  confirmLabel = 'Confirm',
  onConfirm,
  onCancel,
}: ConfirmDialogProps) {
  return (
    <div className="modal-overlay confirm-dialog-overlay" onClick={onCancel}>
      <div className="modal modal-small confirm-dialog" onClick={(e) => e.stopPropagation()}>
        <div className="modal-header">
          <div className="confirm-dialog-title">
            <AlertTriangle size={18} className="confirm-dialog-icon" />
            <h3>{title}</h3>
          </div>
          <button className="btn btn-icon" onClick={onCancel}>
            <X size={18} />
          </button>
        </div>
        <div className="modal-content confirm-dialog-body">
          <p className="confirm-dialog-message">{message}</p>
          {detail && <p className="confirm-dialog-detail">{detail}</p>}
        </div>
        <div className="modal-actions confirm-dialog-actions">
          <button className="btn" onClick={onCancel}>Cancel</button>
          <button className="btn btn-danger" onClick={onConfirm}>{confirmLabel}</button>
        </div>
      </div>
    </div>
  );
}
