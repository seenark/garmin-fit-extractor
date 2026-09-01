import { expect, test } from "@playwright/test";

const extractionId = "11111111-1111-4111-8111-111111111111";
const emptyExtractionId = "22222222-2222-4222-8222-222222222222";
const analysis = {
  schemaVersion: "1.0.0",
  source: { fileName: "chart-qa.fit" },
  activity: { type: "running", subType: "road", date: "2026-08-31T06:00:00.000Z" },
  summary: {
    duration: { value: 3600, unit: "seconds" },
    movingTime: { value: 3540, unit: "seconds" },
    distance: { value: 10000, unit: "meters" },
    calories: { value: 700, unit: "kcal" },
  },
  heartRate: {
    averageBpm: 151,
    maximumBpm: 181,
    zones: [
      { zone: 1, minBpm: 100, maxBpm: 119, durationSeconds: 240 },
      { zone: 2, minBpm: 120, maxBpm: 139, durationSeconds: 900 },
      { zone: 3, minBpm: 140, maxBpm: 159, durationSeconds: 1500 },
      { zone: 4, minBpm: 160, maxBpm: 179, durationSeconds: 720 },
      { zone: 5, minBpm: 180, maxBpm: null, durationSeconds: 180 },
    ],
  },
  pace: {
    average: { value: 360, unit: "seconds_per_kilometer" },
    moving: { value: 350, unit: "seconds_per_kilometer" },
    best: { value: 315, unit: "seconds_per_kilometer" },
  },
  power: { averageWatts: 250, maximumWatts: 410, zones: [
    { zone: 1, minWatts: 0, maxWatts: 150, durationSeconds: 30 },
    { zone: 2, minWatts: 151, maxWatts: 220, durationSeconds: 120 },
    { zone: 3, minWatts: 221, maxWatts: 280, durationSeconds: 300 },
    { zone: 4, minWatts: 281, maxWatts: 340, durationSeconds: 600 },
    { zone: 5, minWatts: 341, maxWatts: 410, durationSeconds: 420 },
    { zone: 6, minWatts: 411, maxWatts: 500, durationSeconds: 120 },
    { zone: 7, minWatts: 501, maxWatts: null, durationSeconds: 10 },
  ] },
  runningDynamics: {
    cadence: { averageStepsPerMinute: 176, maximumStepsPerMinute: 188 },
    strideLength: { value: 1.2, unit: "meters" },
    groundContactTime: { value: 210, unit: "milliseconds" },
    verticalOscillation: { value: 8, unit: "millimeters" },
    verticalRatio: { value: 7, unit: "percent" },
  },
  elevation: {
    ascent: { value: 420, unit: "meters" },
    descent: { value: 415, unit: "meters" },
  },
  temperature: { averageCelsius: 18, minimumCelsius: 15, maximumCelsius: 21 },
  samples: [1, 2, 3, 4, 5, 6].map((index) => ({
    index,
    timestamp: `2026-08-31T06:0${index}:00.000Z`,
    elapsedSeconds: index * 60,
    heartRateBpm: 145 + index,
    powerWatts: 240 + index * 5,
  })),
  laps: [1, 2, 3, 4, 5, 6].map((index) => ({
    index,
    startTime: `2026-08-31T06:0${index}:00.000Z`,
    distance: { value: 1000, unit: "meters" },
    duration: { value: 350 + index * 3, unit: "seconds" },
    movingTime: { value: 345 + index * 3, unit: "seconds" },
    pace: { value: 350 + index * 3, unit: "seconds_per_kilometer" },
    heartRate: { averageBpm: 145 + index, maximumBpm: 158 + index },
    power: { averageWatts: 240 + index, maximumWatts: 300 + index },
    cadence: { averageStepsPerMinute: 172 + index, maximumStepsPerMinute: 182 + index },
  })),
};

const detail = {
  id: extractionId,
  fileName: "chart-qa.fit",
  fileSizeBytes: 1024,
  status: "succeeded",
  activityType: "running",
  activityDate: "2026-08-31T06:00:00.000Z",
  createdAt: "2026-08-31T06:00:00.000Z",
  normalized: analysis,
  raw: [],
};

const emptyDetail = {
  ...detail,
  id: emptyExtractionId,
  normalized: {
    ...analysis,
    heartRate: { ...analysis.heartRate, zones: [] },
    power: { ...analysis.power, zones: [] },
    samples: [],
    laps: analysis.laps.map((lap) => ({
      ...lap,
      power: { averageWatts: null, maximumWatts: null },
    })),
  },
};

