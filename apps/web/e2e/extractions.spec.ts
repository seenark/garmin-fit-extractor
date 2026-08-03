import { expect, test } from "@playwright/test";
import { readFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";

const activityFixture = fileURLToPath(
  new URL("../../api/tests/fixtures/activity.fit", import.meta.url),
);

test("uploads mixed FIT files, views both JSON representations, and manages history", async ({
  page,
}, testInfo) => {
  const activityBytes = await readFile(activityFixture);

  await page.goto("/");

  const uploadInput = page
    .getByTestId("upload-dropzone")
    .locator('input[type="file"]');
  await uploadInput.setInputFiles([
    {
      name: "activity.fit",
      mimeType: "application/octet-stream",
      buffer: activityBytes,
    },
    {
      name: "corrupt.fit",
      mimeType: "application/octet-stream",
      buffer: Buffer.from("not a FIT file"),
    },
  ]);

  await expect(page.getByTestId("selected-file")).toHaveCount(2);
  await page.getByTestId("upload-submit").click();

  const batchResults = page.getByTestId("batch-result");
  await expect(batchResults).toHaveCount(2);
  await expect(batchResults.nth(0)).toContainText("Succeeded");
  await expect(batchResults.nth(1)).toContainText("Failed");

  const successfulResult = batchResults.nth(0);
  await successfulResult.getByRole("link", { name: "View details" }).click();
  await expect(page).toHaveURL(/\/extractions\/[0-9a-f-]{36}$/i);

  await expect(page.getByRole("tab", { name: "Summary" })).toBeVisible();

  const normalizedDownload = page.waitForEvent("download");
  await page.getByRole("button", { name: "Download normalized JSON" }).click();
  const normalizedPath = await (await normalizedDownload).path();
  expect(normalizedPath).not.toBeNull();
  expect(JSON.parse(await readFile(normalizedPath!, "utf8"))).toMatchObject({
    schemaVersion: "1.0.0",
  });

  await page.getByRole("tab", { name: "Raw" }).click();
  await expect(page.getByTestId("raw-json-view")).toContainText('"kind"');

  const rawDownload = page.waitForEvent("download");
  await page.getByRole("button", { name: "Download raw JSON" }).click();
  const rawPath = await (await rawDownload).path();
  expect(rawPath).not.toBeNull();
  expect(JSON.parse(await readFile(rawPath!, "utf8"))).toEqual(
    expect.any(Array),
  );

  await page.getByRole("link", { name: "History", exact: true }).click();
  await expect(page).toHaveURL(/\/history(?:\?.*)?$/);
  const historyTable = page.getByTestId("history-table");
  await expect(historyTable).toContainText("activity.fit");
  await expect(historyTable).toContainText("corrupt.fit");

  const successfulRow = historyTable.locator("tr").filter({
    hasText: "activity.fit",
  });
  await successfulRow.getByRole("link", { name: "Open" }).click();
  await expect(page).toHaveURL(/\/extractions\/[0-9a-f-]{36}$/i);
  await expect(page.getByRole("tab", { name: "Summary" })).toBeVisible();

  await page.getByRole("link", { name: "History", exact: true }).click();
  await successfulRow
    .getByRole("button", { name: "Delete activity.fit" })
    .click();
  const confirmation = page.getByTestId("confirm-delete");
  await expect(confirmation).toBeVisible();
  await confirmation.getByRole("button", { name: "Delete", exact: true }).click();
  await expect(historyTable).not.toContainText("activity.fit");
  await expect(historyTable).toContainText("corrupt.fit");

  await page.getByRole("button", { name: "Clear history" }).click();
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
    .getByRole("button", { name: "Clear history", exact: true })
    .click();
  expect((await clearResponse).status()).toBe(204);
  expect(await (await refreshedHistory).json()).toMatchObject({ total: 0 });
  await page.reload();
  await expect(page.getByText("No uploads yet.")).toBeVisible();

  await testInfo.attach("final-url", {
    body: page.url(),
    contentType: "text/plain",
  });
});
