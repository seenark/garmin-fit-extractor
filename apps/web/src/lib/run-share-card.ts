import { formatNumber, formatPaceTick } from "./formatters";

const scientificNumber = new Intl.NumberFormat("th-TH", { notation: "scientific", maximumFractionDigits: 2 });

function formatShareQuantity(value: number): string {
  return value >= 1e9 ? scientificNumber.format(value) : formatNumber(value);
}

export type RunShareMetric = "distance" | "timer" | "pace" | "heartRate" | "power";
export type RunShareInput = {
  revisionId: string | null;
  startTime: string | null;
  summary: {
    distanceMeters: number | null;
    timerTimeSeconds: number | null;
    averagePaceSecondsPerKm: number | null;
    averageHeartRateBpm: number | null;
    averagePowerWatts: number | null;
  };
};

export function pinRunShareInput(run: RunShareInput): Readonly<RunShareInput> {
  return Object.freeze({ revisionId: run.revisionId, startTime: run.startTime, summary: Object.freeze({
    distanceMeters: run.summary.distanceMeters,
    timerTimeSeconds: run.summary.timerTimeSeconds,
    averagePaceSecondsPerKm: run.summary.averagePaceSecondsPerKm,
    averageHeartRateBpm: run.summary.averageHeartRateBpm,
    averagePowerWatts: run.summary.averagePowerWatts,
  }) });
}

export function formatRunTimer(value: number | null): string {
  if (value === null || !Number.isFinite(value) || value < 0) return "—";
  const seconds = Math.round(value);
  return `${Math.floor(seconds / 3600)}:${String(Math.floor(seconds % 3600 / 60)).padStart(2, "0")}:${String(seconds % 60).padStart(2, "0")}`;
}

export function formatRunShareMetric(metric: RunShareMetric, value: number | null): { value: string; unit: string; label: string } {
  const valid = value !== null && Number.isFinite(value) && value >= 0;
  switch (metric) {
    case "distance": return { value: valid ? formatShareQuantity(value / 1000) : "—", unit: "km", label: "ระยะทาง · Distance" };
    case "timer": return { value: formatRunTimer(value), unit: "h:mm:ss", label: "เวลาจับเวลา · Timer time" };
    case "pace": return { value: valid ? formatPaceTick(value) : "—", unit: "min/km", label: "เพซเฉลี่ย · Average pace" };
    case "heartRate": return { value: valid ? formatShareQuantity(Math.round(value)) : "—", unit: "bpm", label: "หัวใจเฉลี่ย · Average HR" };
    case "power": return { value: valid ? formatShareQuantity(Math.round(value)) : "—", unit: "W", label: "กำลังเฉลี่ย · Average power" };
  }
}

export const RUN_SHARE_PHOTO_MAX_BYTES = 10 * 1024 * 1024;
export const RUN_SHARE_PHOTO_MAX_PIXELS = 24_000_000;
export type RunSharePhotoInfo = { type: "image/png" | "image/jpeg" | "image/webp"; width: number; height: number };

