import { afterEach, describe, expect, test } from "bun:test";

import { ApiError, getCurrentUser, listExtractions } from "./api";

const originalFetch = globalThis.fetch;

afterEach(() => {
  globalThis.fetch = originalFetch;
});

describe("API client", () => {
  test("uses a same-origin history URL and returns parsed history", async () => {
    const calls: Array<{ input: RequestInfo; init?: RequestInit }> = [];
    globalThis.fetch = (async (input: RequestInfo, init?: RequestInit) => {
      calls.push({ input, init });
      return Response.json({ items: [], total: 0, limit: 50, offset: 0 });
    }) as typeof fetch;

    await expect(listExtractions()).resolves.toEqual({
      items: [],
      total: 0,
      limit: 50,
      offset: 0,
    });
    expect(calls).toHaveLength(1);
    expect(calls[0]?.input).toBe(
      "/api/v1/extractions?limit=50&offset=0&order=desc",
    );
    expect(calls[0]?.init).toMatchObject({ credentials: "same-origin" });
  });

  test("loads the nullable current-user envelope with same-origin credentials", async () => {
    const calls: Array<{ input: RequestInfo; init?: RequestInit }> = [];
    globalThis.fetch = (async (input: RequestInfo, init?: RequestInit) => {
      calls.push({ input, init });
      return Response.json({ user: null });
    }) as typeof fetch;

    await expect(getCurrentUser()).resolves.toEqual({ user: null });
    expect(calls[0]?.input).toBe("/api/v1/auth/me");
    expect(calls[0]?.init).toMatchObject({ credentials: "same-origin" });
  });

  test("turns the standard error envelope into ApiError", async () => {
    globalThis.fetch = (async () =>
      Response.json(
        { error: { code: "INVALID_VIEW", message: "view must be normalized or raw." } },
        { status: 400 },
      )) as unknown as typeof fetch;

    await expect(listExtractions()).rejects.toMatchObject({
      status: 400,
      code: "INVALID_VIEW",
      message: "view must be normalized or raw.",
    });
  });
});
