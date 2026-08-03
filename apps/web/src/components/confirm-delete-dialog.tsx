interface ConfirmDeleteDialogProps {
  title: string;
  description: string;
  confirmLabel: string;
  busy: boolean;
  onConfirm: () => void;
  onCancel: () => void;
}

export function ConfirmDeleteDialog({ title, description, confirmLabel, busy, onConfirm, onCancel }: ConfirmDeleteDialogProps) {
  return (
    <div className="card" data-testid="confirm-delete" role="dialog" aria-modal="true" aria-labelledby="confirm-delete-title">
      <h2 id="confirm-delete-title">{title}</h2>
      <p>{description}</p>
      <div className="actions">
        <button type="button" className="danger" disabled={busy} onClick={onConfirm}>{busy ? "Deleting…" : confirmLabel}</button>
        <button type="button" className="secondary" disabled={busy} onClick={onCancel}>Cancel</button>
      </div>
    </div>
  );
}
