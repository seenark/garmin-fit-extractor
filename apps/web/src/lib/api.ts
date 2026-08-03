import type {
  ApiErrorDetail,
  BatchCreateResponse,
  ExtractionDetail,
  ExtractionPage,
} from "./api-types";

const API_BASE = "/api/v1/extractions";

export class ApiError extends Error {
  readonly status: number;
  readonly code: string;
  readonly fileName?: string;

  constructor(status: number, detail: ApiErrorDetail) {
    super(detail.message);
    this.name = "ApiError";
    this.status = status;
    this.code = detail.code;
    this.fileName = detail.fileName;
  }
}

async function parseResponse<T>(response: Response): Promise<T> {
  if (!response.ok) {
    const body = (await response.json().catch(() => null)) as { error?: ApiErrorDetail } | null;
    throw new ApiError(
      response.status,
      body?.error ?? { code: "REQUEST_FAILED", message: "The request could not be completed." },
    );
  }
  return response.json() as Promise<T>;
}

async function request<T>(path: string, init?: RequestInit): Promise<T> {
  return parseResponse<T>(await fetch(path, init));
}

export async function createExtractions(files: File[]): Promise<BatchCreateResponse> {
  const form = new FormData();
  for (const file of files) form.append("files", file, file.name);
  return request<BatchCreateResponse>(API_BASE, { method: "POST", body: form });
}

export async function listExtractions(
  params: { limit?: number; offset?: number } = {},
): Promise<ExtractionPage> {
  const limit = params.limit ?? 50;
  const offset = params.offset ?? 0;
  return request<ExtractionPage>(`${API_BASE}?limit=${limit}&offset=${offset}`);
}

export function getExtraction(id: string): Promise<ExtractionDetail> {
  return request<ExtractionDetail>(`${API_BASE}/${encodeURIComponent(id)}`);
}

export async function deleteExtraction(id: string): Promise<void> {
  const response = await fetch(`${API_BASE}/${encodeURIComponent(id)}`, { method: "DELETE" });
  if (!response.ok) await parseResponse<never>(response);
}

export async function clearExtractions(): Promise<void> {
  const response = await fetch(API_BASE, { method: "DELETE" });
  if (!response.ok) await parseResponse<never>(response);
}

export async function downloadExtraction(id: string, view: "normalized" | "raw"): Promise<Blob> {
  const response = await fetch(`${API_BASE}/${encodeURIComponent(id)}/download?view=${view}`);
  if (!response.ok) await parseResponse<never>(response);
  return response.blob();
}
