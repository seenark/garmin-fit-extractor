import { expect, test, type Page } from "@playwright/test";
import { readFile } from "node:fs/promises";

const firstId = "10000000-0000-4000-8000-000000000001";
const secondId = "10000000-0000-4000-8000-000000000051";
const metrics = {
  distanceMeters: 5000, timerTimeSeconds: 1500, elapsedTimeSeconds: 1510,
  movingTimeSeconds: null, averageSpeedMps: 10 / 3, averagePaceSecondsPerKm: 300,
  averageHeartRateBpm: 150, averagePowerWatts: null, averageCadenceStepsPerMinute: 170,
};
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
  privacyOmissions: [{ category: "location", pathPattern: "activities.*.samples.*.positionLat", count: 2, reason: "Location excluded by policy." }],
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
  };
}

test("selection survives pages and copy plus download preserve pinned Coach and Full bytes", async ({ page }) => {
  await authenticate(page);
  await history(page);
  const exports: unknown[] = [];
  await page.route("**/api/v2/runs/exports", route => {
    const body = route.request().postDataJSON();
    exports.push(body);
    return route.fulfill({ json: snapshot(body.mode, body.mode === "coach" ? coachBytes : fullBytes) });
  });
  await page.route("**/api/v2/runs/exports/*", route => route.fulfill({
    contentType: "application/json", body: route.request().url().endsWith("/coach") ? coachBytes : fullBytes,
    headers: { "Cache-Control": "private, no-store" },
  }));
  await page.goto("/history");
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
    // Full uses metadata-only download; Coach downloads after preview/copy.
    const bytes = mode === "Coach" ? coachBytes : fullBytes;
    if (mode === "Full") {
      const download = page.waitForEvent("download");
      await section.getByRole("button", { name: `Download ${mode} JSON` }).click();
      const path = await (await download).path();
      expect(await readFile(path!)).toEqual(Buffer.from(bytes));
    }
    await section.getByRole("button", { name: "ตรวจรายการที่ละไว้ก่อน Copy" }).click();
    if (mode === "Coach") await expect(section).toContainText("activities.*.samples.*.positionLat");
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

test("clipboard denial preserves snapshot download while revoked tokens block both actions", async ({ page }) => {
  await authenticate(page);
  await history(page);
  let revoked = false;
  let preparations = 0;
  await page.route("**/api/v2/runs/exports", route => {
    preparations++;
    return route.fulfill({ json: snapshot(`clipboard-${preparations}`) });
  });
  await page.route("**/api/v2/runs/exports/*", route => revoked
    ? route.fulfill({ status: 410, json: { error: { code: "EXPORT_EXPIRED", message: "Snapshot is no longer available." } } })
    : route.fulfill({ contentType: "application/json", body: coachBytes }));
  await page.goto("/history");
  await page.getByRole("checkbox", { name: `เลือกกิจกรรม ${firstId}` }).check();
  await page.getByRole("button", { name: "ถัดไป", exact: true }).click();
  await page.getByRole("checkbox", { name: `เลือกกิจกรรม ${secondId}` }).check();
  const actions = page.getByRole("region", { name: "ส่งออกกิจกรรมที่เลือก", exact: true });
  const coach = actions.getByRole("region", { name: "ส่งออก Coach JSON", exact: true });
  await coach.getByRole("button", { name: "เตรียม Coach JSON", exact: true }).click();
  await coach.getByRole("button", { name: "ตรวจรายการที่ละไว้ก่อน Copy" }).click();
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
