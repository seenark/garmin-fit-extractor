import { expect, test as base, type Page } from "@playwright/test";
import { readFile, writeFile } from "node:fs/promises";
import { createServer, request as httpRequest } from "node:http";

const firstId = "10000000-0000-4000-8000-000000000001";
const secondId = "10000000-0000-4000-8000-000000000051";
const metrics = {
  distanceMeters: 5000, timerTimeSeconds: 1500, elapsedTimeSeconds: 1510,
  movingTimeSeconds: null, averageSpeedMps: 10 / 3, averagePaceSecondsPerKm: 300,
  averageHeartRateBpm: 150, averagePowerWatts: null, averageCadenceStepsPerMinute: 170,
};

interface ExportTransport {
  url: string;
  documents: Map<string, string>;
  revoked: Set<string>;
  failedGet: Map<string, "http" | "midstream">;
  requests: Array<{ method: string; token: string }>;
  streamedBytes: number;
}
const test = base.extend<{ exportTransport: ExportTransport }>({
  exportTransport: async ({ baseURL }, use) => {
    const transport: ExportTransport = {
      url: "", documents: new Map(), revoked: new Set(), failedGet: new Map(), requests: [], streamedBytes: 0,
    };
    const server = createServer((request, response) => {
      const token = /^\/api\/v2\/runs\/exports\/([^/?]+)$/.exec(request.url ?? "")?.[1];
      if (!token) {
        const upstream = httpRequest(new URL(request.url!, baseURL!), { method: request.method, headers: request.headers }, result => {
          response.writeHead(result.statusCode!, result.headers);
          result.pipe(response);
        });
        upstream.on("error", error => response.destroy(error));
        request.pipe(upstream);
        return;
      }
      transport.requests.push({ method: request.method!, token });
      const text = transport.documents.get(token);
      const failure = transport.failedGet.get(token);
      if (transport.revoked.has(token) || text === undefined || (request.method === "GET" && failure === "http")) {
        response.writeHead(transport.revoked.has(token) ? 410 : 404, { "Content-Type": "application/json" });
        response.end(request.method === "HEAD" ? undefined : JSON.stringify({ error: { code: "EXPORT_NOT_FOUND", message: "Snapshot unavailable." } }));
        return;
      }
      const bytes = Buffer.from(text);
      response.writeHead(200, {
        "Content-Type": "application/json",
        "Content-Length": bytes.byteLength,
        "Content-Disposition": 'attachment; filename="runs-fixture.json"',
        "Cache-Control": "private, no-store",
      });
      if (request.method === "HEAD") response.end();
      else if (failure === "midstream") {
        const partial = bytes.subarray(0, 100);
        response.write(partial, () => { transport.streamedBytes += partial.byteLength; response.destroy(); });
      } else response.end(bytes);
    });
    server.on("upgrade", (_request, socket) => socket.destroy());
    await new Promise<void>(resolve => server.listen(0, "127.0.0.1", resolve));
    const address = server.address();
    if (!address || typeof address === "string") throw new Error("Export fixture server did not expose its port");
    transport.url = `http://127.0.0.1:${address.port}`;
    try { await use(transport); }
    finally {
      const closed = new Promise<void>((resolve, reject) => server.close(error => error ? reject(error) : resolve()));
      server.closeAllConnections();
      await closed;
    }
  },
});
const rows = Array.from({ length: 51 }, (_, index) => ({
  id: `10000000-0000-4000-8000-${String(index + 1).padStart(12, "0")}`,
  startTime: new Date(Date.UTC(2026, 8, 1, 0, index * 30)).toISOString(),
  endTime: new Date(Date.UTC(2026, 8, 1, 0, index * 30 + 25)).toISOString(),
  summary: metrics,
  processing: { status: "ready", stale: false, updateFailed: false, errorCode: null },
  sourceUnavailable: false, revisionId: `revision-${index + 1}`, possibleDuplicate: false,
}));