test("renders every activity chart without responsive overflow", async ({ page }) => {
  await page.goto("/api/v1/auth/test-login?user=chart-qa");
  await expect(page.getByText("chart-qa", { exact: true })).toBeVisible();

  const consoleErrors: string[] = [];
  const pageErrors: string[] = [];
  page.on("console", (message) => {
    if (message.type() === "error") consoleErrors.push(message.text());
  });
  page.on("pageerror", (error) => pageErrors.push(error.message));

  await page.route(`**/api/v1/extractions/${extractionId}`, async (route) => {
    await route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify(detail),
    });
  });
  await page.goto(`/extractions/${extractionId}`);
  await expect(page.getByTestId("activity-chart-grid")).toBeVisible();
  await expect(page.getByTestId("activity-chart-pace")).toBeVisible();
  await expect(page.getByTestId("activity-chart-heart-rate-zones")).toBeVisible();
  await expect(page.getByTestId("activity-chart-power-zones")).toBeVisible();
  await expect(page.getByTestId("activity-chart-power")).toBeVisible();
  await expect(page.getByTestId("activity-chart-heart-rate")).toBeVisible();
  await expect(page.getByTestId("activity-chart-elevation")).toBeVisible();
  await expect(page.getByRole("img", { name: "กราฟเพซต่อรอบ" })).toBeVisible();
  await expect(page.getByText("เพซ (นาที/กม.)", { exact: true })).toBeVisible();
  await expect(
    page.getByText("เพซ (วินาทีต่อกิโลเมตร)", { exact: true }),
  ).toHaveCount(0);
  await expect(page.getByRole("img", { name: "กราฟเวลาใน Heart-rate zone" })).toBeVisible();
  await expect(page.getByRole("img", { name: "กราฟเวลาใน Power zone" })).toBeVisible();
  await expect(page.getByRole("img", { name: "กราฟกำลังตลอดกิจกรรม" })).toBeVisible();
  await expect(page.getByRole("img", { name: "กราฟอัตราการเต้นหัวใจต่อรอบ" })).toBeVisible();
  await expect(page.getByRole("img", { name: "กราฟสมดุลระดับความสูง" })).toBeVisible();

  const heartRateSwatches = page.getByTestId("activity-chart-heart-rate-zones").locator(".activity-chart-zone-swatch");
  await expect(heartRateSwatches).toHaveCount(5);
  for (const [index, token] of ["gray", "blue", "green", "orange", "red"].entries()) {
    await expect(heartRateSwatches.nth(index)).toHaveAttribute(
      "style",
      expect.stringContaining(`--color-zone-${token}`),
    );
  }
  const powerSwatches = page.getByTestId("activity-chart-power-zones").locator(".activity-chart-zone-swatch");
  await expect(powerSwatches).toHaveCount(7);
  for (const [index, token] of ["gray", "blue", "green", "yellow", "orange", "red", "purple"].entries()) {
    await expect(powerSwatches.nth(index)).toHaveAttribute(
      "style",
      expect.stringContaining(`--color-zone-${token}`),
    );
  }

  const chartSection = page.getByTestId("activity-charts");
  const summaryHeading = page.getByRole("heading", { name: "สรุป", exact: true });
  const chartBox = await chartSection.boundingBox();
  const summaryBox = await summaryHeading.boundingBox();
  expect(chartBox).not.toBeNull();
  expect(summaryBox).not.toBeNull();
  expect(chartBox!.y, "charts must appear before the summary").toBeLessThan(
    summaryBox!.y,
  );

  for (const width of [320, 375, 414, 768, 1280]) {
    await page.setViewportSize({ width, height: 900 });
    await expect(page.getByTestId("activity-chart-grid")).toBeVisible();
    await page.waitForTimeout(100);
    const layout = await page.evaluate(() => {
      const viewport = document.documentElement.clientWidth;
      const visibleElements = Array.from(document.querySelectorAll<HTMLElement>("body *")).filter(
        (element) => {
          const style = window.getComputedStyle(element);
          const rect = element.getBoundingClientRect();
          return style.display !== "none" && style.visibility !== "hidden" && rect.width > 0 && rect.height > 0;
        },
      );
      return {
        clientWidth: viewport,
        scrollWidth: document.documentElement.scrollWidth,
        overflowers: visibleElements
          .filter((element) => {
            const rect = element.getBoundingClientRect();
            return rect.left < -1 || rect.right > viewport + 1;
          })
          .slice(0, 8)
          .map((element) => ({ tag: element.tagName, className: element.className })),
      };
    });
    expect(layout.scrollWidth, `horizontal overflow at ${width}px: ${JSON.stringify(layout.overflowers)}`).toBeLessThanOrEqual(layout.clientWidth);
    await page.screenshot({ path: `/tmp/garmin-chart-qa-${width}.png`, fullPage: true });
  }

  expect(consoleErrors).toEqual([]);
  expect(pageErrors).toEqual([]);
});

test("shows explicit empty states for missing zone and power samples", async ({ page }) => {
  await page.goto("/api/v1/auth/test-login?user=chart-empty-qa");
  await expect(page.getByText("chart-empty-qa", { exact: true })).toBeVisible();

  await page.route(`**/api/v1/extractions/${emptyExtractionId}`, async (route) => {
    await route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify(emptyDetail),
    });
  });
  await page.goto(`/extractions/${emptyExtractionId}`);
  await expect(page.getByTestId("activity-chart-heart-rate-zones")).toBeVisible();
  await expect(page.getByTestId("activity-chart-power-zones")).toBeVisible();
  await expect(page.getByTestId("activity-chart-power")).toBeVisible();
  await expect(page.getByText("ไฟล์ FIT นี้ไม่มีข้อมูลเวลาใน Heart-rate zone", { exact: true })).toBeVisible();
  await expect(page.getByText("ไฟล์ FIT นี้ไม่มีข้อมูลเวลาใน Power zone", { exact: true })).toBeVisible();
  await expect(page.getByText("ไฟล์ FIT นี้ไม่มี record samples ที่มีข้อมูลกำลังสำหรับกราฟ", { exact: true })).toBeVisible();
  await expect(page.getByRole("img", { name: "กราฟเวลาใน Heart-rate zone" })).toHaveCount(0);
  await expect(page.getByRole("img", { name: "กราฟเวลาใน Power zone" })).toHaveCount(0);
  await expect(page.getByRole("img", { name: "กราฟกำลังตลอดกิจกรรม" })).toHaveCount(0);
});
