import { describe, expect, test } from "bun:test";

import { validateFiles } from "./upload-validation";

function file(name: string, size = 1): File {
  return new File([new Uint8Array(size)], name, { type: "application/octet-stream" });
}

describe("validateFiles", () => {
  test("accepts direct FIT and ZIP suffixes case-insensitively within count and size boundaries", () => {
    expect(validateFiles([file("run.FiT"), file("archive.ZiP")])).toEqual([]);
    expect(
      validateFiles(Array.from({ length: 10 }, (_, index) => file(`${index}.fit`))),
    ).toEqual([]);
    expect(validateFiles([file("boundary.fit", 20 * 1024 * 1024)])).toEqual([]);
  });

  test("rejects missing selections and batches above ten files", () => {
    expect(validateFiles([])).not.toEqual([]);
    expect(
      validateFiles(Array.from({ length: 11 }, (_, index) => file(`${index}.zip`))),
    ).not.toEqual([]);
  });

  test("rejects empty files, unsupported suffixes, and files above 20 MiB", () => {
    for (const invalid of [
      file("empty.fit", 0),
      file("empty.zip", 0),
      file("activity.gpx"),
      file("activity.fit.exe"),
      file("large.zip", 20 * 1024 * 1024 + 1),
    ]) {
      expect(validateFiles([invalid])).not.toEqual([]);
    }
  });

  test("enforces filename controls and 255-byte UTF-8 boundary", () => {
    expect(validateFiles([file(`${"é".repeat(125)}a.fit`)])).toEqual([]);
    for (const name of ["bad\u0000.fit", "bad\u007f.zip", `${"é".repeat(126)}.fit`]) {
      expect(validateFiles([file(name)])).not.toEqual([]);
    }
  });
});
