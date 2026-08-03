import { createFileRoute, useRouter } from "@tanstack/react-router";
import { useState } from "react";

import { BatchResults } from "../components/batch-results";
import { UploadDropzone } from "../components/upload-dropzone";
import { ApiError, createExtractions } from "../lib/api";
import type { BatchCreateResponse } from "../lib/api-types";

export const Route = createFileRoute("/")({ component: UploadPage });

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
      setError(cause instanceof ApiError ? cause.message : "The upload could not be completed.");
    } finally {
      setUploading(false);
    }
  }

  return <>
    <UploadDropzone files={files} disabled={uploading} onFilesChange={(nextFiles) => { setFiles(nextFiles); setResult(null); setError(null); }} onSubmit={submit} />
    {error ? <div className="error" role="alert">{error}</div> : null}
    {result ? <BatchResults result={result} /> : null}
  </>;
}
