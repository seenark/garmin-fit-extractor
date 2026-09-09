import { afterEach, describe, expect, test } from "bun:test";

import { ApiError } from "../lib/api";
import {
  createTranscriptEntry,
  deleteAllTranscriptEntries,
  deleteTranscriptEntry,
  getTranscriptEntry,
  listTranscriptEntries,
  updateTranscriptEntry,
  type TranscriptEntry,
} from "./transcriptEntries";

const originalFetch = globalThis.fetch;
const entry: TranscriptEntry = {
  id: 56,
  channelName: "Running channel",
  youtubeUrl: "https://youtu.be/video-56",
  videoId: "video-56",
  transcription: "Transcript stays in the request state.",
  createdAt: 1_700_000_000_000,
  updatedAt: 1_700_000_000_000,
};

afterEach(() => {
  globalThis.fetch = originalFetch;
});

describe("transcript entries API client", () => {
  test("lists typed entries with same-origin credentials", async () => {
    const calls: Array<{ input: RequestInfo; init?: RequestInit }> = [];
    globalThis.fetch = (async (input: RequestInfo, init?: RequestInit) => {
      calls.push({ input, init });
      return Response.json({ items: [entry] });
    }) as unknown as typeof fetch;

    await expect(listTranscriptEntries()).resolves.toEqual([entry]);
    expect(calls[0]?.input).toBe("/api/admin/transcript-entries");
    expect(calls[0]?.init).toMatchObject({ credentials: "same-origin" });
  });

  test("gets one entry by its stable id", async () => {
    globalThis.fetch = (async (input: RequestInfo, init?: RequestInit) => {
      expect(input).toBe("/api/admin/transcript-entries/56");
      expect(init).toMatchObject({ credentials: "same-origin" });
      return Response.json(entry);
    }) as unknown as typeof fetch;

    await expect(getTranscriptEntry(entry.id)).resolves.toEqual(entry);
  });

  test("sends create and update fields as JSON without changing transcription", async () => {
    const calls: Array<{ input: RequestInfo; init?: RequestInit }> = [];
    globalThis.fetch = (async (input: RequestInfo, init?: RequestInit) => {
      calls.push({ input, init });
      return Response.json(entry);
    }) as unknown as typeof fetch;
    const input = {
      channelName: entry.channelName,
      youtubeUrl: entry.youtubeUrl,
      videoId: entry.videoId,
      transcription: `  ${entry.transcription}\n`,
    };

    await createTranscriptEntry(input);
    await updateTranscriptEntry(entry.id, input);
    expect(calls).toHaveLength(2);
    expect(calls[0]?.input).toBe("/api/admin/transcript-entries");
    expect(calls[1]?.input).toBe("/api/admin/transcript-entries/56");
    expect(JSON.parse(String(calls[0]?.init?.body))).toEqual(input);
    expect(calls[0]?.init).toMatchObject({
      credentials: "same-origin",
      method: "POST",
      headers: { "Content-Type": "application/json" },
    });
  });

  test("maps duplicate responses to ApiError without exposing database details", async () => {
    globalThis.fetch = (async () =>
      Response.json(
        { error: { code: "TRANSCRIPT_DUPLICATE_VIDEO", message: "duplicate" } },
        { status: 409 },
      )) as unknown as typeof fetch;

    await expect(createTranscriptEntry({
      channelName: entry.channelName,
      youtubeUrl: entry.youtubeUrl,
      videoId: entry.videoId,
      transcription: entry.transcription,
    })).rejects.toMatchObject({
      code: "TRANSCRIPT_DUPLICATE_VIDEO",
      status: 409,
    });
  });

  test("accepts the 204 response used by delete", async () => {
    let request: { input: RequestInfo; init?: RequestInit } | undefined;
    globalThis.fetch = (async (input: RequestInfo, init?: RequestInit) => {
      request = { input, init };
      return new Response(null, { status: 204 });
    }) as unknown as typeof fetch;

    await expect(deleteTranscriptEntry(entry.id)).resolves.toBeUndefined();
    expect(request).toMatchObject({
      input: "/api/admin/transcript-entries/56",
      init: { credentials: "same-origin", method: "DELETE" },
    });
  });

  test("sends the exact confirmation token for deleting all entries", async () => {
    let request: { input: RequestInfo; init?: RequestInit } | undefined;
    globalThis.fetch = (async (input: RequestInfo, init?: RequestInit) => {
      request = { input, init };
      return Response.json({ deleted: 2 });
    }) as unknown as typeof fetch;

    await expect(deleteAllTranscriptEntries()).resolves.toEqual({ deleted: 2 });
    expect(request).toMatchObject({
      input: "/api/admin/transcript-entries",
      init: {
        credentials: "same-origin",
        method: "DELETE",
        headers: { "Content-Type": "application/json" },
      },
    });
    expect(JSON.parse(String(request?.init?.body))).toEqual({ confirmation: "DELETE_ALL" });
  });
});
