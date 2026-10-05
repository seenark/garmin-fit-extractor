import { afterEach, describe, expect, test } from "bun:test";

import { getCurrentUser } from "./api";

const originalFetch = globalThis.fetch;

afterEach(() => {
  globalThis.fetch = originalFetch;
});

describe("API client", () => {


  test("turns the standard error envelope into ApiError", async () => {
    globalThis.fetch = (async () =>
      Response.json(
        { error: { code: "INVALID_VIEW", message: "view must be normalized or raw." } },
        { status: 400 },
      )) as unknown as typeof fetch;

    await expect(getCurrentUser()).rejects.toMatchObject({
      status: 400,
      code: "INVALID_VIEW",
      message: "view must be normalized or raw.",
    });
  });
});
