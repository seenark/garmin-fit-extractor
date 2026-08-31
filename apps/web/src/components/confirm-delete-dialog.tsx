interface ConfirmDeleteDialogProps {
  title: string;
  description: string;
  confirmLabel: string;
  busy: boolean;
  onConfirm: () => void;
  onCancel: () => void;
}

export function ConfirmDeleteDialog({
  title,
  description,
  confirmLabel,
  busy,
  onConfirm,
  onCancel,
}: ConfirmDeleteDialogProps) {
  return (
    <div className="dialog-backdrop" data-testid="confirm-delete">
      <section
        className="card dialog-card"
        role="dialog"
        aria-modal="true"
        aria-labelledby="confirm-delete-title"
        aria-describedby="confirm-delete-description"
      >
        <h2 id="confirm-delete-title">{title}</h2>
        <p id="confirm-delete-description">{description}</p>
        <div className="dialog-actions">
          <button
            className="danger"
            type="button"
            disabled={busy}
            aria-busy={busy}
            onClick={onConfirm}
          >
            {busy ? "กำลังลบ…" : confirmLabel}
          </button>
          <button
            className="secondary"
            type="button"
            disabled={busy}
            onClick={onCancel}
          >
            ยกเลิก
          </button>
        </div>
      </section>
    </div>
  );
}
