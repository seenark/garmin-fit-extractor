import { Link, createFileRoute, useRouter } from "@tanstack/react-router";
import { useState } from "react";

import { BatchResults } from "../components/batch-results";
import { UploadDropzone } from "../components/upload-dropzone";
import { ApiError, createExtractions } from "../lib/api";
import type { BatchCreateResponse } from "../lib/api-types";

export const Route = createFileRoute("/upload")({ component: UploadPage });

function UploadPage() {
  const router = useRouter();
  const [files, setFiles] = useState<File[]>([]);
  const [result, setResult] = useState<BatchCreateResponse | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [uploading, setUploading] = useState(false);

  async function submit() {
    setError(null);
    setUploading(true);
    try {
      setResult(await createExtractions(files));
      await router.invalidate();
    } catch (cause) {
      setError(
        cause instanceof ApiError
          ? cause.message
          : "The upload could not be completed.",
      );
    } finally {
      setUploading(false);
    }
  }

  function handleFilesChange(nextFiles: File[]) {
    setFiles(nextFiles);
    setResult(null);
    setError(null);
  }

  return (
    <div className="page-stack">
      <header className="page-intro">
        <div>
          <h1>Upload Garmin ZIP files</h1>
          <p className="page-lede">
            Upload 1–10 ZIP files, up to 20 megabytes each. Extracted FIT files
            are processed and then discarded.
          </p>
        </div>
        <Link
          className="button secondary page-intro-action"
          to="/history"
          search={{ offset: 0, order: "desc" }}
        >
          View history
        </Link>
      </header>

      <div className="workbench-grid">
        <UploadDropzone
          files={files}
          disabled={uploading}
          onFilesChange={handleFilesChange}
          onSubmit={submit}
        />
        <aside className="constraints-panel" aria-labelledby="constraints-title">
          <h2 id="constraints-title">Upload requirements</h2>
          <p>The source ZIP stays private; extracted FIT files are discarded after processing.</p>
          <ul className="constraint-list">
            <li>
              <span className="constraint-label">Accepted files</span>
              <span className="constraint-value">ZIP files</span>
            </li>
            <li>
              <span className="constraint-label">Files per upload</span>
              <span className="constraint-value">1–10 files</span>
            </li>
            <li>
              <span className="constraint-label">Maximum size</span>
              <span className="constraint-value">20 megabytes per file</span>
            </li>
          </ul>
        </aside>
      </div>

      {error ? (
        <div className="error" role="alert">
          <span>{error}</span>
        </div>
      ) : null}
      {result ? <BatchResults result={result} /> : null}
    </div>
  );
}