async function authenticate(page: Page) {
  await page.route("**/api/v1/auth/me", route => route.fulfill({ json: {
    user: { id: "runner", email: "runner@example.test", displayName: "Runner" }, isAdmin: false,
  } }));
  await page.route("**/api/v2/runs/thresholds/latest", route => route.fulfill({ json: {
    evidenceCutoff: null, latestAttempt: null, lastAvailable: { lt1: null, lt2: null },
    stale: false, engineStatus: "unavailable",
  } }));
  await page.route("**/api/v2/runs/thresholds/trend", route => route.fulfill({ json: { items: [] } }));
}

async function history(page: Page) {
  await page.route("**/api/v2/runs?*", route => {
    const url = new URL(route.request().url());
    const offset = Number(url.searchParams.get("offset"));
    return route.fulfill({ json: { items: rows.slice(offset, offset + 50), total: 51, limit: 50, offset } });
  });
}

test("pending ascending history keeps current URL order when opening an existing detail link", async ({ page }) => {
  await authenticate(page);
  let releaseAscending!: () => void;
  const ascendingGate = new Promise<void>(resolve => { releaseAscending = resolve; });
  let ascendingRequests = 0;
  await page.route("**/api/v2/runs?*", async route => {
    const url = new URL(route.request().url());
    if (url.searchParams.get("order") === "asc" && ascendingRequests++ === 0) await ascendingGate;
    await route.fulfill({ json: { items: [rows[0]], total: 1, limit: 50, offset: 0 } });
  });
  await page.route(`**/api/v2/runs/${firstId}`, route => route.fulfill({ json: {
    ...rows[0], sourceUnavailable: true, revisionId: null,
    normalized: null, analysis: null, historicalThresholds: null,
    fidelityWarnings: ["LEGACY_SOURCE_UNAVAILABLE"],
  } }));
  try {
    await page.goto("/history?offset=0&order=desc");
    await expect(page.getByTestId("history-table")).toBeVisible();
    await page.getByLabel("เรียงลำดับ").selectOption("asc");
    await expect(page).toHaveURL(/\/history\?.*order=asc/);
    // Ascending data stays blocked. Do not wait for link rewriting: click the still-rendered row.
    await page.getByTestId("history-table").getByRole("link", { name: "เปิดดู" }).click();
    await expect(page).toHaveURL(new RegExp(`/extractions/${firstId}\\?.*order=asc`));
    await page.getByRole("link", { name: "กลับไปประวัติ", exact: true }).click();
    await expect(page).toHaveURL(/\/history\?.*order=asc/);
    await expect(page.getByLabel("เรียงลำดับ")).toHaveValue("asc");
  } finally {
    releaseAscending();
  }
});

const generatedAt = "2026-10-05T12:00:00Z";
const coachOmissions = [{ category: "location", pathPattern: "activities.*.samples.*.positionLat", count: 2, reason: "Location excluded by policy." }];
const coachBytes = JSON.stringify({
  schemaVersion: "2.0.0", generatedAt, mode: "coach",
  privacy: { includeLocation: false, includeDeviceIdentifiers: false },
  selection: [firstId, secondId],
  activities: [firstId, secondId].map(id => ({
    id, startTime: "2026-09-01T00:00:00Z", endTime: "2026-09-01T00:25:00Z",
    subtype: null, summary: metrics, laps: [], segments: [], samples: [],
    quality: { warnings: ["SAMPLES_UNAVAILABLE"] }, thresholds: null,
    historicalThresholds: null, transformations: [],
  })),
  privacyOmissions: coachOmissions,
  transformations: [],
}, null, 2) + "\n";
const fullBytes = JSON.stringify({
  schemaVersion: "2.0.0", generatedAt, mode: "full",
  privacy: { includeLocation: false, includeDeviceIdentifiers: false },
  selection: [firstId, secondId],
  activities: [firstId, secondId].map(id => ({
    id,
    decoded: { schemaVersion: "2.0.0", decoder: { name: "FIT" }, messages: [], warnings: [] },
    normalized: {
      schemaVersion: "2.0.0", session: { index: 0, sourceReferences: [] },
      startTime: "2026-09-01T00:00:00Z", endTime: "2026-09-01T00:25:00Z",
      sport: "running", subtype: null, summary: metrics, samples: [], laps: [],
      timerEvents: [], sensors: [], zones: [], deviceReportedThresholds: [],
      extensions: [], rr: { intervals: [], alignmentEligible: false, reasons: [] }, warnings: [],
    },
    analysis: { schemaVersion: "2.0.0", quality: {}, segments: [], thresholds: null, transformations: [] },
    historicalThresholds: null,
  })),
  privacyOmissions: [], transformations: [],
}, null, 2) + "\n";

