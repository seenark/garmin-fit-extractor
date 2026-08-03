import { describe, expect, test } from "bun:test";

import { validateFiles } from "./upload-validation";

function file(name: string, size = 1): File {
  return new File([new Uint8Array(size)], name, { type: "application/octet-stream" });
}

describe("validateFiles", () => {
  test("accepts one through ten .fit files under the server limits", () => {
    expect(validateFiles([file("run.FIT"), file("ride.fit")])).toEqual([]);
    expect(validateFiles(Array.from({ length: 10 }, (_, index) => file(`${index}.fit`)))).toEqual([]);
  });

  test("rejects empty and over-limit selections", () => {
    expect(validateFiles([])).toEqual(["Select at least one FIT file."]);
    expect(validateFiles(Array.from({ length: 11 }, (_, index) => file(`${index}.fit`)))).toEqual([
      "Select at most 10 FIT files.",
    ]);
  });

  test("rejects invalid suffix, control characters, oversized files, and UTF-8 names over 255 bytes", () => {
    expect(validateFiles([file("activity.gpx")])).toEqual(["activity.gpx must end with .fit."]);
    expect(validateFiles([file("bad\u0000.fit")])).toEqual(["bad\u0000.fit contains an invalid control character."]);
    expect(validateFiles([file("large.fit", 20 * 1024 * 1024 + 1)])).toEqual([
      "large.fit exceeds the 20 MiB limit.",
    ]);
    expect(validateFiles([file(`${"é".repeat(128)}.fit`)])).toEqual([
      `${"é".repeat(128)}.fit exceeds the 255-byte file-name limit.`,
    ]);
  });
});
