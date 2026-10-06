import { expect, test } from "@playwright/test";
import { readFile } from "node:fs/promises";

test("native PNG keeps alpha, geometry, metric values and private fields off download", async ({ page }) => {
  await page.goto("/");
  const result = await page.evaluate(async () => {
    // Browser realm must load Vite's transformed module, not Node's test import.
    const url = "/src/lib/run-share-card.ts";
    const share = await import(url);
    const run = share.pinRunShareInput({ revisionId: "one", startTime: "2026-10-05T06:00:00Z", summary: { distanceMeters: 10001, timerTimeSeconds: 3661.4, averagePaceSecondsPerKm: 359.6, averageHeartRateBpm: null, averagePowerWatts: null }, fileName: "PRIVATE.fit" });
    const options = { theme: "transparent", size: "portrait", layout: "bottom-left", metrics: ["distance", "timer", "pace", "heartRate", "power"], includeDate: false, includeName: false, name: "PRIVATE NAME", cropX: .5, cropY: .5 };
    const canvas = await share.renderRunShareCard(run, options);
    document.body.replaceChildren(canvas);
    const context = canvas.getContext("2d")!;
    const pixels = context.getImageData(0, 0, canvas.width, canvas.height).data;
    let ink = 0;
    for (let index = 3; index < pixels.length; index += 4) if (pixels[index]) ink++;
    const blob = await share.runShareCanvasToPng(canvas);
    const bitmap = await createImageBitmap(blob);
    const bytes = new Uint8Array(await blob.arrayBuffer());
    const privateChanged = await share.renderRunShareCard({ ...run, startTime: "2020-01-01T00:00:00Z" }, { ...options, name: "Another private name" });
    const otherBytes = new Uint8Array(await (await share.runShareCanvasToPng(privateChanged)).arrayBuffer());
    const privateStable = bytes.length === otherBytes.length && bytes.every((byte, index) => byte === otherBytes[index]);
    const output = { width: bitmap.width, height: bitmap.height, alpha: context.getImageData(0, 0, 1, 1).data[3], ink, signature: Array.from(bytes.slice(0, 8)), type: blob.type, metrics: share.runShareMetrics(run, options.metrics), privateStable, snapshotKeys: Object.keys(run).sort() };
    bitmap.close();
    const button = document.createElement("button");
    button.textContent = "Download PNG";
    button.onclick = () => share.downloadRunSharePng(blob);
    document.body.append(button);
    return output;
  });
  expect(result.width).toBe(1080);
  expect(result.height).toBe(1920);
  expect(result.alpha).toBe(0);
  expect(result.ink).toBeGreaterThan(1000);
  expect(result.ink).toBeLessThan(1080 * 1920 / 2);
  expect(result.signature).toEqual([137, 80, 78, 71, 13, 10, 26, 10]);
  expect(result.type).toBe("image/png");
  expect(result.metrics.map((metric: { value: string }) => metric.value)).toEqual(["10", "1:01:01", "6:00", "—", "—"]);
  expect(result.privateStable).toBe(true);
  expect(result.snapshotKeys).toEqual(["revisionId", "startTime", "summary"]);
  const download = page.waitForEvent("download");
  await page.getByRole("button", { name: "Download PNG", exact: true }).click();
  const file = await download;
  expect(file.suggestedFilename()).toBe("run-card.png");
  expect(await file.failure()).toBeNull();
  const path = await file.path();
  expect(path).not.toBeNull();
  const bytes = await readFile(path!);
  expect(Array.from(bytes.subarray(0, 8))).toEqual([137, 80, 78, 71, 13, 10, 26, 10]);
  expect(bytes.readUInt32BE(16)).toBe(1080);
  expect(bytes.readUInt32BE(20)).toBe(1920);
  const chunks: string[] = [];
  for (let offset = 8; offset < bytes.length; offset += bytes.readUInt32BE(offset) + 12) chunks.push(bytes.toString("ascii", offset + 4, offset + 8));
  for (const privateChunk of ["eXIf", "tEXt", "iTXt", "zTXt"]) expect(chunks).not.toContain(privateChunk);
});

