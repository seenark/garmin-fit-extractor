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
    expect(validateFiles([])).toEqual(["Select at least one ZIP file."]);
    expect(
      validateFiles(Array.from({ length: 11 }, (_, index) => file(`${index}.zip`))),
    ).toEqual(["Select at most 10 ZIP files."]);
  });

  test("rejects invalid suffix, control characters, oversized files, and UTF-8 names over 255 bytes", () => {
    expect(validateFiles([file("activity.gpx")])).toEqual([
      "activity.gpx must end with .zip.",
    ]);
    expect(validateFiles([file("bad\u0000.zip")])).toEqual([
      "bad\u0000.zip contains an invalid control character.",
    ]);
    expect(validateFiles([file("large.zip", 20 * 1024 * 1024 + 1)])).toEqual([
      "large.zip is larger than the 20-megabyte upload limit.",
    ]);
    expect(validateFiles([file(`${"é".repeat(128)}.zip`)])).toEqual([
      `${"é".repeat(128)}.zip has a file name longer than 255 bytes.`,
    ]);
  });
});