/** Read container dimensions before any browser image decoder allocates pixels. */
export function inspectRunSharePhoto(bytes: Uint8Array, mime: string): RunSharePhotoInfo {
  const malformed = () => new Error("ไฟล์ภาพไม่สมบูรณ์ หรือรูปแบบไม่รองรับ");
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  const ascii = (offset: number, count: number) => String.fromCharCode(...bytes.subarray(offset, offset + count));
  let info: RunSharePhotoInfo | undefined;
  if (bytes.length >= 33 && ascii(0, 8) === "\u0089PNG\r\n\u001a\n") {
    if (view.getUint32(8) !== 13 || ascii(12, 4) !== "IHDR") throw malformed();
    let offset = 8;
    let data = false;
    let end = false;
    while (offset + 12 <= bytes.length) {
      const length = view.getUint32(offset);
      if (length > bytes.length - offset - 12) throw malformed();
      const kind = ascii(offset + 4, 4);
      if (kind === "IDAT") data = true;
      offset += length + 12;
      if (kind === "IEND") {
        if (length !== 0 || offset !== bytes.length) throw malformed();
        end = true;
        break;
      }
    }
    if (!data || !end) throw malformed();
    info = { type: "image/png", width: view.getUint32(16), height: view.getUint32(20) };
  } else if (bytes.length >= 4 && bytes[0] === 0xff && bytes[1] === 0xd8) {
    let offset = 2;
    let width = 0;
    let height = 0;
    let scan = false;
    while (offset + 4 <= bytes.length) {
      if (bytes[offset++] !== 0xff) throw malformed();
      while (bytes[offset] === 0xff) offset++;
      const marker = bytes[offset++];
      if (marker === undefined || marker === 0 || marker === 0xd9) throw malformed();
      if (marker === 0x01 || (marker >= 0xd0 && marker <= 0xd7)) continue;
      if (offset + 2 > bytes.length) throw malformed();
      const length = view.getUint16(offset);
      if (length < 2 || length > bytes.length - offset) throw malformed();
      if ([0xc0, 0xc1, 0xc2].includes(marker)) {
        if (width || length < 8 || length !== 8 + 3 * bytes[offset + 7]!) throw malformed();
        height = view.getUint16(offset + 3);
        width = view.getUint16(offset + 5);
      }
      if (marker === 0xda) { scan = true; break; }
      offset += length;
    }
    if (!scan || bytes.at(-2) !== 0xff || bytes.at(-1) !== 0xd9) throw malformed();
    info = { type: "image/jpeg", width, height };
  } else if (bytes.length >= 20 && ascii(0, 4) === "RIFF" && ascii(8, 4) === "WEBP") {
    if (view.getUint32(4, true) + 8 !== bytes.length) throw malformed();
    let offset = 12;
    let width = 0;
    let height = 0;
    let image = false;
    while (offset + 8 <= bytes.length) {
      const kind = ascii(offset, 4);
      const length = view.getUint32(offset + 4, true);
      const start = offset + 8;
      if (length > bytes.length - start) throw malformed();
      if (kind === "VP8X") {
        if (length !== 10 || width || image) throw malformed();
        if ((bytes[start]! & 2) !== 0) throw new Error("ไม่รองรับภาพเคลื่อนไหว");
        width = 1 + (bytes[start + 4]! | bytes[start + 5]! << 8 | bytes[start + 6]! << 16);
        height = 1 + (bytes[start + 7]! | bytes[start + 8]! << 8 | bytes[start + 9]! << 16);
      } else if (kind === "VP8 ") {
        if (image || length < 10 || (bytes[start]! & 1) !== 0 || ascii(start + 3, 3) !== "\u009d\u0001\u002a") throw malformed();
        const encodedWidth = view.getUint16(start + 6, true) & 0x3fff;
        const encodedHeight = view.getUint16(start + 8, true) & 0x3fff;
        if (width && (width !== encodedWidth || height !== encodedHeight)) throw malformed();
        width = encodedWidth;
        height = encodedHeight;
        image = true;
      } else if (kind === "VP8L") {
        if (image || length < 5 || bytes[start] !== 0x2f) throw malformed();
        const bits = view.getUint32(start + 1, true);
        const encodedWidth = (bits & 0x3fff) + 1;
        const encodedHeight = ((bits >>> 14) & 0x3fff) + 1;
        if (width && (width !== encodedWidth || height !== encodedHeight)) throw malformed();
        width = encodedWidth;
        height = encodedHeight;
        image = true;
      }
      offset = start + length + (length % 2);
    }
    if (!image || offset !== bytes.length) throw malformed();
    info = { type: "image/webp", width, height };
  }
  if (!info) throw new Error("เลือกภาพ JPEG, PNG หรือ WebP เท่านั้น ไม่รองรับ SVG/HTML");
  if (mime && mime !== info.type) throw new Error("ชนิดไฟล์ไม่ตรงกับข้อมูลภาพ");
  if (!info.width || !info.height) throw malformed();
  if (info.width * info.height > RUN_SHARE_PHOTO_MAX_PIXELS) throw new Error("ภาพต้องไม่เกิน 24 ล้านพิกเซล");
  return info;
}

