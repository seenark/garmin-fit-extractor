import { expect, test, type Page } from "@playwright/test";
import type { NormalizedRun, RunDetail, RunHistoricalThresholds, RunMetrics, RunSegment } from "../src/lib/runs-types";

const activityId = "11111111-1111-4111-8111-111111111111";
const summary: RunMetrics = { distanceMeters: 6000, timerTimeSeconds: 1599, elapsedTimeSeconds: 1629, movingTimeSeconds: null, averageSpeedMps: 4, averagePaceSecondsPerKm: 250, averageHeartRateBpm: 150, averagePowerWatts: 300.125, averageCadenceStepsPerMinute: null };
const normalized: NormalizedRun = {
  schemaVersion: "2.0.0", session: { index: 0, sourceReferences: {} },
  startTime: "2026-08-31T06:00:00Z", endTime: "2026-08-31T06:27:09Z", sport: "running", subtype: "generic", summary,
  samples: Array.from({ length: 1600 }, (_, index) => ({
    index, timestamp: null, elapsedSeconds: index + (index >= 100 ? 30 : 0), speedMps: index === 56 ? 0 : 4, paceSecondsPerKm: index === 56 ? null : 250,
    heartRateBpm: index === 54 ? null : 150, powerWatts: index === 42 ? 999 : index === 55 ? 0 : 300.125,
    cadenceStepsPerMinute: null, altitudeMeters: null, distanceMeters: null, timerRunning: index === 57 ? false : true,
    sourceReferences: { powerWatts: { messageIndex: index, globalMessageNumber: 20, fieldNumber: 7 } },
  })),
  laps: [{ index: 0, startTime: "2026-08-31T06:00:00Z", endTime: null, startElapsedSeconds: 0, endElapsedSeconds: 37.5, summary: { ...summary, distanceMeters: 150, timerTimeSeconds: 37.5 }, sourceReferences: [] }],
  timerEvents: [], rr: { intervals: [], alignmentEligible: false, reasons: ["RR_MISSING"] }, sensors: [], zones: {}, deviceReportedThresholds: [], extensions: {}, warnings: [],
};
const segments: RunSegment[] = [
  { index: 0, kind: "steady", startElapsedSeconds: 10, endElapsedSeconds: 40, durationSeconds: 30, timeBasis: "elapsed", version: "test-policy", features: { heartRateCoverage: 1 }, eligibility: { lt1: { accepted: true, reasons: [] }, lt2: { accepted: false, reasons: ["DURATION_SHORT"] } } },
  { index: 1, kind: "surge", startElapsedSeconds: 41, endElapsedSeconds: 60, durationSeconds: 19, timeBasis: "elapsed", version: "test-policy", features: { pauseDetected: true }, eligibility: { lt1: { accepted: false, reasons: ["TIMER_PAUSE"] }, lt2: { accepted: false, reasons: ["TIMER_PAUSE"] } } },
];
const detail: RunDetail = {
  id: activityId, startTime: normalized.startTime, endTime: normalized.endTime, summary, sourceUnavailable: false,
  processing: { status: "ready", stale: false, updateFailed: false, errorCode: null }, revisionId: "chart-revision",
  normalized, analysis: { schemaVersion: "2.0.0", quality: {}, segments, thresholds: {}, transformations: [] }, historicalThresholds: null,
};
async function openRun(page: Page, run: RunDetail = detail) {
  await page.route("**/api/v1/auth/me", (route) => route.fulfill({ json: { user: { id: "chart-user", email: "chart@example.test", displayName: "Chart test" }, isAdmin: false } }));
  await page.route(`**/api/v2/runs/${activityId}`, (route) => route.fulfill({ json: run }));
  await page.goto(`/extractions/${activityId}`);
  await expect(page.locator(".runs-charts")).toBeVisible();
}

