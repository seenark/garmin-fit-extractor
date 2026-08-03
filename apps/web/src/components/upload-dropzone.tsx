import { useRef, useState } from "react";

import { validateFiles } from "../lib/upload-validation";

interface UploadDropzoneProps {
  files: File[];
  disabled: boolean;
  onFilesChange: (files: File[]) => void;
  onSubmit: () => void;
}

function formatBytes(bytes: number): string {
  return `${(bytes / (1024 * 1024)).toFixed(bytes >= 1024 * 1024 ? 1 : 2)} MiB`;
}

export function UploadDropzone({ files, disabled, onFilesChange, onSubmit }: UploadDropzoneProps) {
  const input = useRef<HTMLInputElement>(null);
  const [dragging, setDragging] = useState(false);
  const errors = validateFiles(files);

  const accept = (incoming: FileList | File[]) => onFilesChange(Array.from(incoming));

  return (
    <section className="card" aria-labelledby="upload-title">
      <h1 id="upload-title">Upload Garmin FIT files</h1>
      <p className="muted">Upload 1–10 files, up to 20 MiB each. FIT bytes are processed but never retained.</p>
      <div
        className={`dropzone${dragging ? " dragging" : ""}`}
        data-testid="upload-dropzone"
        onDragEnter={(event) => {
          event.preventDefault();
          setDragging(true);
        }}
        onDragOver={(event) => event.preventDefault()}
        onDragLeave={() => setDragging(false)}
        onDrop={(event) => {
          event.preventDefault();
          setDragging(false);
          accept(event.dataTransfer.files);
        }}
      >
        <input
          ref={input}
          aria-label="Choose FIT files"
          type="file"
          accept=".fit,.FIT"
          multiple
          hidden
          onChange={(event) => event.target.files && accept(event.target.files)}
        />
        <p>Drop FIT files here, or choose them from your computer.</p>
        <button type="button" className="secondary" onClick={() => input.current?.click()} disabled={disabled}>
          Choose files
        </button>
      </div>
      {files.length > 0 ? (
        <ul className="file-list" aria-label="Selected files">
          {files.map((file, index) => (
            <li key={`${file.name}-${file.size}-${index}`} data-testid="selected-file">
              <span>{file.name} <span className="muted">({formatBytes(file.size)})</span></span>
              <button type="button" aria-label={`Remove ${file.name}`} disabled={disabled} onClick={() => onFilesChange(files.filter((_, current) => current !== index))}>Remove</button>
            </li>
          ))}
        </ul>
      ) : null}
      {errors.length > 0 ? <div className="error" role="alert">{errors.map((error) => <div key={error}>{error}</div>)}</div> : null}
      <div className="actions">
        <button type="button" data-testid="upload-submit" disabled={disabled || errors.length > 0} onClick={onSubmit}>
          {disabled ? "Uploading and extracting…" : "Upload FIT files"}
        </button>
        {disabled ? <span className="muted">Uploading and extracting…</span> : null}
      </div>
    </section>
  );
}
