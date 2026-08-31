import { describe, expect, test } from "bun:test";

import { formatApiError } from "./copy";

describe("formatApiError", () => {
  test("uses Thai copy for known API errors without changing their codes", () => {
    expect(
      formatApiError({ code: "INVALID_ZIP", message: "File is not a valid ZIP archive." }),
    ).toBe("ไฟล์นี้ไม่ใช่ ZIP ที่ถูกต้อง หรือไฟล์เสียหาย");
  });

  test("falls back to a Thai message for unknown errors", () => {
    expect(formatApiError({ code: "UNKNOWN", message: "unexpected English" })).toBe(
      "ระบบทำรายการนี้ไม่สำเร็จ ลองใหม่อีกครั้ง",
    );
  });
});
