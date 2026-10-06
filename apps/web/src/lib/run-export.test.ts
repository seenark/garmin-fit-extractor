import { afterAll, afterEach, expect, test } from "bun:test";
import { ClipboardExportError, createRunExportSession, ExportChangedError } from "./run-export";
import { RunExportTextReadError } from "./runs-api";
import { ApiError } from "./api";

const originalFetch = globalThis.fetch;
const pinnedText = '{\n  "generatedAt": "2026-01-01T00:00:00Z",\n  "mode": "coach",\n  "note": "วิ่ง",\n  "privacy": { "includeLocation": false, "includeDeviceIdentifiers": false },\n  "selection": ["one", "two"],\n  "activities": [{ "id": "one", "revisionId": "revision-1" }, { "id": "two", "revisionId": "revision-2" }],\n  "privacyOmissions": [{ "category": "location", "pathPattern": "activities.*.samples.*.position", "count": 12, "reason": "Location is excluded" }]\n}\n';
const locationText = '{\n  "mode": "coach",\n  "privacy": {"includeLocation": true, "includeDeviceIdentifiers": false},\n  "privacyOmissions": [{"category": "deviceIdentifiers", "count": 2, "pathPattern": "activities.*.sensors.*.serialNumber", "reason": "Device identifiers are excluded"}]\n}\n';
const devicesText = '{\n  "mode": "coach",\n  "privacy": {"includeLocation": false, "includeDeviceIdentifiers": true},\n  "privacyOmissions": [{"category": "location", "count": 12, "pathPattern": "activities.*.samples.*.position", "reason": "Location is excluded"}]\n}\n';
const fullText = '{\n  "mode": "full",\n  "privacyOmissions": []\n}\n';
const texts: Record<string, string> = { pinned: pinnedText, location: locationText, devices: devicesText, full: fullText };
let expired = false;
let byteLengthOverride: number | undefined;
let bodyRequests = 0;
const server = Bun.serve({
  port: 0,
  async fetch(request) {
    const url = new URL(request.url);
    if (request.method === "POST") {
      const body = await request.json() as { activityIds: string[]; mode: string; includeLocation: boolean; includeDeviceIdentifiers: boolean };
      if (body.activityIds.includes("missing")) return Response.json({ error: { code: "ACTIVITY_NOT_READY", message: "Complete selection is not ready" } }, { status: 409 });
      const token = body.mode === "full" ? "full" : body.includeLocation ? "location" : body.includeDeviceIdentifiers ? "devices" : "pinned";
      return Response.json({ token, generatedAt: "2026-01-01T00:00:00Z", expiresAt: "2099-01-01T00:15:00Z", byteLength: byteLengthOverride ?? new TextEncoder().encode(texts[token]).byteLength, downloadUrl: `/api/v2/runs/exports/${token}`, privacyOmissions: JSON.parse(texts[token]).privacyOmissions });
    }
    if (request.method === "GET") bodyRequests++;
    if (expired) return Response.json({ error: { code: "EXPORT_EXPIRED", message: "Snapshot expired" } }, { status: 410 });
    const text = texts[url.pathname.split("/").at(-1)!];
    if (text) return new Response(request.method === "HEAD" ? null : text, { headers: { "Content-Type": "application/json", "Content-Length": String(new TextEncoder().encode(text).byteLength), "Content-Disposition": "attachment; filename=runs-coach.json", "Cache-Control": "private, no-store" } });
    return new Response(null, { status: 404 });
  },
});
function useServer() {
  globalThis.fetch = ((input, init) => originalFetch(new URL(String(input), server.url), init)) as typeof fetch;
}
afterEach(() => { globalThis.fetch = originalFetch; expired = false; byteLengthOverride = undefined; bodyRequests = 0; });
afterAll(() => { server.stop(true); });

test("Copy and Download keep one pinned selection and exact server JSON bytes", async () => {
  useServer();
  const session = createRunExportSession(["one", "two"], false, false);
  const snapshot = await session.prepare("coach");
  const preview = await session.preview("coach");
  expect(preview.omissions).toEqual([{ category: "location", pathPattern: "activities.*.samples.*.position", count: 12, reason: "Location is excluded" }]);
  let copied = "";
  await session.copy("coach", async (text) => { copied = text; });
  expect(copied).toBe(pinnedText);
  expect(await (await originalFetch(new URL(await session.download("coach"), server.url))).text()).toBe(pinnedText);
  expect(await session.prepare("coach")).toEqual(snapshot);
});

test("clipboard denial never reports success and preserves the pinned Download fallback", async () => {
  useServer();
  const session = createRunExportSession(["one", "two"]);
  const snapshot = await session.prepare("coach");
  await expect(session.copy("coach", async () => { throw new DOMException("Denied", "NotAllowedError"); })).rejects.toBeInstanceOf(ClipboardExportError);
  expect(await (await originalFetch(new URL(await session.download("coach"), server.url))).text()).toBe(pinnedText);
  expect((await session.prepare("coach")).generatedAt).toBe(snapshot.generatedAt);
});

