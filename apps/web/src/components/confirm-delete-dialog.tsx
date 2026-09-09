import { useEffect, useState } from "react";

interface ConfirmDeleteDialogProps {
  title: string;
  description: string;
  confirmLabel: string;
  confirmationPhrase?: string;
  confirmationPrompt?: string;
  busy: boolean;
  onConfirm: () => void;
  onCancel: () => void;
}

export function ConfirmDeleteDialog({
  title,
  description,
  confirmLabel,
  confirmationPhrase,
  confirmationPrompt,
  busy,
  onConfirm,
  onCancel,
}: ConfirmDeleteDialogProps) {
  const [confirmation, setConfirmation] = useState("");
  const requiresConfirmation = Boolean(confirmationPhrase);

  useEffect(() => {
    setConfirmation("");
  }, [confirmationPhrase, title]);

  const canConfirm = !confirmationPhrase || confirmation === confirmationPhrase;

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
        {confirmationPhrase ? (
          <label className="dialog-confirmation" htmlFor="delete-confirmation">
            {confirmationPrompt ?? `พิมพ์ ${confirmationPhrase} เพื่อยืนยัน`}
            <input
              id="delete-confirmation"
              autoComplete="off"
              autoFocus
              disabled={busy}
              onChange={(event) => setConfirmation(event.target.value)}
              value={confirmation}
            />
          </label>
        ) : null}
        <div className="dialog-actions">
          <button
            className="danger"
            type="button"
            disabled={busy || (requiresConfirmation && !canConfirm)}
            aria-busy={busy}
            onClick={onConfirm}
          >
            {busy ? "กำลังลบ…" : confirmLabel}
          </button>
          <button
            autoFocus
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
