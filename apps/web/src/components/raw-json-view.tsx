import { useMemo } from "react";

import type { RawFitRecord } from "../lib/api-types";

export function RawJsonView({ records }: { records: RawFitRecord[] }) {
  const formatted = useMemo(() => JSON.stringify(records, null, 2), [records]);
  return <pre data-testid="raw-json-view" aria-label="Raw decoded FIT JSON">{formatted}</pre>;
}
