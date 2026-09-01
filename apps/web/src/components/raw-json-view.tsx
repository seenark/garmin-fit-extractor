import { useMemo } from "react";

import type { RawFitRecord } from "../lib/api-types";
import { formatRawJson } from "../lib/raw-json";

export function RawJsonView({ records }: { records: RawFitRecord[] }) {
  const formatted = useMemo(() => formatRawJson(records), [records]);

  return (
    <pre data-testid="raw-json-view" aria-label="JSON ของข้อมูล FIT ที่ถอดรหัสแล้ว">
      {formatted}
    </pre>
  );
}