test("all PNG layouts and themes support local raster, orientation and crop without metadata", async ({ page }) => {
  await page.goto("/");
  const result = await page.evaluate(async () => {
    // Load public Canvas boundary inside browser realm via Vite transformation.
    const moduleUrl = "/src/lib/run-share-card.ts";
    const share = await import(moduleUrl);
    const source = document.createElement("canvas");
    source.width = 640; source.height = 320;
    const context = source.getContext("2d")!;
    context.fillStyle = "#ff0000"; context.fillRect(0, 0, 320, 320);
    context.fillStyle = "#0000ff"; context.fillRect(320, 0, 320, 320);
    const photos: ImageBitmap[] = [];
    for (const type of ["image/png", "image/jpeg", "image/webp"]) {
      const { promise, resolve } = Promise.withResolvers<Blob>();
      source.toBlob((blob) => resolve(blob!), type);
      photos.push(await share.loadRunSharePhoto(new File([await promise], "local", { type })));
    }
    const { promise, resolve } = Promise.withResolvers<Blob>();
    source.toBlob((blob) => resolve(blob!), "image/jpeg");
    const jpeg = new Uint8Array(await (await promise).arrayBuffer());
    // APP1 EXIF orientation 6. Browser must rotate before crop; export carries no EXIF.
    const exif = Uint8Array.from([255,225,0,34,69,120,105,102,0,0,73,73,42,0,8,0,0,0,1,0,18,1,3,0,1,0,0,0,6,0,0,0,0,0,0,0]);
    const oriented = await share.loadRunSharePhoto(new File([jpeg.slice(0, 2), exif, jpeg.slice(2)], "rotated", { type: "image/jpeg" }));
    const run = { revisionId: "pinned", startTime: "2026-10-05T06:00:00Z", summary: { distanceMeters: 987654321, timerTimeSeconds: 987654321, averagePaceSecondsPerKm: 359.6, averageHeartRateBpm: null, averagePowerWatts: 300 } };
    const base = { theme: "light", size: "square", layout: "bottom-left", metrics: ["distance", "timer", "pace", "heartRate", "power"], includeDate: true, includeName: true, name: "การวิ่งระยะยาวมาก Long English name ".repeat(16), cropX: .5, cropY: .5 };
    const rows: { width: number; height: number; alpha: number; expectedAlpha: number }[] = [];
    for (const photo of [null, ...photos]) for (const size of ["square", "portrait"]) for (const theme of ["light", "dark", "transparent"]) for (const layout of ["bottom-left", "lower-center", "bottom-right"]) {
      const canvas = await share.renderRunShareCard(run, { ...base, size, theme, layout }, photo);
      const blob = await share.runShareCanvasToPng(canvas);
      const bitmap = await createImageBitmap(blob);
      rows.push({ width: bitmap.width, height: bitmap.height, alpha: canvas.getContext("2d")!.getImageData(0, 0, 1, 1).data[3]!, expectedAlpha: !photo && theme === "transparent" ? 0 : 255 });
      bitmap.close();
    }
    const left = await share.renderRunShareCard(run, { ...base, cropX: 0 }, photos[0]);
    const right = await share.renderRunShareCard(run, { ...base, cropX: 1 }, photos[0]);
    const crop = [left, right].map((canvas) => Array.from(canvas.getContext("2d")!.getImageData(540, 20, 1, 1).data));
    const dimensions = [oriented.width, oriented.height];
    for (const photo of [...photos, oriented]) photo.close();
    return { rows, dimensions, crop };
  });
  expect(result.dimensions).toEqual([320, 640]);
  for (const row of result.rows) {
    expect(row.width).toBe(1080);
    expect([1080, 1920]).toContain(row.height);
    expect(row.alpha).toBe(row.expectedAlpha);
  }
  expect(result.crop).toEqual([[255, 0, 0, 255], [0, 0, 255, 255]]);
});

