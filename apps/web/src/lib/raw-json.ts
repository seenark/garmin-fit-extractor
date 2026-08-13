import type { RawFitRecord } from "./api-types";
export function formatRawJson(records: RawFitRecord[]): string { return JSON.stringify(records, null, 2); }
