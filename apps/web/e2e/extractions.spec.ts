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
  await expect(
    page.getByRole("button", { name: "Continue with Google" }),
  ).toBeVisible();
  await page.goto("/history");
  await expect(
    page.getByRole("button", { name: "Continue with Google" }),
  ).toBeVisible();

  await page.goto("/api/v1/auth/test-login?user=alice");
  await expect(page.getByText("alice", { exact: true })).toBeVisible();

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
  await expect(batchResults.nth(0)).toContainText("Succeeded");
  await expect(batchResults.nth(1)).toContainText("Failed");
  await expect(batchResults.nth(0)).toContainText("activity.zip::activity.fit");
  await expect(batchResults.nth(2)).toContainText("Succeeded");
  await expect(batchResults.nth(3)).toContainText("Succeeded");
  await expect(batchResults.nth(4)).toContainText("Failed");

  const successfulResult = batchResults.nth(0);
  await successfulResult.getByRole("link", { name: "View details" }).click();
  await expect(page).toHaveURL(/\/extractions\/[0-9a-f-]{36}(?:\?.*)?$/i);
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
  await page.getByRole("button", { name: "Copy raw JSON" }).click();
  await expect(page.getByRole("button", { name: "Copied" })).toBeVisible();
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
  await page.getByRole("button", { name: "Copied" }).click();
  await expect(page.getByRole("alert")).toContainText("Copy failed");
  await expect(page.getByTestId("raw-json-view")).toContainText('"kind"');
  expect(JSON.parse(copiedRaw)).toEqual(expect.any(Array));

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
  await expect(historyTable).toContainText("activity.zip::activity.fit");
  await expect(historyTable).toContainText("corrupt.zip");
  await expect(historyTable).toContainText("Date");
  await expect(historyTable).toContainText("Exercise");

  await page.getByLabel("Order").selectOption("asc");
  await expect(page).toHaveURL(/\/history\?.*order=asc/);
  const successfulRow = historyTable.locator("tr").filter({
    hasText: "activity.zip::activity.fit",
  });
  await expect(successfulRow.locator("td").nth(2)).not.toHaveText("Unknown");
  await expect(successfulRow.locator("td").nth(3)).not.toHaveText("Unknown");
  await successfulRow.getByRole("link", { name: "Open" }).click();
  await expect(page).toHaveURL(/\/extractions\/[0-9a-f-]{36}\?.*order=asc/i);
  await page.getByRole("link", { name: "Back to history", exact: true }).click();
  await expect(page.getByLabel("Order")).toHaveValue("asc");
  await successfulRow
    .getByRole("button", {
      name: "Delete activity.zip::activity.fit",
    })
    .click();
  const confirmation = page.getByTestId("confirm-delete");
  await expect(confirmation).toBeVisible();
  await confirmation.getByRole("button", { name: "Delete", exact: true }).click();
  await expect(historyTable).not.toContainText("activity.zip::activity.fit");
  await expect(historyTable).toContainText("corrupt.zip");

  await page.getByRole("button", { name: "Sign out" }).click();
  await expect(
    page.getByRole("button", { name: "Continue with Google" }),
  ).toBeVisible();
  await page.goto("/api/v1/auth/test-login?user=bob");
  await expect(page.getByText("bob", { exact: true })).toBeVisible();
  await page.getByRole("link", { name: "History", exact: true }).click();
  await expect(page.getByText("No uploads yet.")).toBeVisible();

  await page.getByRole("button", { name: "Sign out" }).click();
  await page.goto("/api/v1/auth/test-login?user=alice");
  await expect(page.getByText("alice", { exact: true })).toBeVisible();
  await page.getByRole("link", { name: "History", exact: true }).click();
  await expect(historyTable).toContainText("corrupt.zip");
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

  await testInfo.attach("final-url", {
    body: page.url(),
    contentType: "text/plain",
  });
});