function snapshot(token: string, bytes = coachBytes) {
  return {
    token, generatedAt, expiresAt: new Date(Date.now() + 15 * 60_000).toISOString(),
    byteLength: Buffer.byteLength(bytes), downloadUrl: `/api/v2/runs/exports/${token}`,
    privacyOmissions: bytes === coachBytes ? coachOmissions : [],
  };
}


test("selection survives pages and copy plus download preserve pinned Coach and Full bytes", async ({ page, exportTransport }) => {
  await authenticate(page);
  await history(page);
  const exports: unknown[] = [];
  await page.route("**/api/v2/runs/exports", route => {
    const body = route.request().postDataJSON();
    exports.push(body);
    exportTransport.documents.set(body.mode, body.mode === "coach" ? coachBytes : fullBytes);
    return route.fulfill({ json: snapshot(body.mode, body.mode === "coach" ? coachBytes : fullBytes) });
  });
  await page.goto(`${exportTransport.url}/history`);
  await expect(page.getByRole("checkbox", { name: `เลือกกิจกรรม ${firstId}` })).not.toBeChecked();
  await page.getByRole("checkbox", { name: `เลือกกิจกรรม ${firstId}` }).check();
  await page.getByRole("button", { name: "ถัดไป", exact: true }).click();
  await page.getByRole("checkbox", { name: `เลือกกิจกรรม ${secondId}` }).check();
  await page.getByRole("button", { name: "ก่อนหน้า", exact: true }).click();
  await expect(page.getByRole("checkbox", { name: `เลือกกิจกรรม ${firstId}` })).toBeChecked();
  const actions = page.getByRole("region", { name: "ส่งออกกิจกรรมที่เลือก", exact: true });
  await expect(actions).toContainText("เลือกไว้ 2 กิจกรรม");
  for (const mode of ["Coach", "Full"] as const) {
    const section = actions.getByRole("region", { name: `ส่งออก ${mode} JSON`, exact: true });
    await section.getByRole("button", { name: `เตรียม ${mode} JSON`, exact: true }).click();
    await expect(section.getByRole("button", { name: `Copy ${mode} JSON` })).toBeDisabled();
    expect(exportTransport.requests.filter(request => request.token === mode.toLowerCase() && request.method === "GET")).toHaveLength(0);
    if (mode === "Coach") await expect(section).toContainText("activities.*.samples.*.positionLat");
    // Full uses metadata-only download; Coach downloads after preview/copy.
    const bytes = mode === "Coach" ? coachBytes : fullBytes;
    if (mode === "Full") {
      const download = page.waitForEvent("download");
      await section.getByRole("button", { name: `Download ${mode} JSON` }).click();
      const path = await (await download).path();
      expect(await readFile(path!)).toEqual(Buffer.from(bytes));
    }
    await section.getByRole("button", { name: "ตรวจ snapshot ก่อน Copy" }).click();
    expect(exportTransport.requests.filter(request => request.token === mode.toLowerCase() && request.method === "GET")).toHaveLength(mode === "Full" ? 1 : 0);
    await section.getByRole("button", { name: `Copy ${mode} JSON` }).click();
    await expect.poll(() => page.evaluate(() => navigator.clipboard.readText())).toBe(bytes);
    const download = page.waitForEvent("download");
    await section.getByRole("button", { name: `Download ${mode} JSON` }).click();
    const path = await (await download).path();
    expect(await readFile(path!)).toEqual(Buffer.from(bytes));
  }
  expect(exports).toEqual([
    { activityIds: [firstId, secondId], mode: "coach", includeLocation: false, includeDeviceIdentifiers: false },
    { activityIds: [firstId, secondId], mode: "full", includeLocation: false, includeDeviceIdentifiers: false },
  ]);
  expect(exportTransport.requests.filter(request => request.method === "GET")).toEqual([
    { method: "GET", token: "coach" }, { method: "GET", token: "coach" },
    { method: "GET", token: "full" }, { method: "GET", token: "full" }, { method: "GET", token: "full" },
  ]);
});