test("actual normalized timelines keep gaps and exact original inspection linked through zoom and recorded laps", async ({ page }) => {
  const errors: string[] = [];
  page.on("pageerror", (error) => errors.push(error.message));
  await openRun(page);
  const charts = page.locator(".runs-charts");
  const plots = charts.locator(".runs-chart-plot");
  await expect(plots).toHaveCount(3);
  // Metric-specific missing values and shared pause/gap must remain separate rendered paths.
  await expect(plots.nth(0).locator("svg .ts-chart__line path")).toHaveCount(3);
  await expect(plots.nth(1).locator("svg .ts-chart__line path")).toHaveCount(4);
  await expect(plots.nth(2).locator("svg .ts-chart__line path")).toHaveCount(3);
  const sourceSlider = charts.locator('input[type="range"]');
  await sourceSlider.fill("45");
  const inspector = charts.locator(".runs-chart-inspector");
  await expect(inspector).toContainText("300.125 W");
  await expect(inspector).toContainText("250 s/km");
  await expect(inspector).toContainText("45 s");
  await inspector.getByText("ที่มาของค่าจุดนี้", { exact: true }).click();
  await expect(inspector.locator("pre")).toContainText('"messageIndex": 45');
  const controls = charts.locator(".runs-chart-interaction");
  await controls.first().focus();
  await page.keyboard.press("ArrowRight");
  await expect(sourceSlider).toHaveValue("46");
  await page.keyboard.press("+");
  await expect(charts.getByRole("button", { name: "คืนช่วงทั้งหมด", exact: true })).toBeEnabled();
  await page.keyboard.press("r");
  await expect(charts.getByRole("button", { name: "คืนช่วงทั้งหมด", exact: true })).toBeDisabled();
  await charts.getByRole("button", { name: "Lap 1", exact: true }).click();
  await expect(charts.getByRole("button", { name: "คืนช่วงทั้งหมด", exact: true })).toBeEnabled();
  await expect(plots.nth(2).locator("svg circle")).not.toHaveCount(0);
  await charts.getByRole("button", { name: "คืนช่วงทั้งหมด", exact: true }).click();
  await sourceSlider.fill("55");
  await expect(inspector).toContainText("0 W");
  await sourceSlider.fill("54");
  await expect(inspector.locator("dl > div").nth(1).locator("dd")).toHaveText("ไม่มีข้อมูล");
  await sourceSlider.fill("57");
  await expect(inspector).toContainText("300.125 W");
  const rejected = charts.locator(".runs-chart-segments details").nth(1);
  await rejected.locator("summary").click();
  await expect(rejected).toContainText("TIMER_PAUSE");
  await charts.locator('input[type="checkbox"]').uncheck();
  await expect(rejected).toContainText("TIMER_PAUSE");
  expect(errors).toEqual([]);
});

test("mobile touch inspection and controls remain usable without page overflow", async ({ page }) => {
  await page.setViewportSize({ width: 375, height: 900 });
  await openRun(page);
  const charts = page.locator(".runs-charts");
  const plot = charts.locator(".runs-chart-interaction").first();
  await plot.scrollIntoViewIfNeeded();
  const svg = plot.locator("svg");
  const box = await svg.boundingBox();
  expect(box).not.toBeNull();
  await svg.dispatchEvent("pointerdown", { clientX: box!.x + box!.width / 2, clientY: box!.y + box!.height / 2, pointerType: "touch", bubbles: true });
  const slider = charts.locator('input[type="range"]');
  await expect.poll(async () => Number(await slider.inputValue())).toBeGreaterThan(0);
  await charts.getByRole("button", { name: "ซูมเข้า", exact: true }).click();
  await expect(charts.getByRole("button", { name: "คืนช่วงทั้งหมด", exact: true })).toBeEnabled();
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= document.documentElement.clientWidth)).toBe(true);
  await page.screenshot({ path: "/tmp/runs-chart-mobile-proof.png", fullPage: true });
});

test("missing streams never create sample charts from summary or lap averages", async ({ page }) => {
  await openRun(page, { ...detail, normalized: { ...normalized, samples: [] }, analysis: { ...detail.analysis!, segments: [] } });
  const charts = page.locator(".runs-charts");
  await expect(charts.locator(".runs-chart-plot")).toHaveCount(0);
  await expect(charts.locator('input[type="range"]')).toHaveCount(0);
  await expect(charts.locator(".runs-chart-laps tbody tr")).toHaveCount(1);
  await expect(charts.getByRole("button", { name: "Lap 1", exact: true })).toBeDisabled();
});

test("a missing sensor stream has no synthetic zero line or numeric chart axis", async ({ page }) => {
  await openRun(page, { ...detail, normalized: { ...normalized, samples: normalized.samples.map((sample) => ({ ...sample, powerWatts: null })) } });
  const plots = page.locator(".runs-chart-plot");
  await expect(plots.nth(0).locator("svg")).toHaveCount(1);
  await expect(plots.nth(1).locator("svg")).toHaveCount(1);
  await expect(plots.nth(2).locator("svg")).toHaveCount(0);
  await expect(page.locator(".runs-chart-inspector dl > div").nth(2).locator("dd")).toHaveText("ไม่มีข้อมูล");
});

