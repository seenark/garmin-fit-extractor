import { ApiError } from "./api";
import type { ApiErrorDetail } from "./api-types";
import type { RunDetail, RunExportMode, RunExportSnapshot, RunImportResult, RunLatestThresholds, RunOrder, RunPage, RunTrend } from "./runs-types";
const BASE = "/api/v2/runs";
async function responseOrError(response: Response): Promise<Response> {
  if (!response.ok) {
    const body = await response.json().catch(() => null) as { error?: ApiErrorDetail } | null;
    throw new ApiError(response.status, body?.error ?? { code: "REQUEST_FAILED", message: "The request could not be completed." });
  }
  return response;
}
async function request<T>(path: string, init?: RequestInit): Promise<T> {
  return (await responseOrError(await fetch(path, { ...init, credentials: "same-origin", cache: "no-store" }))).json() as Promise<T>;
}
export function validateHistorySearch(search: Record<string, unknown>): { offset: number; order: RunOrder } {
  const offset = typeof search.offset === "number" ? search.offset : typeof search.offset === "string" ? Number(search.offset) : 0;
  return { offset: Number.isSafeInteger(offset) && offset >= 0 ? offset : 0, order: search.order === "asc" ? "asc" : "desc" };
}
export function listRuns({ limit = 50, offset = 0, order = "desc" }: { limit?: number; offset?: number; order?: RunOrder } = {}): Promise<RunPage> {
  return request(`${BASE}?limit=${limit}&offset=${offset}&sort=startTime&order=${order}`);
}
export function getRun(id: string): Promise<RunDetail> { return request(`${BASE}/${encodeURIComponent(id)}`); }
export function getLatestThresholds(): Promise<RunLatestThresholds> { return request(`${BASE}/thresholds/latest`); }
export function getThresholdTrend(): Promise<RunTrend> { return request(`${BASE}/thresholds/trend`); }
export async function deleteRun(id: string): Promise<void> { await responseOrError(await fetch(`${BASE}/${encodeURIComponent(id)}`, { method: "DELETE", credentials: "same-origin" })); }
export function reprocessRun(id: string): Promise<{ activityId: string; status: "queued" }> { return request(`${BASE}/${encodeURIComponent(id)}/reprocess`, { method: "POST" }); }
export function importRuns(files: File[]): Promise<RunImportResult> {
  const form = new FormData();
  for (const file of files) form.append("files", file, file.name);
  return request(`${BASE}/imports`, { method: "POST", body: form });
}
export function createRunExport(activityIds: string[], mode: RunExportMode, includeLocation = false, includeDeviceIdentifiers = false): Promise<RunExportSnapshot> {
  return request(`${BASE}/exports`, { method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify({ activityIds, mode, includeLocation, includeDeviceIdentifiers }) });
}
export function runExportUrl(snapshot: RunExportSnapshot): string {
  const expected = `${BASE}/exports/${encodeURIComponent(snapshot.token)}`;
  if (snapshot.downloadUrl !== expected) throw new Error("Invalid export download URL");
  return expected;
}
export async function readRunExport(snapshot: RunExportSnapshot): Promise<string> {
  return (await responseOrError(await fetch(runExportUrl(snapshot), { credentials: "same-origin", cache: "no-store" }))).text();
}
export async function downloadRunExport(snapshot: RunExportSnapshot): Promise<Blob> {
  return (await responseOrError(await fetch(runExportUrl(snapshot), { credentials: "same-origin", cache: "no-store" }))).blob();
}
