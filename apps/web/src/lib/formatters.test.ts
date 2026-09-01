import { describe, expect, test } from "bun:test";

import {
  formatCadence,
  formatCalories,
  formatDuration,
  formatFileSize,
  formatActivityType,
  formatHeartRate,
  formatMetric,
  formatPace,
  formatPaceTick,
  formatPower,
  formatPowerRange,
  formatTemperature,
} from "./formatters";

describe("human-readable formatters", () => {
  test("turns durations and paces into readable values", () => {
    expect(formatDuration(312)).toBe("5 นาที 12 วินาที");
    expect(formatDuration(3_661)).toBe("1 ชั่วโมง 1 นาที 1 วินาที");
    expect(formatPace(312)).toBe("5:12 นาทีต่อกิโลเมตร");
  });

  test("formats pace chart ticks as minutes per kilometer", () => {
    expect(formatPaceTick(312)).toBe("5:12");
    expect(formatPaceTick(360)).toBe("6:00");
    expect(formatPaceTick(null)).toBe("—");
  });

  test("expands common activity units", () => {
    expect(formatHeartRate(142)).toBe("142 ครั้งต่อนาที");
    expect(formatCadence(176)).toBe("176 ก้าวต่อนาที");
    expect(formatPower(245)).toBe("245 วัตต์");
    expect(formatTemperature(18.5)).toBe("18.5 องศาเซลเซียส");
    expect(formatCalories(450)).toBe("450 กิโลแคลอรี");
  });

  test("formats power-zone ranges with open-ended boundaries", () => {
    expect(formatPowerRange(151, 220)).toBe("151–220 วัตต์");
    expect(formatPowerRange(501, null)).toBe("ตั้งแต่ 501 วัตต์");
    expect(formatPowerRange(null, null)).toBe("ไม่มีข้อมูล");
  });

  test("formats normalized metrics without exposing API unit names", () => {
    expect(formatMetric({ value: 5_000, unit: "meters" })).toBe("5,000 เมตร");
    expect(formatMetric({ value: 312, unit: "seconds_per_kilometer" })).toBe(
      "5:12 นาทีต่อกิโลเมตร",
    );
    expect(formatMetric({ value: 68, unit: "percent" })).toBe("68 เปอร์เซ็นต์");
    expect(formatMetric({ value: 2, unit: "custom_unit" })).toBe("2 หน่วย (custom unit)");
    expect(formatMetric({ value: null, unit: "seconds" })).toBe("ไม่มีข้อมูล");
    expect(formatActivityType("running")).toBe("วิ่ง");
  });

  test("uses words instead of binary file-size abbreviations", () => {
    expect(formatFileSize(1_024)).toBe("1 กิโลไบต์");
    expect(formatFileSize(1_536)).toBe("1.5 กิโลไบต์");
    expect(formatFileSize(20 * 1_024 * 1_024)).toBe("20 เมกะไบต์");
  });
});
