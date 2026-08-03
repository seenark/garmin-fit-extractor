import { afterEach, describe, expect, test } from "bun:test";

import { ApiError, listExtractions } from "./api";

const originalFetch = globalThis.fetch;

afterEach(() => {
  globalThis.fetch = originalFetch;
});

describe("API client", () => {
  test("uses a same-origin history URL and returns parsed history", async () => {
    const calls: RequestInfo[] = [];
    globalThis.fetch = (async (input: RequestInfo) => {
      calls.push(input);
      return Response.json({ items: [], total: 0, limit: 50, offset: 0 });
    }) as typeof fetch;

    await expect(listExtractions()).resolves.toEqual({ items: [], total: 0, limit: 50, offset: 0 });
    expect(calls).toEqual(["/api/v1/extractions?limit=50&offset=0"]);
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