test("failed complete-selection exports never remove selected activities or export a partial subset", async ({ page }) => {
  await authenticate(page);
  let removed = false;
  await page.route("**/api/v2/runs?*", route => {
    const offset = Number(new URL(route.request().url()).searchParams.get("offset"));
    return route.fulfill({ json: { items: rows.slice(offset, offset + 50).filter(row => !removed || row.id !== firstId), total: removed ? 50 : 51, limit: 50, offset } });
  });
  await page.route(`**/api/v2/runs/${firstId}`, route => { removed = true; return route.fulfill({ status: 204 }); });
  const bodies: unknown[] = [];
  const errors = [
    { status: 404, code: "RUN_NOT_FOUND", message: "Selected activity no longer exists." },
    { status: 409, code: "RUN_NOT_READY", message: "Selected activity is not ready." },
    { status: 422, code: "LEGACY_EXPORT_UNSUPPORTED", message: "Original source unavailable." },
  ];
  await page.route("**/api/v2/runs/exports", route => {
    bodies.push(route.request().postDataJSON());
    const error = errors[bodies.length - 1]!;
    return route.fulfill({ status: error.status, json: { error: { code: error.code, message: error.message } } });
  });
  await page.goto("/history");
  await page.getByRole("checkbox", { name: `เลือกกิจกรรม ${firstId}` }).check();
  await page.getByRole("button", { name: "ถัดไป", exact: true }).click();
  await page.getByRole("checkbox", { name: `เลือกกิจกรรม ${secondId}` }).check();
  await page.getByRole("button", { name: "ก่อนหน้า", exact: true }).click();
  await page.getByRole("button", { name: `ลบกิจกรรม ${firstId}`, exact: true }).click();
  await page.getByTestId("confirm-delete").getByRole("button", { name: "ลบรายการ", exact: true }).click();
  await expect(page.getByRole("checkbox", { name: `เลือกกิจกรรม ${firstId}` })).toHaveCount(0);
  const actions = page.getByRole("region", { name: "ส่งออกกิจกรรมที่เลือก", exact: true });
  const coach = actions.getByRole("region", { name: "ส่งออก Coach JSON", exact: true });
  for (const error of errors) {
    await coach.getByRole("button", { name: "เตรียม Coach JSON", exact: true }).click();
    await expect(coach.getByRole("alert")).toContainText(error.code);
    await expect(actions).toContainText("เลือกไว้ 2 กิจกรรม");
    await expect(coach.getByRole("button", { name: "Copy Coach JSON", exact: true })).toHaveCount(0);
    await expect(coach.getByRole("button", { name: "Download Coach JSON", exact: true })).toHaveCount(0);
  }
  expect(bodies).toEqual(errors.map(() => ({
    activityIds: [firstId, secondId], mode: "coach", includeLocation: false, includeDeviceIdentifiers: false,
  })));
});

