import { describe, expect, test } from "bun:test";

import { validateFiles } from "./upload-validation";

function file(name: string, size = 1): File {
  return new File([new Uint8Array(size)], name, { type: "application/octet-stream" });
}

describe("validateFiles", () => {
  test("accepts one through ten .zip files under the server limits", () => {
    expect(validateFiles([file("run.ZIP"), file("ride.zip")])).toEqual([]);
    expect(
      validateFiles(Array.from({ length: 10 }, (_, index) => file(`${index}.zip`))),
    ).toEqual([]);
  });

  test("rejects empty and over-limit selections", () => {
    expect(validateFiles([])).toEqual(["เลือกไฟล์ ZIP อย่างน้อย 1 ไฟล์"]);
    expect(
      validateFiles(Array.from({ length: 11 }, (_, index) => file(`${index}.zip`))),
    ).toEqual(["เลือกได้ไม่เกิน 10 ไฟล์ ZIP"]);
  });

  test("rejects invalid suffix, control characters, oversized files, and UTF-8 names over 255 bytes", () => {
    expect(validateFiles([file("activity.gpx")])).toEqual([
      "activity.gpx ต้องลงท้ายด้วย .zip",
    ]);
    expect(validateFiles([file("bad\u0000.zip")])).toEqual([
      "bad\u0000.zip มีอักขระควบคุมที่ใช้ไม่ได้",
    ]);
    expect(validateFiles([file("large.zip", 20 * 1024 * 1024 + 1)])).toEqual([
      "large.zip มีขนาดเกิน 20 เมกะไบต์",
    ]);
    expect(validateFiles([file(`${"é".repeat(128)}.zip`)])).toEqual([
      `${"é".repeat(128)}.zip ยาวเกิน 255 ไบต์`,
    ]);
  });
});
