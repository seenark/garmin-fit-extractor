import { expect, test } from "@playwright/test";
import { readFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";

const activityArchive = fileURLToPath(
  new URL("../../api/tests/fixtures/activity.zip", import.meta.url),
);

test("authenticates, uploads ZIP members, copies raw JSON, and isolates history", async ({
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
  await expect(
    page.getByRole("heading", { name: "ดูประวัติการวิ่งของคุณ" }),
  ).toBeVisible();
  await expect(
    page.getByRole("heading", { name: "เข้าสู่ระบบเพื่อไปต่อ" }),
  ).toBeVisible();
  await page.goto("/upload");
  await expect(
    page.getByRole("heading", {
      name: "ยังไม่มีไฟล์ ZIP? ดาวน์โหลดจาก Garmin Connect ตามนี้ได้เลย",
    }),
  ).toBeVisible();
  await expect(
    page.getByRole("heading", {
      name: "อ่าน guide ได้ก่อน โดยยังไม่ต้องเข้าสู่ระบบ",
    }),
  ).toBeVisible();
  await expect(page.getByTestId("upload-dropzone")).toHaveCount(0);
  await expect(
    page.getByRole("button", { name: "เข้าสู่ระบบเพื่ออัปโหลด" }),
  ).toBeVisible();

  await page.goto("/api/v1/auth/test-login?user=alice");
  await expect(page.getByText("alice", { exact: true })).toBeVisible();
  await expect(
    page.getByRole("heading", {
      name: "เรื่องวิ่งของคุณ มีอะไรให้ดูมากกว่าที่คิด",
    }),
  ).toBeVisible();
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
  await page.getByTestId("upload-submit").click();

  const batchResults = page.getByTestId("batch-result");
  await expect(batchResults).toHaveCount(5);
  await expect(batchResults.nth(0)).toContainText("สำเร็จ");
  await expect(batchResults.nth(1)).toContainText("ไม่สำเร็จ");
  await expect(batchResults.nth(0)).toContainText("activity.zip::activity.fit");
  await expect(batchResults.nth(2)).toContainText("สำเร็จ");
  await expect(batchResults.nth(3)).toContainText("สำเร็จ");
  await expect(batchResults.nth(4)).toContainText("ไม่สำเร็จ");

  const successfulResult = batchResults.nth(0);
  await successfulResult.getByRole("link", { name: "ดูรายละเอียด" }).click();
  await expect(page).toHaveURL(/\/extractions\/[0-9a-f-]{36}(?:\?.*)?$/i);
  await expect(page.getByRole("tab", { name: "วิเคราะห์" })).toBeVisible();
  await expect(page.getByTestId("activity-chart-pace")).toBeVisible();
  await expect(
    page.getByRole("img", { name: "กราฟเพซต่อรอบ" }),
  ).toBeVisible();

  const copyActions = page.getByTestId("json-copy-actions");
  const tablist = page.getByRole("tablist", { name: "มุมมองข้อมูล" });
  await expect(copyActions).toBeVisible();
  await expect(page.getByTestId("copy-normalized-json")).toBeVisible();
  await expect(page.getByTestId("copy-raw-json")).toBeVisible();
  const copyActionsBox = await copyActions.boundingBox();
  const tablistBox = await tablist.boundingBox();
  expect(copyActionsBox).not.toBeNull();
  expect(tablistBox).not.toBeNull();
  expect(copyActionsBox!.y + copyActionsBox!.height).toBeLessThanOrEqual(
    tablistBox!.y,
  );

  await page.getByTestId("copy-normalized-json").click();
  await expect(page.getByTestId("copy-normalized-json")).toHaveText("คัดลอกแล้ว");
  const copiedNormalized = await page.evaluate(() => navigator.clipboard.readText());
  expect(JSON.parse(copiedNormalized)).toMatchObject({ schemaVersion: "1.0.0" });

  const normalizedDownload = page.waitForEvent("download");
  await page.getByRole("button", { name: "ดาวน์โหลด JSON แบบวิเคราะห์" }).click();
  const normalizedPath = await (await normalizedDownload).path();
  expect(normalizedPath).not.toBeNull();
  expect(JSON.parse(await readFile(normalizedPath!, "utf8"))).toMatchObject({
    schemaVersion: "1.0.0",
  });

  await page.getByRole("tab", { name: "ข้อมูลดิบ" }).click();
  await expect(page.getByTestId("raw-json-view")).toContainText('"kind"');
  await page.getByRole("button", { name: "คัดลอก Raw JSON" }).click();
  await expect(page.getByRole("button", { name: "คัดลอกแล้ว" })).toBeVisible();
  const copiedRaw = await page.evaluate(() => navigator.clipboard.readText());
  expect(copiedRaw).toBe(
    await page.getByTestId("raw-json-view").textContent(),
  );
  await page.evaluate(() => {
    Object.defineProperty(navigator, "clipboard", {
      configurable: true,
      value: {
        writeText: async () => {
          throw new Error("clipboard permission denied");
        },
      },
    });
  });
  await page.getByRole("button", { name: "คัดลอกแล้ว" }).click();
  await expect(page.getByRole("alert")).toContainText("คัดลอกไม่สำเร็จ");
  await expect(page.getByTestId("raw-json-view")).toContainText('"kind"');
  expect(JSON.parse(copiedRaw)).toEqual(expect.any(Array));

  const rawDownload = page.waitForEvent("download");
  await page.getByRole("button", { name: "ดาวน์โหลด Raw JSON" }).click();
  const rawPath = await (await rawDownload).path();
  expect(rawPath).not.toBeNull();
  expect(JSON.parse(await readFile(rawPath!, "utf8"))).toEqual(
    expect.any(Array),
  );

  await page.getByRole("link", { name: "Runs", exact: true }).click();
  await expect(page).toHaveURL(/\/history(?:\?.*)?$/);
  const historyTable = page.getByTestId("history-table");
  await expect(historyTable).toContainText("activity.zip::activity.fit");
  await expect(historyTable).toContainText("corrupt.zip");
  await expect(historyTable).toContainText("วันที่กิจกรรม");
  await expect(historyTable).toContainText("ประเภทกิจกรรม");

  await page.getByLabel("เรียงลำดับ").selectOption("asc");
  await expect(page).toHaveURL(/\/history\?.*order=asc/);
  const successfulRow = historyTable.locator("tr").filter({
    hasText: "activity.zip::activity.fit",
  });
  await expect(successfulRow.locator("td").nth(2)).not.toHaveText("กิจกรรม");
  await expect(successfulRow.locator("td").nth(3)).not.toHaveText("กิจกรรม");
  await successfulRow.getByRole("link", { name: "เปิดดู" }).click();
  await expect(page).toHaveURL(/\/extractions\/[0-9a-f-]{36}\?.*order=asc/i);
  await page.getByRole("link", { name: "กลับไปประวัติ", exact: true }).click();
  await expect(page.getByLabel("เรียงลำดับ")).toHaveValue("asc");
  await successfulRow
    .getByRole("button", {
      name: "ลบ activity.zip::activity.fit",
    })
    .click();
  const confirmation = page.getByTestId("confirm-delete");
  await expect(confirmation).toBeVisible();
  await confirmation.getByRole("button", { name: "ลบรายการ", exact: true }).click();
  await expect(historyTable).not.toContainText("activity.zip::activity.fit");
  await expect(historyTable).toContainText("corrupt.zip");

  await page.getByRole("button", { name: "ออกจากระบบ" }).click();
  await expect(
    page.getByRole("button", { name: "เข้าสู่ระบบด้วย Google" }),
  ).toBeVisible();
  await page.goto("/api/v1/auth/test-login?user=bob");
  await expect(page.getByText("bob", { exact: true })).toBeVisible();
  await page.getByRole("link", { name: "Runs", exact: true }).click();
  await expect(page.getByText("ยังไม่มีไฟล์ที่อัปโหลด")).toBeVisible();

  await page.getByRole("button", { name: "ออกจากระบบ" }).click();
  await page.goto("/api/v1/auth/test-login?user=alice");
  await expect(page.getByText("alice", { exact: true })).toBeVisible();
  await page.getByRole("link", { name: "Runs", exact: true }).click();
  await expect(historyTable).toContainText("corrupt.zip");
  await page.getByRole("button", { name: "ล้างประวัติ" }).click();
  await expect(confirmation).toBeVisible();
  const clearResponse = page.waitForResponse(
    (response) =>
      new URL(response.url()).pathname === "/api/v1/extractions" &&
      response.request().method() === "DELETE",
  );
  const refreshedHistory = page.waitForResponse(
    (response) =>
      new URL(response.url()).pathname === "/api/v1/extractions" &&
      response.request().method() === "GET",
  );
  await confirmation
    .getByRole("button", { name: "ล้างประวัติ", exact: true })
    .click();
  expect((await clearResponse).status()).toBe(204);
  expect(await (await refreshedHistory).json()).toMatchObject({ total: 0 });

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