test("privacy switches stay independent and changing either invalidates the previous snapshot", async ({ page }) => {
  await authenticate(page);
  await history(page);
  const policies: unknown[] = [];
  await page.route("**/api/v2/runs/exports", route => {
    policies.push(route.request().postDataJSON());
    return route.fulfill({ json: snapshot(`privacy-${policies.length}`) });
  });
  await page.goto("/history");
  await page.getByRole("checkbox", { name: `เลือกกิจกรรม ${firstId}` }).check();
  const actions = page.getByRole("region", { name: "ส่งออกกิจกรรมที่เลือก", exact: true });
  const coach = actions.getByRole("region", { name: "ส่งออก Coach JSON", exact: true });
  await coach.getByRole("button", { name: "เตรียม Coach JSON", exact: true }).click();
  await expect(coach.getByRole("button", { name: "Download Coach JSON" })).toBeEnabled();
  await actions.getByRole("checkbox", { name: "รวมข้อมูลตำแหน่ง", exact: true }).check();
  await expect(coach.getByRole("button", { name: "Download Coach JSON" })).toHaveCount(0);
  await expect(actions.getByRole("checkbox", { name: "รวมตัวระบุอุปกรณ์" })).not.toBeChecked();
  await coach.getByRole("button", { name: "เตรียม Coach JSON", exact: true }).click();
  await expect(coach.getByRole("button", { name: "Download Coach JSON" })).toBeEnabled();
  await actions.getByRole("checkbox", { name: "รวมข้อมูลตำแหน่ง", exact: true }).uncheck();
  await actions.getByRole("checkbox", { name: "รวมตัวระบุอุปกรณ์", exact: true }).check();
  await expect(actions.getByRole("checkbox", { name: "รวมข้อมูลตำแหน่ง" })).not.toBeChecked();
  await expect(coach.getByRole("button", { name: "Download Coach JSON" })).toHaveCount(0);
  await coach.getByRole("button", { name: "เตรียม Coach JSON", exact: true }).click();
  await expect(coach.getByRole("button", { name: "Download Coach JSON" })).toBeEnabled();
  expect(policies).toEqual([
    { activityIds: [firstId], mode: "coach", includeLocation: false, includeDeviceIdentifiers: false },
    { activityIds: [firstId], mode: "coach", includeLocation: true, includeDeviceIdentifiers: false },
    { activityIds: [firstId], mode: "coach", includeLocation: false, includeDeviceIdentifiers: true },
  ]);
});

test("clipboard denial preserves snapshot download while revoked tokens block both actions", async ({ page, exportTransport }) => {
  await authenticate(page);
  await history(page);
  let revoked = false;
  let preparations = 0;
  await page.route("**/api/v2/runs/exports", route => {
    preparations++;
    const token = `clipboard-${preparations}`;
    exportTransport.documents.set(token, coachBytes);
    if (revoked) exportTransport.revoked.add(token);
    return route.fulfill({ json: snapshot(`clipboard-${preparations}`) });
  });
  await page.goto(`${exportTransport.url}/history`);
  await page.getByRole("checkbox", { name: `เลือกกิจกรรม ${firstId}` }).check();
  await page.getByRole("button", { name: "ถัดไป", exact: true }).click();
  await page.getByRole("checkbox", { name: `เลือกกิจกรรม ${secondId}` }).check();
  const actions = page.getByRole("region", { name: "ส่งออกกิจกรรมที่เลือก", exact: true });
  const coach = actions.getByRole("region", { name: "ส่งออก Coach JSON", exact: true });
  await coach.getByRole("button", { name: "เตรียม Coach JSON", exact: true }).click();
  await coach.getByRole("button", { name: "ตรวจ snapshot ก่อน Copy" }).click();
  await page.evaluate(() => {
    Object.defineProperty(navigator, "clipboard", {
      configurable: true, value: { writeText: async () => { throw new DOMException("Clipboard denied", "NotAllowedError"); } },
    });
  });
  await coach.getByRole("button", { name: "Copy Coach JSON", exact: true }).click();
  await expect(coach.getByRole("alert")).toBeVisible();
  await expect(actions).toContainText("เลือกไว้ 2 กิจกรรม");
  const download = page.waitForEvent("download");
  await coach.getByRole("button", { name: "Download Coach JSON", exact: true }).click();
  expect(await readFile((await (await download).path())!)).toEqual(Buffer.from(coachBytes));
  expect(preparations).toBe(1);

  await page.evaluate(async () => {
    Object.defineProperty(navigator, "clipboard", {
      configurable: true, value: { writeText: async () => { throw new Error("Revoked export must not reach clipboard"); } },
    });
  });
  revoked = true;
  exportTransport.revoked.add("clipboard-1");
  await coach.getByRole("button", { name: "Copy Coach JSON", exact: true }).click();
  await expect(coach.getByRole("button", { name: "Copy Coach JSON", exact: true })).toBeDisabled();
  await expect(coach.getByRole("button", { name: "Download Coach JSON", exact: true })).toBeDisabled();
  await expect(actions).toContainText("เลือกไว้ 2 กิจกรรม");
  expect(preparations).toBe(1);
  await actions.getByRole("button", { name: "สร้าง snapshot ใหม่", exact: true }).click();
  await coach.getByRole("button", { name: "เตรียม Coach JSON", exact: true }).click();
  await coach.getByRole("button", { name: "Download Coach JSON", exact: true }).click();
  await expect(coach.getByRole("button", { name: "Download Coach JSON", exact: true })).toBeDisabled();
  await expect(actions).toContainText("เลือกไว้ 2 กิจกรรม");
});

