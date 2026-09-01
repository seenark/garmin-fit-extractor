import { useMemo, useState } from "react";

import type { Analysis, RawFitRecord } from "../lib/api-types";
import { formatRawJson } from "../lib/raw-json";

type CopyTarget = "normalized" | "raw";
type CopyStatus = CopyTarget | "failed" | "idle";

export function JsonCopyActions({
  normalized,
  raw,
}: {
  normalized: Analysis;
  raw: RawFitRecord[];
}) {
  const normalizedJson = useMemo(
    () => JSON.stringify(normalized, null, 2),
    [normalized],
  );
  const rawJson = useMemo(() => formatRawJson(raw), [raw]);
  const [status, setStatus] = useState<CopyStatus>("idle");

  async function copy(target: CopyTarget, value: string) {
    setStatus("idle");
    try {
      await navigator.clipboard.writeText(value);
      setStatus(target);
    } catch {
      setStatus("failed");
    }
  }

  return (
    <div
      className="json-copy-actions"
      data-testid="json-copy-actions"
      role="group"
      aria-label="คัดลอกข้อมูล JSON"
    >
      <button
        className="secondary"
        data-testid="copy-normalized-json"
        type="button"
        onClick={() => copy("normalized", normalizedJson)}
      >
        {status === "normalized" ? "คัดลอกแล้ว" : "คัดลอก JSON แบบวิเคราะห์"}
      </button>
      <button
        className="secondary"
        data-testid="copy-raw-json"
        type="button"
        onClick={() => copy("raw", rawJson)}
      >
        {status === "raw" ? "คัดลอกแล้ว" : "คัดลอก Raw JSON"}
      </button>
      <span aria-live="polite">
        {status === "normalized" ? "คัดลอก JSON แบบวิเคราะห์แล้ว" : null}
        {status === "raw" ? "คัดลอก Raw JSON แล้ว" : null}
        {status === "failed" ? <span role="alert">คัดลอกไม่สำเร็จ</span> : null}
      </span>
    </div>
  );
}
