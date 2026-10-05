import { afterEach, expect, mock, spyOn, test } from "bun:test";
import { readRunExport, validateHistorySearch, validateRunExport } from "./runs-api";
afterEach(() => { mock.restore(); });
test("history preserves explicit ascending order and finite offsets", () => {
  expect(validateHistorySearch({ order: "asc", offset: "50" })).toEqual({ order: "asc", offset: 50 });
  expect(validateHistorySearch({ order: "desc", offset: -50 })).toEqual({ order: "desc", offset: 0 });
  expect(validateHistorySearch({ offset: "Infinity" })).toEqual({ order: "desc", offset: 0 });
});
test("export rejects a snapshot URL outside the authenticated endpoint", async () => {
  spyOn(globalThis, "fetch").mockImplementation(Object.assign(async () => new Response("{}"), { preconnect: globalThis.fetch.preconnect }));
  const snapshot = { token: "abc", generatedAt: "", expiresAt: "", byteLength: 2, downloadUrl: "/api/v2/runs/exports/abc", privacyOmissions: [] };
  await expect(readRunExport({ ...snapshot, downloadUrl: "https://attacker.test/steal" })).rejects.toThrow();
  await expect(validateRunExport({ ...snapshot, downloadUrl: "https://attacker.test/steal" })).rejects.toThrow();
});
