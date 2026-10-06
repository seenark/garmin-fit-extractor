import { describe, expect, test } from "bun:test";
import type { NormalizedRun, RunSample } from "./runs-types";
import { buildRunChartData, inspectRunSample } from "./run-chart-data";

function sample(index: number, overrides: Partial<RunSample> = {}): RunSample {
  return { index, timestamp: null, elapsedSeconds: index, speedMps: 4, paceSecondsPerKm: 250, heartRateBpm: 150, powerWatts: 300, cadenceStepsPerMinute: null, altitudeMeters: null, distanceMeters: null, sourceReferences: {}, ...overrides };
}
function run(samples: RunSample[]): Pick<NormalizedRun, "samples" | "laps" | "timerEvents"> {
  return { samples, laps: [], timerEvents: [] };
}

describe("normalized run chart consumer", () => {
  test("breaks unavailable metrics, zero-speed pace, timer pauses and elapsed gaps without changing inspection values", () => {
    const source = run([
      sample(0), sample(1, { heartRateBpm: null, powerWatts: 0 }),
      sample(2, { speedMps: 0, paceSecondsPerKm: null }),
      sample(3, { timerRunning: false }), sample(4), sample(5, { elapsedSeconds: 30 }),
    ]);
    const chart = buildRunChartData(source);
    expect(chart.rows.map((row) => [row.seconds, row.pace, row.heartRate, row.power])).toEqual([
      [0, 250, 150, 300], [1, 250, null, 0], [2, null, 150, 300],
      [3, null, null, null], [4, 250, 150, 300], [30, null, null, null], [30, 250, 150, 300],
    ]);
    expect(inspectRunSample(chart, 1)).toBe(source.samples[1]);
    expect(inspectRunSample(chart, 30)?.index).toBe(5);
    expect(source.samples[3]?.heartRateBpm).toBe(150);
  });
  test("downsampling retains source extrema, lap and detected boundaries, missing transitions and original inspection", () => {
    const source = run(Array.from({ length: 100 }, (_, index) => sample(index, {
      powerWatts: index === 42 ? 999 : 300,
      heartRateBpm: index === 54 ? null : 150,
    })));
    source.laps = [{ index: 0, startTime: null, endTime: null, startElapsedSeconds: 0, endElapsedSeconds: 37.5, summary: { distanceMeters: null, timerTimeSeconds: null, elapsedTimeSeconds: null, movingTimeSeconds: null, averageSpeedMps: null, averagePaceSecondsPerKm: null, averageHeartRateBpm: null, averagePowerWatts: null, averageCadenceStepsPerMinute: null }, sourceReferences: [] }];
    const segment = { index: 0, kind: "steady", startElapsedSeconds: 61.5, endElapsedSeconds: 80.5, durationSeconds: 19, timeBasis: "elapsed" as const, version: "test", features: {}, eligibility: { lt1: { accepted: false, reasons: ["RR_MISSING"] }, lt2: { accepted: false, reasons: ["RR_MISSING"] } } };
    const chart = buildRunChartData(source, [segment], 20);
    const positions = chart.rows.map((row) => row.sourcePosition);
    for (const index of [0, 99, 37, 38, 42, 53, 54, 55, 61, 62, 80, 81]) expect(positions).toContain(index);
    expect(chart.metadata.originalCount).toBe(100);
    expect(chart.metadata.displayCount).toBeLessThan(100);
    expect(inspectRunSample(chart, 43.1)).toBe(source.samples[43]);
    expect(source.samples).toHaveLength(100);
  });
  test("timer events split paths even when no sample is recorded during the pause; untimed records also split paths", () => {
    const source = run([sample(0), sample(1, { elapsedSeconds: 8 }), sample(2, { elapsedSeconds: null }), sample(3, { elapsedSeconds: 9 })]);
    source.timerEvents = [
      { index: 0, timestamp: null, elapsedSeconds: 2, event: 0, eventType: 4, sourceReferences: [] },
      { index: 1, timestamp: null, elapsedSeconds: 6, event: 0, eventType: 0, sourceReferences: [] },
    ];
    const chart = buildRunChartData(source);
    expect(chart.rows.map((row) => [row.seconds, row.sourcePosition])).toEqual([[0, 0], [8, null], [8, 1], [9, null], [9, 3]]);
    expect(chart.metadata.untimedCount).toBe(1);
    expect(inspectRunSample(chart, 8)?.index).toBe(1);
  });
  test("every native timer stop enum breaks a short pause even without a paused sample", () => {
    for (const eventType of [1, 4, 8, 9]) {
      const source = run([sample(0), sample(1, { elapsedSeconds: 8 })]);
      source.timerEvents = [
        { index: 0, timestamp: null, elapsedSeconds: 2, event: 0, eventType, sourceReferences: [] },
        { index: 1, timestamp: null, elapsedSeconds: 6, event: 0, eventType: 0, sourceReferences: [] },
      ];
      const chart = buildRunChartData(source);
      expect(chart.rows.map(row => [row.seconds, row.sourcePosition, row.heartRate])).toEqual([[0, 0, 150], [8, null, null], [8, 1, 150]]);
      expect(inspectRunSample(chart, 8)).toBe(source.samples[1]);
    }
  });
  test("pointer inspection preserves distinct native records with the same elapsed time", () => {
    const source = run([
      sample(0, { heartRateBpm: 100, powerWatts: 200 }),
      sample(1, { elapsedSeconds: 8, heartRateBpm: 120, powerWatts: 240 }),
      sample(2, { elapsedSeconds: 8, heartRateBpm: 180, powerWatts: 360 }),
      sample(3, { elapsedSeconds: 10, heartRateBpm: 160, powerWatts: 300 }),
    ]);
    const chart = buildRunChartData(source);
    expect(inspectRunSample(chart, 8, "heartRate", 179)).toBe(source.samples[2]);
    expect(inspectRunSample(chart, 8, "power", 241)).toBe(source.samples[1]);
    expect(chart.inspectable.map(item => item.index)).toEqual([0, 1, 2, 3]);
    expect(source.samples.map(item => item.elapsedSeconds)).toEqual([0, 8, 8, 10]);
  });
  test("native unknown timer states do not fabricate a restart or hide recorded values", () => {
    const source = run([sample(0), sample(1, { elapsedSeconds: 3, timerRunning: null }), sample(2, { elapsedSeconds: 4, timerRunning: null })]);
    source.timerEvents = [
      { index: 0, timestamp: null, elapsedSeconds: 0, event: 0, eventType: 0, sourceReferences: [] },
      { index: 1, timestamp: null, elapsedSeconds: 2, event: 0, eventType: 1, sourceReferences: [] },
      { index: 2, timestamp: null, elapsedSeconds: 3, event: 0, eventType: 253, sourceReferences: [] },
    ];
    const chart = buildRunChartData(source);
    expect(chart.rows.map(row => [row.seconds, row.sourcePosition, row.heartRate])).toEqual([[0, 0, 150], [3, null, null], [3, 1, 150], [4, 2, 150]]);
    expect(inspectRunSample(chart, 3)?.timerRunning).toBeNull();
  });
});