test("ready activity with pending historical thresholds keeps selection and unlocks export after polling", async ({ page }) => {
  await authenticate(page);
  let historyReady = false;
  await page.route("**/api/v2/runs?*", route => route.fulfill({ json: {
    items: [{
      ...rows[0],
      processing: { ...rows[0]!.processing, historyStatus: historyReady ? "ready" : "pending" },
    }],
    total: 1, limit: 50, offset: 0,
  } }));
  await page.goto("/history?offset=0&order=desc");
  const selected = page.getByRole("checkbox", { name: `เลือกกิจกรรม ${firstId}`, exact: true });
  await selected.check();
  const actions = page.getByRole("region", { name: "ส่งออกกิจกรรมที่เลือก", exact: true });
  await expect(actions).toContainText("เลือกไว้ 1 กิจกรรม");
  const coach = actions.getByRole("button", { name: "เตรียม Coach JSON", exact: true });
  const full = actions.getByRole("button", { name: "เตรียม Full JSON", exact: true });
  await expect(coach).toBeDisabled();
  await expect(full).toBeDisabled();
  const polled = page.waitForResponse(response =>
    new URL(response.url()).pathname === "/api/v2/runs" &&
    response.request().method() === "GET");
  historyReady = true;
  await polled;
  await expect(coach).toBeEnabled();
  await expect(full).toBeEnabled();
  await expect(selected).toBeChecked();
  await expect(actions).toContainText("เลือกไว้ 1 กิจกรรม");
});

