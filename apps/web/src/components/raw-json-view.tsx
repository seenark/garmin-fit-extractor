import { useMemo, useState } from "react";

import type { RawFitRecord } from "../lib/api-types";
import { formatRawJson } from "../lib/raw-json";

export function RawJsonView({ records }: { records: RawFitRecord[] }) {
  const formatted = useMemo(() => formatRawJson(records), [records]);
  const [status, setStatus] = useState<"idle" | "copied" | "failed">("idle");

  return (
    <>
      <div className="actions">
        <button
          type="button"
          onClick={async () => {
            try {
              await navigator.clipboard.writeText(formatted);
              setStatus("copied");
            } catch {
              setStatus("failed");
            }
          }}
        >
          {status === "copied" ? "คัดลอกแล้ว" : "คัดลอก Raw JSON"}
        </button>
        <span aria-live="polite">
          {status === "copied" ? "คัดลอกแล้ว" : null}
          {status === "failed" ? <span role="alert">คัดลอกไม่สำเร็จ</span> : null}
        </span>
      </div>
      <pre data-testid="raw-json-view" aria-label="JSON ของข้อมูล FIT ที่ถอดรหัสแล้ว">
        {formatted}
      </pre>
    </>
  );
}