export async function loadRunSharePhoto(file: File): Promise<ImageBitmap> {
  if (file.size > RUN_SHARE_PHOTO_MAX_BYTES) throw new Error("ภาพต้องไม่เกิน 10 MiB");
  const info = inspectRunSharePhoto(new Uint8Array(await file.arrayBuffer()), file.type);
  let bitmap: ImageBitmap;
  try {
    // Native decoder applies EXIF orientation; Canvas exports only new raster pixels.
    bitmap = await createImageBitmap(file, { imageOrientation: "from-image" });
  } catch {
    throw new Error("อ่านภาพไม่สำเร็จ เลือกไฟล์ JPEG, PNG หรือ WebP ที่สมบูรณ์");
  }
  if (!((bitmap.width === info.width && bitmap.height === info.height) ||
    (bitmap.width === info.height && bitmap.height === info.width))) {
    bitmap.close();
    throw new Error("ขนาดภาพที่ถอดรหัสไม่ตรงกับไฟล์");
  }
  return bitmap;
}

export type RunShareOptions = {
  theme: "light" | "dark" | "transparent";
  size: "square" | "portrait";
  layout: "bottom-left" | "lower-center" | "bottom-right";
  metrics: readonly RunShareMetric[];
  includeDate: boolean;
  includeName: boolean;
  name: string;
  cropX: number;
  cropY: number;
};

export function runShareMetrics(run: RunShareInput, metrics: readonly RunShareMetric[]) {
  const values = { distance: run.summary.distanceMeters, timer: run.summary.timerTimeSeconds,
    pace: run.summary.averagePaceSecondsPerKm, heartRate: run.summary.averageHeartRateBpm,
    power: run.summary.averagePowerWatts };
  return [...new Set(metrics)].map((metric) => formatRunShareMetric(metric, values[metric]));
}

function drawFittedText(context: CanvasRenderingContext2D, bodyFont: string, text: string, x: number, y: number, width: number, size: number, weight = 500, truncate = false) {
  context.font = `${weight} ${size}px ${bodyFont}`;
  const measured = context.measureText(text).width;
  if (measured > width) {
    if (truncate) {
      const graphemes = Array.from(new Intl.Segmenter("th", { granularity: "grapheme" }).segment(text), (part) => part.segment);
      while (graphemes.length && context.measureText(`${graphemes.join("")}…`).width > width) graphemes.pop();
      text = `${graphemes.join("")}…`;
    } else {
      size *= width / measured;
      context.font = `${weight} ${size}px ${bodyFont}`;
    }
  }
  context.fillText(text, x, y);
}