test("browser text-capacity failure leaves clipboard untouched and preserves the same native snapshot download", async ({ page, exportTransport }) => {
  await authenticate(page);
  await history(page);
  let preparations = 0;
  await page.route("**/api/v2/runs/exports", route => {
    preparations++;
    exportTransport.documents.set("text-capacity", coachBytes);
    return route.fulfill({ json: snapshot("text-capacity") });
  });
  await page.goto(`${exportTransport.url}/history`);
  await page.getByRole("checkbox", { name: `เลือกกิจกรรม ${firstId}`, exact: true }).check();
  await page.getByRole("button", { name: "ถัดไป", exact: true }).click();
  await page.getByRole("checkbox", { name: `เลือกกิจกรรม ${secondId}`, exact: true }).check();
  const actions = page.getByRole("region", { name: "ส่งออกกิจกรรมที่เลือก", exact: true });
  const coach = actions.getByRole("region", { name: "ส่งออก Coach JSON", exact: true });
  await coach.getByRole("button", { name: "เตรียม Coach JSON", exact: true }).click();
  await coach.getByRole("button", { name: "ตรวจ snapshot ก่อน Copy" }).click();
  expect(exportTransport.requests.filter(request => request.method === "GET")).toEqual([]);
  await page.evaluate(() => {
    const originalText = Response.prototype.text;
    Response.prototype.text = function () {
      return this.url.endsWith("/api/v2/runs/exports/text-capacity")
        ? Promise.reject(new RangeError("Invalid string length"))
        : originalText.call(this);
    };
    Object.defineProperty(navigator, "clipboard", {
      configurable: true,
      value: { writeText: async () => { document.documentElement.dataset.clipboardTouched = "true"; } },
    });
  });
  await coach.getByRole("button", { name: "Copy Coach JSON", exact: true }).click();
  await expect(coach.getByRole("alert")).toBeVisible();
  await expect(coach.getByRole("status")).toHaveText("");
  expect(await page.evaluate(() => document.documentElement.dataset.clipboardTouched)).toBeUndefined();
  await expect(actions).toContainText("เลือกไว้ 2 กิจกรรม");
  const downloadPromise = page.waitForEvent("download");
  await coach.getByRole("button", { name: "Download Coach JSON", exact: true }).click();
  const download = await downloadPromise;
  expect(await download.failure()).toBeNull();
  expect(download.url()).toBe(new URL("/api/v2/runs/exports/text-capacity", page.url()).href);
  expect(await readFile((await download.path())!)).toEqual(Buffer.from(coachBytes));
  expect(preparations).toBe(1);
  expect(exportTransport.requests.filter(request => request.method === "GET")).toEqual([
    { method: "GET", token: "text-capacity" }, { method: "GET", token: "text-capacity" },
  ]);
});

test("native download HTTP and midstream failures never save error JSON or a truncated export", async ({ page, exportTransport }, testInfo) => {
  await authenticate(page);
  await history(page);
  exportTransport.documents.set("native-failure", coachBytes);
  await page.route("**/api/v2/runs/exports", route => route.fulfill({ json: snapshot("native-failure") }));
  await page.goto(`${exportTransport.url}/history`);
  await page.getByRole("checkbox", { name: `เลือกกิจกรรม ${firstId}`, exact: true }).check();
  const actions = page.getByRole("region", { name: "ส่งออกกิจกรรมที่เลือก", exact: true });
  const coach = actions.getByRole("region", { name: "ส่งออก Coach JSON", exact: true });
  await coach.getByRole("button", { name: "เตรียม Coach JSON", exact: true }).click();
  const failures: Array<{ phase: string; failure: string | null }> = [];
  for (const failure of ["http", "midstream"] as const) {
    exportTransport.failedGet.set("native-failure", failure);
    const downloadPromise = page.waitForEvent("download");
    await coach.getByRole("button", { name: "Download Coach JSON", exact: true }).click();
    const download = await downloadPromise;
    const reason = await download.failure();
    failures.push({ phase: failure, failure: reason });
    expect(reason).not.toBeNull();
    expect(download.url()).toBe(new URL("/api/v2/runs/exports/native-failure", page.url()).href);
    expect(await download.path().catch(() => null)).toBeNull();
    await expect(actions).toContainText("เลือกไว้ 1 กิจกรรม");
  }
  const evidence = testInfo.outputPath("native-transfer-failures.json");
  await writeFile(evidence, JSON.stringify({ failures, requests: exportTransport.requests, streamedBytes: exportTransport.streamedBytes }, null, 2));
  await testInfo.attach("native-transfer-failures", {
    contentType: "application/json",
    path: evidence,
  });
});

