import { ApiError } from "../lib/api";
import type { ApiErrorDetail } from "../lib/api-types";

const API_PATH = "/api/admin/transcript-entries";

export type TranscriptEntry = {
  id: number;
  channelName: string;
  youtubeUrl: string;
  videoId: string;
  transcription: string;
  createdAt: number;
  updatedAt: number;
};

export type TranscriptEntryInput = Pick<
  TranscriptEntry,
  "channelName" | "youtubeUrl" | "transcription"
> &
  Partial<Pick<TranscriptEntry, "videoId">>;

export type TranscriptEntryDeleteAllResponse = {
  deleted: number;
};

type ErrorEnvelope = { error?: Partial<ApiErrorDetail> };
type TranscriptListResponse = { items: TranscriptEntry[] };

async function request<T>(path: string, init?: RequestInit): Promise<T> {
  const response = await fetch(path, {
    ...init,
    credentials: "same-origin",
    headers: init?.body
      ? { "Content-Type": "application/json", ...init.headers }
      : init?.headers,
  });

  if (!response.ok) {
    const body = (await response.json().catch(() => null)) as ErrorEnvelope | null;
    const detail = body?.error;
    throw new ApiError(response.status, {
      code: typeof detail?.code === "string" ? detail.code : "REQUEST_FAILED",
      message:
        typeof detail?.message === "string"
          ? detail.message
          : "The request could not be completed.",
    });
  }

  if (response.status === 204) return undefined as T;
  return (await response.json()) as T;
}

export async function listTranscriptEntries(): Promise<TranscriptEntry[]> {
  const response = await request<TranscriptListResponse>(API_PATH);
  return response.items;
}

export function getTranscriptEntry(id: number): Promise<TranscriptEntry> {
  return request<TranscriptEntry>(`${API_PATH}/${encodeURIComponent(String(id))}`);
}

export function createTranscriptEntry(input: TranscriptEntryInput): Promise<TranscriptEntry> {
  return request<TranscriptEntry>(API_PATH, {
    body: JSON.stringify(input),
    method: "POST",
  });
}

export function updateTranscriptEntry(
  id: number,
  input: TranscriptEntryInput,
): Promise<TranscriptEntry> {
  return request<TranscriptEntry>(`${API_PATH}/${encodeURIComponent(String(id))}`, {
    body: JSON.stringify(input),
    method: "PUT",
  });
}

export function deleteTranscriptEntry(id: number): Promise<void> {
  return request<void>(`${API_PATH}/${encodeURIComponent(String(id))}`, { method: "DELETE" });
}

export function deleteAllTranscriptEntries(): Promise<TranscriptEntryDeleteAllResponse> {
  return request<TranscriptEntryDeleteAllResponse>(API_PATH, {
    body: JSON.stringify({ confirmation: "DELETE_ALL" }),
    method: "DELETE",
  });
}
