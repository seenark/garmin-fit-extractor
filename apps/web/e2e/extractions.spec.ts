import { expect, test } from "@playwright/test";
import { readFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";

const activityArchive = fileURLToPath(
  new URL("../../api/tests/fixtures/runs/garmin_mixed.zip", import.meta.url),
);

test("authenticates, imports ZIP members, exports pinned Runs v2 snapshots, and isolates history", async ({
  page,
  context,
}, testInfo) => {
  await context.grantPermissions(["clipboard-read", "clipboard-write"]);

  await page.goto("/");
  await expect(page.locator("h1")).toHaveCount(1);
  await expect(page.locator("h1")).toHaveText(
    "เรื่องวิ่งของคุณ มีอะไรให้ดูมากกว่าที่คิด",
  );
  await expect(page.getByText("Runner’s Garage", { exact: true }).first()).toBeVisible();
  await expect(page.getByText("by น้ำเน่ารันคลับ", { exact: true }).first()).toBeVisible();
  await expect(page.getByRole("link", { name: "Runs", exact: true })).toHaveAttribute(
    "href",
    "/history?offset=0&order=desc",
  );
  await expect(page.getByRole("link", { name: "Shoes", exact: true })).toHaveAttribute(
    "href",
    "/shoes",
  );
  await expect(
    page
      .getByRole("navigation", { name: "เมนูหลัก" })
      .getByRole("link", { name: "เพิ่มข้อมูลวิ่ง", exact: true }),
  ).toHaveAttribute(
    "href",
    "/upload",
  );
  await expect(page.getByRole("link", { name: "หน้าหลัก", exact: true })).toHaveCount(0);
  await expect(page.getByTestId("home-runs-cta")).toHaveAttribute(
    "href",
    "/history?offset=0&order=desc",
  );
  await expect(page.getByTestId("home-shoes-cta")).toHaveAttribute("href", "/shoes");
  await expect(page.getByTestId("home-run-visualization")).toBeVisible();
  await expect(page.getByRole("heading", { name: "หยิบใช้ทีละเรื่อง ไม่ต้องเปิดทุกอย่างพร้อมกัน" })).toBeVisible();
  await expect(page.getByRole("heading", { name: "เก็บข้อมูลของเราเอง ดูให้ลึกขึ้น" })).toBeVisible();
  await expect(page.getByRole("heading", { name: "ไซซ์เดียวกัน ไม่ได้แปลว่าฟิตเหมือนกัน" })).toBeVisible();
  await page.goto("/shoes/new-balance-fuelcell-supercomp-elite-v6");
  await expect(page.getByRole("heading", { name: "FuelCell SuperComp Elite v6" })).toBeVisible();
  await expect(page.getByRole("heading", { name: "กำลังหาว่าคู่นี้ควรใส่ไซซ์อะไร?" })).toBeVisible();
  await expect(page.getByRole("combobox", { name: "รองเท้าที่คุณใส่อยู่" })).toBeVisible();
  await expect(page.getByLabel("ระบบไซซ์")).toHaveValue("US_M");
  await expect(page.getByRole("radio", { name: "พอดี" })).toBeChecked();
  await page.getByRole("button", { name: "เทียบไซซ์" }).click();
  await expect(page.getByTestId("shoe-size-result")).toContainText("US Men’s 9.5");
  await expect(page.getByTestId("shoe-size-result")).toContainText("1 direct bridge");
  await expect(page.getByTestId("shoe-size-result")).toContainText("Papziza");
  await expect(page.getByTestId("shoe-size-result")).toContainText("ไซซ์ที่ผู้รีวิวใส่จริง");
  await expect(page.getByRole("heading", { name: "ตารางไซซ์ทางการ" })).toBeVisible();
  await page.goto("/history");
  await expect(
    page.getByRole("button", { name: "เข้าสู่ระบบด้วย Google" }),
  ).toBeVisible();
  await page.goto("/upload");
  await expect(page.getByTestId("upload-dropzone")).toHaveCount(0);
  await expect(
    page.getByRole("button", { name: "เข้าสู่ระบบเพื่ออัปโหลด" }),
  ).toBeVisible();

  await page.goto("/api/v1/auth/test-login?user=alice");
  await expect(page.getByText("alice", { exact: true })).toBeVisible();
  await page
    .getByRole("navigation", { name: "เมนูหลัก" })
    .getByRole("link", { name: "เพิ่มข้อมูลวิ่ง", exact: true })
    .click();
  await expect(page).toHaveURL(/\/upload$/);

  const activityBytes = await readFile(activityArchive);
  const uploadInput = page
    .getByTestId("upload-dropzone")
    .locator('input[type="file"]');
  await uploadInput.setInputFiles([
    {
      name: "activity.zip",
      mimeType: "application/zip",
      buffer: activityBytes,
    },
    {
      name: "corrupt.zip",
      mimeType: "application/zip",
      buffer: Buffer.from("not a ZIP archive"),
    },
  ]);

  await expect(page.getByTestId("selected-file")).toHaveCount(2);
  const importedResponse = page.waitForResponse(response =>
    new URL(response.url()).pathname === "/api/v2/runs/imports" &&
    response.request().method() === "POST");
  await page.getByTestId("upload-submit").click();
  const batch = await (await importedResponse).json();
  expect(batch.counts).toEqual({ imported: 1, duplicate: 0, unsupported: 2, failed: 2 });
  const imported = batch.items.find((item: { status: string }) => item.status === "imported");
  expect(imported.activityId).toMatch(/^[0-9a-f-]{36}$/i);
  const activityId: string = imported.activityId;
  const batchResults = page.getByTestId("batch-result");
  await expect(batchResults).toHaveCount(5);
  await expect(page.locator('[data-testid="batch-result"][data-status="imported"]')).toHaveCount(1);
  await expect(page.locator('[data-testid="batch-result"][data-status="unsupported"]')).toHaveCount(2);
  await expect(page.locator('[data-testid="batch-result"][data-status="failed"]')).toHaveCount(2);
  await page.locator('[data-testid="batch-result"][data-status="imported"]').getByRole("link", { name: "ดูรายละเอียด" }).click();
  await expect(page).toHaveURL(/\/extractions\/[0-9a-f-]{36}(?:\?.*)?$/i);
  await expect(page.getByRole("region", { name: "Pace / HR / Power", exact: true })).toBeVisible({ timeout: 45_000 });
  const detailResponse = await context.request.get(`/api/v2/runs/${activityId}`);
  expect(detailResponse.ok()).toBe(true);
  const detail = await detailResponse.json();
  expect(detail.normalized.summary).toMatchObject({
    distanceMeters: 1000, timerTimeSeconds: 300, elapsedTimeSeconds: 360, movingTimeSeconds: null,
  });
  for (const [mode, label] of [["coach", "Coach"], ["full", "Full"]] as const) {
    const section = page.getByRole("region", { name: `ส่งออก ${label} JSON`, exact: true });
    const preparedResponse = page.waitForResponse(response =>
      new URL(response.url()).pathname === "/api/v2/runs/exports" &&
      response.request().method() === "POST");
    await section.getByRole("button", { name: `เตรียม ${label} JSON`, exact: true }).click();
    const snapshot = await (await preparedResponse).json();
    const serverResponse = await context.request.get(snapshot.downloadUrl);
    expect(serverResponse.ok()).toBe(true);
    const serverBytes = await serverResponse.body();
    expect(serverBytes.at(-1)).toBe(10);
    expect(JSON.parse(serverBytes.toString())).toMatchObject({
      schemaVersion: "2.0.0", mode, selection: [activityId],
      privacy: { includeLocation: false, includeDeviceIdentifiers: false },
    });
    await section.getByRole("button", { name: "ตรวจรายการที่ละไว้ก่อน Copy" }).click();
    await section.getByRole("button", { name: `Copy ${label} JSON`, exact: true }).click();
    await expect.poll(() => page.evaluate(() => navigator.clipboard.readText())).toBe(serverBytes.toString());
    const download = page.waitForEvent("download");
    await section.getByRole("button", { name: `Download ${label} JSON`, exact: true }).click();
    const path = await (await download).path();
    expect(await readFile(path!)).toEqual(serverBytes);
  }

  await page.getByRole("link", { name: "Runs", exact: true }).click();
  await expect(page).toHaveURL(/\/history(?:\?.*)?$/);
  const historyTable = page.getByTestId("history-table");
  const successfulRow = historyTable.locator("tr").filter({
    has: page.getByRole("checkbox", { name: `เลือกกิจกรรม ${activityId}`, exact: true }),
  });
  await expect(successfulRow).toBeVisible();
  await page.getByLabel("เรียงลำดับ").selectOption("asc");
  await expect(page).toHaveURL(/\/history\?.*order=asc/);
  await successfulRow.getByRole("link", { name: "เปิดดู" }).click();
  await expect(page).toHaveURL(/\/extractions\/[0-9a-f-]{36}\?.*order=asc/i);
  await page.getByRole("link", { name: "กลับไปประวัติ", exact: true }).click();
  await expect(page.getByLabel("เรียงลำดับ")).toHaveValue("asc");

  await page.getByRole("button", { name: "ออกจากระบบ" }).click();
  await expect(page.getByRole("button", { name: "เข้าสู่ระบบด้วย Google" })).toBeVisible();
  await page.goto("/api/v1/auth/test-login?user=bob");
  await expect(page.getByText("bob", { exact: true })).toBeVisible();
  await page.getByRole("link", { name: "Runs", exact: true }).click();
  await expect(historyTable).toHaveCount(0);
  await expect(page.getByRole("button", { name: "เตรียม Coach JSON", exact: true })).toBeDisabled();
  const denied = await context.request.get(`/api/v2/runs/${activityId}`);
  expect(denied.status()).toBe(404);
  await page.getByRole("button", { name: "ออกจากระบบ" }).click();
  await page.goto("/api/v1/auth/test-login?user=alice");
  await expect(page.getByText("alice", { exact: true })).toBeVisible();
  await page.getByRole("link", { name: "Runs", exact: true }).click();
  await expect(successfulRow).toBeVisible();
  await successfulRow.getByRole("button", { name: `ลบกิจกรรม ${activityId}`, exact: true }).click();
  const confirmation = page.getByTestId("confirm-delete");
  const deleted = page.waitForResponse(response =>
    new URL(response.url()).pathname === `/api/v2/runs/${activityId}` &&
    response.request().method() === "DELETE");
  await confirmation.getByRole("button", { name: "ลบรายการ", exact: true }).click();
  expect((await deleted).status()).toBe(204);
  await expect(successfulRow).toHaveCount(0);
  expect((await context.request.get(`/api/v2/runs/${activityId}`)).status()).toBe(404);
  await testInfo.attach("final-url", {
    body: page.url(),
    contentType: "text/plain",
  });
});

test("fails closed when a shoe pair has no reviewer bridge", async ({ page }) => {
  await page.goto("/shoes/puma-deviate-pure-nitro");
  await expect(page.getByRole("heading", { name: "Deviate Pure NITRO" })).toBeVisible();

  const referenceShoes = page.getByRole("combobox", { name: "รองเท้าที่คุณใส่อยู่" });
  await referenceShoes.click();
  await referenceShoes.fill("Camel Carbon 5K");
  await page.getByRole("option", { name: /Carbon 5K/i }).click();
  await page.getByRole("button", { name: "เทียบไซซ์" }).click();

  const result = page.getByTestId("shoe-size-result");
  await expect(result).toContainText("ยังไม่มีข้อมูลตรงพอให้เทียบคู่นี้");
  await expect(result).toContainText("จะไม่เดา");
  await expect(page.getByTestId("shoe-size-evidence")).toContainText("ไม่มี reviewer bridge");
});
