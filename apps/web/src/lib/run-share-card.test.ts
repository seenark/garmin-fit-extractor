import { expect, test } from "bun:test";
import { pinRunShareInput, formatRunShareMetric, inspectRunSharePhoto, loadRunSharePhoto } from "./run-share-card";

test("PNG pins summary revision and uses timer time without inventing missing metrics", () => {
  const run = {
    revisionId: "revision-one", startTime: "2026-10-05T06:00:00Z",
    summary: { distanceMeters: 10001, timerTimeSeconds: 3661.4, averagePaceSecondsPerKm: 366.1, averageHeartRateBpm: null, averagePowerWatts: null },
  };
  const pinned = pinRunShareInput(run);
  run.revisionId = "revision-two";
  run.summary.distanceMeters = 20000;
  expect(pinned.revisionId).toBe("revision-one");
  expect(formatRunShareMetric("distance", pinned.summary.distanceMeters).value).toBe("10");
  expect(formatRunShareMetric("timer", pinned.summary.timerTimeSeconds)).toEqual({ value: "1:01:01", unit: "h:mm:ss", label: "เวลาจับเวลา · Timer time" });
  expect(formatRunShareMetric("pace", 359.6).value).toBe("6:00");
  expect(formatRunShareMetric("heartRate", null).value).toBe("—");
  expect(formatRunShareMetric("power", Number.NaN).value).toBe("—");
  expect(formatRunShareMetric("pace", -1).value).toBe("—");
});

test("local photo boundary rejects wrong content, malformed containers and bombs before decoding", async () => {
  const png = Uint8Array.from(Buffer.from("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+aPfoAAAAASUVORK5CYII=", "base64"));
  expect(inspectRunSharePhoto(png, "image/png")).toEqual({ type: "image/png", width: 1, height: 1 });
  expect(() => inspectRunSharePhoto(png, "image/jpeg")).toThrow("ชนิด");
  expect(() => inspectRunSharePhoto(new TextEncoder().encode("<svg width='1' height='1'></svg>"), "image/png")).toThrow("JPEG");
  expect(() => inspectRunSharePhoto(png.slice(0, 33), "image/png")).toThrow("ไม่สมบูรณ์");
  const bomb = png.slice();
  new DataView(bomb.buffer).setUint32(16, 24_000_001);
  await expect(loadRunSharePhoto(new File([bomb], "local.png", { type: "image/png" }))).rejects.toThrow("24");
  await expect(loadRunSharePhoto(new File([new Uint8Array(10 * 1024 * 1024 + 1)], "local.png", { type: "image/png" }))).rejects.toThrow("10");
});

test("WebP cannot hide oversized encoded dimensions behind small canvas header", () => {
  const bytes = new Uint8Array(44);
  const view = new DataView(bytes.buffer);
  bytes.set(new TextEncoder().encode("RIFF"), 0);
  view.setUint32(4, 36, true);
  bytes.set(new TextEncoder().encode("WEBPVP8X"), 8);
  view.setUint32(16, 10, true);
  bytes.set(new TextEncoder().encode("VP8L"), 30);
  view.setUint32(34, 5, true);
  bytes[38] = 0x2f;
  view.setUint32(39, 5000 | (5000 << 14), true);
  expect(() => inspectRunSharePhoto(bytes, "image/webp")).toThrow("ไม่สมบูรณ์");
});

test("extreme finite quantities stay readable without dropping units or turning unknown into zero", () => {
  expect(formatRunShareMetric("distance", 1e308)).toEqual({ value: "1E305", unit: "km", label: "ระยะทาง · Distance" });
  expect(formatRunShareMetric("power", 1e308)).toEqual({ value: "1E308", unit: "W", label: "กำลังเฉลี่ย · Average power" });
  expect(formatRunShareMetric("heartRate", 0).value).toBe("0");
});
