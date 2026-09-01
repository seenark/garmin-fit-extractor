import { describe, expect, test } from "bun:test";

import type { Analysis } from "./api-types";
import {
  buildElevationData,
  buildHeartRateZoneData,
  buildLapHeartRateData,
  buildLapPowerData,
  buildLapPaceData,
  buildPowerData,
  buildPowerZoneData,
} from "./activity-chart-data";

const metric = (value: number | null, unit: string) => ({ value, unit });

const analysis: Analysis = {
  schemaVersion: "1.0.0",
  source: { fileName: "activity.fit" },
  activity: { type: "running", subType: "road", date: null },
  summary: {
    duration: metric(3_600, "seconds"),
    movingTime: metric(3_540, "seconds"),
    distance: metric(10_000, "meters"),
    calories: { value: 700, unit: "kcal" },
  },
  heartRate: {
    averageBpm: 150,
    maximumBpm: 180,
    zones: [
      { zone: 1, minBpm: 100, maxBpm: 119, durationSeconds: 0 },
      { zone: 2, minBpm: 120, maxBpm: 139, durationSeconds: null },
      { zone: 3, minBpm: 140, maxBpm: 159, durationSeconds: 900 },
    ],
  },
  pace: {
    average: metric(360, "seconds_per_kilometer"),
    moving: metric(350, "seconds_per_kilometer"),
    best: metric(300, "seconds_per_kilometer"),
  },
  power: {
    averageWatts: 250,
    maximumWatts: 410,
    zones: [
      { zone: 1, minWatts: 0, maxWatts: 150, durationSeconds: 30 },
      { zone: 2, minWatts: 151, maxWatts: 220, durationSeconds: null },
      { zone: 7, minWatts: 501, maxWatts: null, durationSeconds: 5 },
    ],
  },
  runningDynamics: {
    cadence: { averageStepsPerMinute: 176, maximumStepsPerMinute: 188 },
    strideLength: metric(1.2, "meters"),
    groundContactTime: metric(210, "milliseconds"),
    verticalOscillation: metric(8, "millimeters"),
    verticalRatio: metric(7, "percent"),
  },
  elevation: {
    ascent: metric(120, "meters"),
    descent: metric(0, "meters"),
  },
  temperature: {
    averageCelsius: 18,
    minimumCelsius: 15,
    maximumCelsius: 21,
  },
  samples: [
    {
      index: 0,
      timestamp: "2026-08-31T06:00:00.000Z",
      elapsedSeconds: 0,
      heartRateBpm: 140,
      powerWatts: 0,
    },
    {
      index: 1,
      timestamp: "2026-08-31T06:00:01.000Z",
      elapsedSeconds: 1,
      heartRateBpm: 145,
      powerWatts: 245,
    },
    {
      index: 2,
      timestamp: null,
      elapsedSeconds: null,
      heartRateBpm: null,
      powerWatts: 300,
    },
    {
      index: 3,
      timestamp: "2026-08-31T06:00:03.000Z",
      elapsedSeconds: 3,
      heartRateBpm: null,
      powerWatts: null,
    },
  ],
  laps: [
    {
      index: 1,
      startTime: null,
      distance: metric(1_000, "meters"),
      duration: metric(360, "seconds"),
      movingTime: metric(350, "seconds"),
      pace: metric(360, "seconds_per_kilometer"),
      heartRate: { averageBpm: 145, maximumBpm: 155 },
      power: { averageWatts: 245, maximumWatts: 300 },
      cadence: { averageStepsPerMinute: 174, maximumStepsPerMinute: 182 },
    },
    {
      index: 2,
      startTime: null,
      distance: metric(1_000, "meters"),
      duration: metric(370, "seconds"),
      movingTime: metric(365, "seconds"),
      pace: metric(null, "seconds_per_kilometer"),
      heartRate: { averageBpm: null, maximumBpm: 160 },
      power: { averageWatts: null, maximumWatts: null },
      cadence: { averageStepsPerMinute: null, maximumStepsPerMinute: null },
    },
    {
      index: 3,
      startTime: null,
      distance: metric(1_000, "meters"),
      duration: metric(355, "seconds"),
      movingTime: metric(350, "seconds"),
      pace: metric(355, "seconds_per_kilometer"),
      heartRate: { averageBpm: 150, maximumBpm: null },
      power: { averageWatts: Number.NaN, maximumWatts: 320 },
      cadence: { averageStepsPerMinute: 178, maximumStepsPerMinute: null },
    },
  ],
};

describe("activity chart data", () => {
  test("keeps ordered lap pace points and omits missing or invalid values", () => {
    expect(buildLapPaceData(analysis.laps)).toEqual([
      { lap: 1, paceSecondsPerKm: 360 },
      { lap: 3, paceSecondsPerKm: 355 },
    ]);
  });

  test("keeps zero-duration zones but omits zones without duration", () => {
    expect(buildHeartRateZoneData(analysis.heartRate.zones)).toEqual([
      { zone: 1, durationSeconds: 0, minBpm: 100, maxBpm: 119 },
      { zone: 3, durationSeconds: 900, minBpm: 140, maxBpm: 159 },
    ]);
  });

  test("keeps power-zone thresholds and omits zones without duration", () => {
    expect(buildPowerZoneData(analysis.power.zones)).toEqual([
      { zone: 1, durationSeconds: 30, minWatts: 0, maxWatts: 150 },
      { zone: 7, durationSeconds: 5, minWatts: 501, maxWatts: null },
    ]);
  });

  test("keeps timestamped power samples including zero watts", () => {
    expect(buildPowerData(analysis.samples)).toEqual([
      { seconds: 0, watts: 0 },
      { seconds: 1, watts: 245 },
    ]);
  });

  test("builds a lap-power fallback from either valid power series", () => {
    expect(buildLapPowerData(analysis.laps)).toEqual([
      { lap: 1, averageWatts: 245, maximumWatts: 300 },
      { lap: 3, averageWatts: null, maximumWatts: 320 },
    ]);
  });

  test("keeps a lap when either heart-rate series has a valid value", () => {
    expect(buildLapHeartRateData(analysis.laps)).toEqual([
      { lap: 1, averageBpm: 145, maximumBpm: 155 },
      { lap: 2, averageBpm: null, maximumBpm: 160 },
      { lap: 3, averageBpm: 150, maximumBpm: null },
    ]);
  });

  test("returns ascent and descent only when the activity has elevation data", () => {
    expect(buildElevationData(analysis.elevation)).toEqual([
      { direction: "ascent", meters: 120 },
      { direction: "descent", meters: 0 },
    ]);
    expect(
      buildElevationData({ ascent: metric(0, "meters"), descent: metric(0, "meters") }),
    ).toEqual([]);
  });
});