test("failed history refresh retains rows, selection, and pinned snapshot through recovery", async ({ page }) => {
  await authenticate(page);
  let phase: "pending" | "fail" | "ready" = "pending";
  await page.route("**/api/v2/runs?*", route => {
    const order = new URL(route.request().url()).searchParams.get("order");
    if (order === "asc" && phase === "fail") return route.fulfill({
      status: 500, json: { error: { code: "REQUEST_FAILED", message: "History update unavailable." } },
    });
    return route.fulfill({ json: {
      items: [{
        ...rows[0],
        processing: { ...rows[0]!.processing, historyStatus: order === "desc" || phase === "ready" ? "ready" : "pending" },
      }],
      total: 1, limit: 50, offset: 0,
    } });
  });
  let preparations = 0;
  await page.route("**/api/v2/runs/exports", route => {
    preparations++;
    return route.fulfill({ json: snapshot("poll-retained") });
  });
  await page.goto("/history?offset=0&order=desc");
  const selected = page.getByRole("checkbox", { name: `เลือกกิจกรรม ${firstId}`, exact: true });
  await selected.check();
  const actions = page.getByRole("region", { name: "ส่งออกกิจกรรมที่เลือก", exact: true });
  const coach = actions.getByRole("region", { name: "ส่งออก Coach JSON", exact: true });
  const full = actions.getByRole("button", { name: "เตรียม Full JSON", exact: true });
  await coach.getByRole("button", { name: "เตรียม Coach JSON", exact: true }).click();
  await expect(coach.locator(`time[datetime="${generatedAt}"]`)).toBeVisible();
  await page.getByLabel("เรียงลำดับ").selectOption("asc");
  await expect(page).toHaveURL(/\/history\?.*order=asc/);
  await expect(full).toBeDisabled();
  phase = "fail";
  const failed = page.waitForResponse(response =>
    new URL(response.url()).pathname === "/api/v2/runs" && response.status() === 500);
  await failed;
  await expect(page.getByTestId("history-table")).toBeVisible();
  await expect(selected).toBeChecked();
  await expect(actions).toContainText("เลือกไว้ 1 กิจกรรม");
  await expect(coach.locator(`time[datetime="${generatedAt}"]`)).toBeVisible();
  await expect(page.getByRole("alert")).toBeVisible();
  phase = "ready";
  await expect(full).toBeEnabled({ timeout: 10_000 });
  await expect(selected).toBeChecked();
  await expect(actions).toContainText("เลือกไว้ 1 กิจกรรม");
  await expect(coach.locator(`time[datetime="${generatedAt}"]`)).toBeVisible();
  await expect(page.getByRole("alert")).toHaveCount(0);
  expect(preparations).toBe(1);
});

test("successful deletion queues a fresh history read behind an older in-flight refresh", async ({ page }) => {
  await authenticate(page);
  await page.route("**/api/v2/runs/thresholds/latest", route => route.fulfill({
    status: 503, json: { error: { code: "REQUEST_FAILED", message: "Thresholds temporarily unavailable." } },
  }));
  let deleted = false;
  let reads = 0;
  let release!: () => void;
  let captured!: () => void;
  const gate = new Promise<void>(resolve => { release = resolve; });
  const held = new Promise<void>(resolve => { captured = resolve; });
  await page.route("**/api/v2/runs?*", async route => {
    const items = deleted ? [] : [rows[0]];
    if (++reads === 2) { captured(); await gate; }
    await route.fulfill({ json: { items, total: items.length, limit: 50, offset: 0 } });
  });
  await page.route(`**/api/v2/runs/${firstId}`, route => {
    deleted = true;
    return route.fulfill({ status: 204 });
  });
  try {
    await page.goto("/history");
    const selected = page.getByRole("checkbox", { name: `เลือกกิจกรรม ${firstId}`, exact: true });
    await selected.check();
    await page.getByRole("button", { name: "ลองโหลดอีกครั้ง", exact: true }).click();
    await held;
    await page.getByRole("button", { name: `ลบกิจกรรม ${firstId}`, exact: true }).click();
    const committed = page.waitForResponse(response =>
      new URL(response.url()).pathname === `/api/v2/runs/${firstId}` && response.request().method() === "DELETE");
    await page.getByTestId("confirm-delete").getByRole("button", { name: "ลบรายการ", exact: true }).click();
    await committed;
    release();
    await expect(selected).toHaveCount(0);
    await expect(page.getByRole("region", { name: "ส่งออกกิจกรรมที่เลือก", exact: true })).toContainText("เลือกไว้ 1 กิจกรรม");
  } finally { release(); }
});