test("selection change during preview cannot copy stale selected activity data", async () => {
  useServer();
  const session = createRunExportSession(["one", "two"]);
  await session.prepare("coach");
  let release!: () => void;
  let reached!: () => void;
  const readStarted = new Promise<void>((resolve) => { reached = resolve; });
  const continueRead = new Promise<void>((resolve) => { release = resolve; });
  globalThis.fetch = (async (input, init) => {
    reached();
    await continueRead;
    return originalFetch(new URL(String(input), server.url), init);
  }) as typeof fetch;
  let copied = false;
  const copy = session.copy("coach", async () => { copied = true; });
  await readStarted;
  session.invalidate();
  release();
  await expect(copy).rejects.toBeInstanceOf(ExportChangedError);
  expect(copied).toBe(false);
  useServer();
  const changed = createRunExportSession(["one"], true, false);
  expect((await changed.preview("coach")).omissions).toEqual([{ category: "deviceIdentifiers", count: 2, pathPattern: "activities.*.sensors.*.serialNumber", reason: "Device identifiers are excluded" }]);
});

test("location and device consent are independent and modes keep distinct snapshots", async () => {
  useServer();
  const location = createRunExportSession(["one"], true, false);
  const devices = createRunExportSession(["one"], false, true);
  expect(await (await originalFetch(new URL(await location.download("coach"), server.url))).text()).toBe(locationText);
  expect(await (await originalFetch(new URL(await devices.download("coach"), server.url))).text()).toBe(devicesText);
  expect(await (await originalFetch(new URL(await location.download("full"), server.url))).text()).toBe(fullText);
  expect(await (await originalFetch(new URL(await location.download("coach"), server.url))).text()).toBe(locationText);
});

test("complete-selection failure leaves caller selection untouched and expiry is not a JSON download", async () => {
  useServer();
  const selection = ["one", "missing"];
  const session = createRunExportSession(selection);
  await expect(session.prepare("coach")).rejects.toBeInstanceOf(ApiError);
  expect(selection).toEqual(["one", "missing"]);
  const ready = createRunExportSession(["one"]);
  await ready.prepare("coach");
  expired = true;
  await expect(ready.download("coach")).rejects.toMatchObject({ status: 410 });
});

test("revoked pinned snapshot cannot leak cached preview through Copy or Download", async () => {
  useServer();
  const session = createRunExportSession(["one", "two"]);
  await session.prepare("coach");
  await session.preview("coach");
  expired = true;
  let copied = false;
  await expect(session.copy("coach", async () => { copied = true; })).rejects.toMatchObject({ status: 410, code: "EXPORT_EXPIRED" });
  expect(copied).toBe(false);
  await expect(session.download("coach")).rejects.toMatchObject({ status: 410 });
});

test("clipboard completion after privacy changes cannot report success for old consent", async () => {
  useServer();
  const session = createRunExportSession(["one"]);
  let started!: () => void;
  let finish!: () => void;
  const clipboardStarted = new Promise<void>((resolve) => { started = resolve; });
  const clipboardFinished = new Promise<void>((resolve) => { finish = resolve; });
  const copy = session.copy("coach", async () => { started(); await clipboardFinished; });
  await clipboardStarted;
  session.invalidate();
  finish();
  await expect(copy).rejects.toBeInstanceOf(ExportChangedError);
  const newConsent = createRunExportSession(["one"], false, true);
  expect(await (await originalFetch(new URL(await newConsent.download("coach"), server.url))).text()).toBe(devicesText);
});

test("preview and native download authorize a large pinned snapshot without fetching its JSON body", async () => {
  useServer();
  byteLengthOverride = 825_000_000;
  const session = createRunExportSession(["one"]);
  const snapshot = await session.prepare("full");
  expect(await session.preview("full")).toEqual({ omissions: [] });
  expect(await session.download("full")).toBe(snapshot.downloadUrl);
  expect(bodyRequests).toBe(0);
  expect(snapshot.byteLength).toBe(825_000_000);
});

test("actual native text-capacity failure is distinct from clipboard denial and retains complete Download", async () => {
  useServer();
  const session = createRunExportSession(["one"]);
  const snapshot = await session.prepare("coach");
  globalThis.fetch = (async (input, init) => {
    const response = await originalFetch(new URL(String(input), server.url), init);
    if (init?.method !== "HEAD") response.text = async () => { throw new RangeError("Invalid string length"); };
    return response;
  }) as typeof fetch;
  let copied = false;
  const copying = session.copy("coach", async () => { copied = true; });
  await expect(copying).rejects.toBeInstanceOf(RunExportTextReadError);
  await expect(copying).rejects.toMatchObject({ reason: "capacity", byteLength: snapshot.byteLength });
  expect(copied).toBe(false);
  expect(await session.download("coach")).toBe(snapshot.downloadUrl);
});