export async function renderRunShareCard(run: RunShareInput, options: RunShareOptions, photo?: ImageBitmap | null): Promise<HTMLCanvasElement> {
  // Copy before awaiting fonts. A new server revision cannot alter this render.
  const pinned = pinRunShareInput(run);
  const settings = { ...options, metrics: [...options.metrics] };
  if (!pinned.revisionId) throw new Error("ยังไม่มี revision สำหรับภาพนี้");
  if (settings.metrics.length === 0) throw new Error("เลือกอย่างน้อยหนึ่งค่า");
  const tokens = getComputedStyle(document.documentElement);
  const bodyFont = tokens.getPropertyValue("--font-body").trim();
  const paper = tokens.getPropertyValue("--color-paper").trim();
  const ink = tokens.getPropertyValue("--color-ink").trim();
  const code = tokens.getPropertyValue("--color-code").trim();
  const codeInk = tokens.getPropertyValue("--color-code-ink").trim();
  if (![bodyFont, paper, ink, code, codeInk].every(Boolean)) throw new Error("โหลดรูปแบบภาพไม่สำเร็จ ลองโหลดหน้าใหม่");
  await Promise.all([document.fonts.load(`500 30px ${bodyFont}`, "วิ่ง Run"), document.fonts.load(`700 70px ${bodyFont}`, "วิ่ง Run")]);
  await document.fonts.ready;
  const canvas = document.createElement("canvas");
  canvas.width = 1080;
  canvas.height = settings.size === "square" ? 1080 : 1920;
  const context = canvas.getContext("2d");
  if (!context) throw new Error("เบราว์เซอร์นี้ไม่รองรับ Canvas");
  const dark = settings.theme === "dark";
  if (settings.theme !== "transparent") {
    context.fillStyle = dark ? code : paper;
    context.fillRect(0, 0, canvas.width, canvas.height);
  }
  if (photo) {
    const scale = Math.max(canvas.width / photo.width, canvas.height / photo.height);
    const width = photo.width * scale;
    const height = photo.height * scale;
    context.drawImage(photo, (canvas.width - width) * Math.min(1, Math.max(0, settings.cropX)),
      (canvas.height - height) * Math.min(1, Math.max(0, settings.cropY)), width, height);
  }
  const metrics = runShareMetrics(pinned, settings.metrics);
  const center = settings.layout === "lower-center";
  const right = settings.layout === "bottom-right";
  const rowHeight = center ? 160 : 136;
  const rows = center ? Math.ceil(metrics.length / 2) : metrics.length;
  const metadata: string[] = [];
  if (settings.includeName && settings.name.trim()) metadata.push(settings.name.trim());
  if (settings.includeDate && pinned.startTime) {
    const date = new Date(pinned.startTime);
    if (!Number.isNaN(date.getTime())) metadata.push(new Intl.DateTimeFormat("th-TH", { dateStyle: "medium", timeZone: "UTC" }).format(date) + " · UTC");
  }
  const bottom = canvas.height - 84;
  const top = bottom - rows * rowHeight - (metadata.length ? metadata.length * 46 + 28 : 0);
  if (photo) {
    // Local panel guarantees contrast regardless of crop. No panel in alpha/no-photo mode.
    context.save();
    context.globalAlpha = .94;
    context.fillStyle = dark || settings.theme === "transparent" ? code : paper;
    context.fillRect(40, top - 36, 1000, bottom - top + 72);
    context.restore();
  }
  context.fillStyle = dark || (photo && settings.theme === "transparent") ? codeInk : ink;
  context.textBaseline = "alphabetic";
  context.textAlign = center ? "center" : right ? "right" : "left";
  const anchor = center ? 540 : right ? 996 : 84;
  metadata.forEach((text, index) => drawFittedText(context, bodyFont, text, anchor, top + index * 46, 912, 30, 500, true));
  const metricTop = top + (metadata.length ? metadata.length * 46 + 28 : 0);
  metrics.forEach((metric, index) => {
    const row = center ? Math.floor(index / 2) : index;
    const column = center ? index % 2 : 0;
    const lastCentered = center && metrics.length % 2 === 1 && index === metrics.length - 1;
    const x = center ? lastCentered ? 540 : 300 + column * 480 : anchor;
    const y = metricTop + row * rowHeight;
    const maxWidth = center ? lastCentered ? 880 : 420 : 880;
    drawFittedText(context, bodyFont, metric.label, x, y + 30, maxWidth, 26);
    drawFittedText(context, bodyFont, `${metric.value} ${metric.unit}`, x, y + 104, maxWidth, center ? 54 : right ? 62 : 70, 700);
  });
  return canvas;
}

export function runShareCanvasToPng(canvas: HTMLCanvasElement): Promise<Blob> {
  const { promise, resolve, reject } = Promise.withResolvers<Blob>();
  try {
    canvas.toBlob((blob) => {
      if (!blob || blob.type !== "image/png") reject(new Error("สร้าง PNG ไม่สำเร็จ ลองอีกครั้ง"));
      else resolve(blob);
    }, "image/png");
  } catch {
    reject(new Error("สร้าง PNG ไม่สำเร็จ ลองอีกครั้ง"));
  }
  return promise;
}

export function downloadRunSharePng(blob: Blob): void {
  if (blob.type !== "image/png") throw new Error("ไฟล์ที่สร้างไม่ใช่ PNG");
  const url = URL.createObjectURL(blob);
  const link = document.createElement("a");
  link.href = url;
  link.download = "run-card.png";
  document.body.append(link);
  try { link.click(); } finally {
    link.remove();
    // Keep URL alive until browser has accepted the download, then release it.
    window.setTimeout(() => URL.revokeObjectURL(url), 1000);
  }
}