test("share editor preserves choices and pinned revision, and cancels pending local photo safely", async ({ page }) => {
  await page.goto("/");
  await page.evaluate(async () => {
    // Vite chooses runtime module URLs. Mount actual component without fake HTTP APIs.
    const refreshUrl = "/@react-refresh";
    const refresh = await import(refreshUrl);
    refresh.default.injectIntoGlobalHook(window);
    Object.assign(window, { $RefreshReg$: () => {}, $RefreshSig$: () => (type: unknown) => type, __vite_plugin_react_preamble_installed__: true });
    const componentUrl = "/src/components/run-share-card.tsx";
    const entry = await (await fetch("/src/main.tsx")).text();
    const source = await (await fetch(componentUrl)).text();
    const domUrl = entry.match(/from "([^"]*react-dom_client[^"]*)"/)![1]!;
    const reactUrl = source.match(/from "([^"]*\/react\.js[^"]*)"/)![1]!;
    const [dom, react, component] = await Promise.all([import(domUrl), import(reactUrl), import(componentUrl)]);
    const host = document.createElement("main");
    document.body.replaceChildren(host);
    const root = dom.default.createRoot(host);
    const run = { revisionId: "one", startTime: null, summary: { distanceMeters: 10000, timerTimeSeconds: 3600, averagePaceSecondsPerKm: 360, averageHeartRateBpm: null, averagePowerWatts: null } };
    const update = (revisionId: string, distanceMeters: number) => root.render(react.default.createElement(component.RunShareCard, { run: { ...run, revisionId, summary: { ...run.summary, distanceMeters } } }));
    Object.assign(window, { updateShareProof: update });
    update("one", 10000);
  });
  await expect(page.getByRole("button", { name: "ดาวน์โหลด PNG", exact: true })).toBeEnabled();
  await page.getByLabel("หัวใจเฉลี่ย", { exact: true }).check();
  await page.getByRole("combobox", { name: "ตำแหน่งตัวเลข", exact: true }).selectOption("bottom-right");
  await expect(page.getByLabel("หัวใจเฉลี่ย", { exact: true })).toBeChecked();
  await page.evaluate(() => Reflect.get(window, "updateShareProof")("two", 22000));
  await expect(page.locator(".runs-share-values")).toContainText("10 km");
  await page.getByRole("button", { name: "ใช้ revision ล่าสุด", exact: true }).click();
  await expect(page.locator(".runs-share-values")).toContainText("22 km");
  await page.evaluate(async () => {
    const canvas = document.createElement("canvas"); canvas.width = 4; canvas.height = 4;
    const { promise, resolve } = Promise.withResolvers<Blob>();
    canvas.toBlob((value) => resolve(value!));
    const file = new File([await promise], "photo.png", { type: "image/png" });
    const transfer = new DataTransfer(); transfer.items.add(file);
    const input = document.querySelector<HTMLInputElement>('input[type="file"]')!;
    input.files = transfer.files; input.dispatchEvent(new Event("change", { bubbles: true }));
  });
  await expect(page.getByRole("button", { name: "ลบภาพพื้นหลัง", exact: true })).toBeVisible();
  await page.evaluate(async () => {
    const canvas = document.createElement("canvas"); canvas.width = 4; canvas.height = 4;
    const { promise, resolve } = Promise.withResolvers<Blob>();
    canvas.toBlob((blob) => resolve(blob!));
    const file = new File([await promise], "pending.png", { type: "image/png" });
    const bytes = await file.arrayBuffer();
    const pending = Promise.withResolvers<ArrayBuffer>();
    // Delay only local file reading to exercise user's remove-while-reading transition.
    file.arrayBuffer = () => pending.promise;
    Object.assign(window, { completeSharePhotoRead: () => pending.resolve(bytes) });
    const transfer = new DataTransfer(); transfer.items.add(file);
    const input = document.querySelector<HTMLInputElement>('input[type="file"]')!;
    input.files = transfer.files; input.dispatchEvent(new Event("change", { bubbles: true }));
  });
  await expect(page.getByText("กำลังอ่านภาพ…", { exact: true })).toBeVisible();
  await page.getByRole("button", { name: "ลบภาพพื้นหลัง", exact: true }).click();
  await page.evaluate(() => Reflect.get(window, "completeSharePhotoRead")());
  await expect(page.getByText("กำลังอ่านภาพ…", { exact: true })).not.toBeVisible();
  await expect(page.getByLabel("เลือกภาพจากเครื่อง", { exact: true })).toBeEnabled();
  await expect(page.getByRole("button", { name: "ดาวน์โหลด PNG", exact: true })).toBeEnabled();
});
