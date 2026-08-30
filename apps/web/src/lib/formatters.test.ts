import { describe, expect, test } from "bun:test";

import {
  formatCadence,
  formatCalories,
  formatDuration,
  formatFileSize,
  formatHeartRate,
  formatMetric,
  formatPace,
  formatPower,
  formatTemperature,
} from "./formatters";

describe("human-readable formatters", () => {
  test("turns durations and paces into readable values", () => {
    expect(formatDuration(312)).toBe("5 minutes 12 seconds");
    expect(formatDuration(3_661)).toBe("1 hour 1 minute 1 second");
    expect(formatPace(312)).toBe("5:12 minutes per kilometer");
  });

  test("expands common activity units", () => {
    expect(formatHeartRate(142)).toBe("142 beats per minute");
    expect(formatCadence(176)).toBe("176 steps per minute");
    expect(formatPower(245)).toBe("245 watts");
    expect(formatTemperature(18.5)).toBe("18.5 degrees Celsius");
    expect(formatCalories(450)).toBe("450 calories");
  });

  test("formats normalized metrics without exposing API unit names", () => {
    expect(formatMetric({ value: 5_000, unit: "meters" })).toBe("5,000 meters");
    expect(formatMetric({ value: 312, unit: "seconds_per_kilometer" })).toBe(
      "5:12 minutes per kilometer",
    );
    expect(formatMetric({ value: 68, unit: "percent" })).toBe("68 percent");
    expect(formatMetric({ value: 2, unit: "custom_unit" })).toBe("2 custom unit");
    expect(formatMetric({ value: null, unit: "seconds" })).toBe("Not available");
  });

  test("uses words instead of binary file-size abbreviations", () => {
    expect(formatFileSize(1_024)).toBe("1 kilobyte");
    expect(formatFileSize(1_536)).toBe("1.5 kilobytes");
    expect(formatFileSize(20 * 1_024 * 1_024)).toBe("20 megabytes");
  });
});