test("duplicate timestamps remain separately selectable by slider, arrows and native point inspection", async ({ page }) => {
  const samples = [0, 4, 4, 8].map((elapsedSeconds, index) => ({
    ...normalized.samples[index]!,
    index, elapsedSeconds,
    heartRateBpm: [120, 140, 175, 150][index]!,
    powerWatts: [100, 200, 900.125, 300][index]!,
    paceSecondsPerKm: [250, 260, 300, 270][index]!,
    speedMps: 1000 / [250, 260, 300, 270][index]!,
  }));
  await openRun(page, { ...detail, normalized: { ...normalized, samples, laps: [] }, analysis: { ...detail.analysis!, segments: [] } });
  const charts = page.locator(".runs-charts");
  const slider = charts.locator('input[type="range"]');
  const inspector = charts.locator(".runs-chart-inspector");
  await slider.fill("1");
  const keyboard = charts.locator(".runs-chart-interaction").first();
  await keyboard.focus();
  await page.keyboard.press("ArrowRight");
  await expect(slider).toHaveValue("2");
  await expect(inspector).toContainText("175 bpm");
  await expect(inspector).toContainText("900.125 W");
  await page.keyboard.press("ArrowRight");
  await expect(slider).toHaveValue("3");
  await page.keyboard.press("ArrowLeft");
  await expect(slider).toHaveValue("2");
  await slider.fill("2");
  await expect(inspector).toContainText("300 s/km");
  await slider.fill("0");
  const hrPlot = charts.locator(".runs-chart-plot").nth(1);
  const point = await hrPlot.locator("svg circle").nth(2).boundingBox();
  expect(point).not.toBeNull();
  await hrPlot.locator("svg").dispatchEvent("pointerdown", { clientX: point!.x + point!.width / 2, clientY: point!.y + point!.height / 2, pointerType: "mouse", bubbles: true });
  await expect(slider).toHaveValue("2");
  await expect(inspector).toContainText("175 bpm");
  await expect(inspector).toContainText("900.125 W");
});

test("each target can independently dismiss and reopen its unchanged suggestions", async ({ page }) => {
  const lt1: RunHistoricalThresholds["lt1"] = {
    status: "insufficient_data", engineStatus: "experimental", researchBlocked: true,
    method: { id: "running-dfa", version: "1.0.0", configurationHash: "suggestion-policy" }, targetDefinition: "VT proxy",
    value: null, uncertainty: { interval: null, reason: "research_not_validated" }, reasons: ["RR_COVERAGE_LOW"],
    evidence: { independentActivityCount: 1, startTime: normalized.startTime, endTime: normalized.endTime, ageDays: 0 }, trace: null,
    suggestions: [{ reason: "RR_COVERAGE_LOW", message: "คำแนะนำ LT1 · optional" }],
  };
  await openRun(page, { ...detail, historicalThresholds: {
    evidenceCutoff: normalized.endTime, computedAt: normalized.endTime,
    lt1, lt2: { ...lt1, suggestions: [{ reason: "INSUFFICIENT_ACTIVITY_HISTORY", message: "คำแนะนำ LT2 · optional" }] },
  } });
  const lt1Aside = page.getByRole("complementary", { name: "คำแนะนำเสริม LT1", exact: true });
  const lt2Aside = page.getByRole("complementary", { name: "คำแนะนำเสริม LT2", exact: true });
  const original = await lt1Aside.locator("ul").textContent();
  await lt1Aside.getByRole("button", { name: "ซ่อนคำแนะนำ LT1", exact: true }).click();
  await expect(lt1Aside.locator("ul")).toBeHidden();
  await expect(lt2Aside.locator("ul")).toBeVisible();
  const reopen = lt1Aside.getByRole("button", { name: "แสดงคำแนะนำอีกครั้ง LT1", exact: true });
  await expect(reopen).toHaveAttribute("aria-expanded", "false");
  await reopen.focus();
  await page.keyboard.press("Enter");
  await expect(lt1Aside.locator("ul")).toBeVisible();
  expect(await lt1Aside.locator("ul").textContent()).toBe(original);
  await expect(lt1Aside.getByRole("button", { name: "ซ่อนคำแนะนำ LT1", exact: true })).toHaveAttribute("aria-expanded", "true");
});
