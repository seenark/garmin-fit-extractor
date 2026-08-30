import { useState } from "react";

import { formatFileSize } from "../lib/formatters";
import { validateFiles } from "../lib/upload-validation";

interface UploadDropzoneProps {
  files: File[];
  disabled: boolean;
  onFilesChange: (files: File[]) => void;
  onSubmit: () => void;
}

export function UploadDropzone({
  files,
  disabled,
  onFilesChange,
  onSubmit,
}: UploadDropzoneProps) {
  const [dragging, setDragging] = useState(false);
  const errors = files.length > 0 ? validateFiles(files) : [];

  function accept(incoming: FileList | File[]) {
    onFilesChange(Array.from(incoming));
  }

  return (
    <section className="card upload-card" aria-labelledby="upload-card-title">
      <div className="section-heading">
        <div>
          <h2 id="upload-card-title">Choose ZIP files</h2>
          <p className="section-note">
            Drop files here, or use Choose files if drag and drop is unavailable.
          </p>
        </div>
        <span className="selection-count" aria-live="polite">
          {files.length === 0
            ? "No files selected"
            : `${files.length} ${files.length === 1 ? "file" : "files"} selected`}
        </span>
      </div>

      <label
        className={`dropzone${dragging ? " dragging" : ""}`}
        data-testid="upload-dropzone"
        aria-disabled={disabled}
        onDragEnter={(event) => {
          event.preventDefault();
          if (!disabled) setDragging(true);
        }}
        onDragOver={(event) => event.preventDefault()}
        onDragLeave={() => setDragging(false)}
        onDrop={(event) => {
          event.preventDefault();
          setDragging(false);
          if (!disabled) accept(event.dataTransfer.files);
        }}
      >
        <input
          className="file-input"
          aria-label="Choose ZIP files"
          type="file"
          accept=".zip,.ZIP"
          multiple
          disabled={disabled}
          onChange={(event) => {
            if (event.target.files) accept(event.target.files);
          }}
        />
        <svg
          className="dropzone-icon"
          viewBox="0 0 32 32"
          fill="none"
          stroke="currentColor"
          strokeLinecap="round"
          strokeLinejoin="round"
          strokeWidth="1.6"
          aria-hidden="true"
        >
          <path d="M16 21V5m0 0-5 5m5-5 5 5" />
          <path d="M7 17v8h18v-8" />
        </svg>
        <span className="dropzone-title">Drop ZIP files here</span>
        <span className="dropzone-copy">or choose them from your computer</span>
        <span className="button secondary" aria-hidden="true">
          Choose files
        </span>
      </label>

      {files.length > 0 ? (
        <ul className="file-list" aria-label="Selected files">
          {files.map((file, index) => (
            <li key={`${file.name}-${file.size}-${index}`} data-testid="selected-file">
              <span className="file-name">
                {file.name} <span className="file-size">{formatFileSize(file.size)}</span>
              </span>
              <button
                className="remove-file"
                type="button"
                aria-label={`Remove ${file.name}`}
                disabled={disabled}
                onClick={() =>
                  onFilesChange(files.filter((_, current) => current !== index))
                }
              >
                Remove
              </button>
            </li>
          ))}
        </ul>
      ) : null}

      {errors.length > 0 ? (
        <div className="inline-error" role="alert">
          <div>
            <strong>Upload needs attention.</strong>
            {errors.map((error) => (
              <div key={error}>{error}</div>
            ))}
          </div>
        </div>
      ) : null}

      <div className="upload-actions">
        <span className="section-note">ZIP files only · 20 megabytes maximum per file</span>
        <button
          type="button"
          data-testid="upload-submit"
          disabled={disabled || files.length === 0 || errors.length > 0}
          aria-busy={disabled}
          onClick={onSubmit}
        >
          {disabled ? "Uploading and extracting…" : "Upload ZIP files"}
        </button>
      </div>
    </section>
  );
}
