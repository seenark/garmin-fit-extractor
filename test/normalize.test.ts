import assert from "node:assert/strict";
import test from "node:test";
import { normalizeFitMessages } from "../src/normalize.js";

const decoded = {
  sessionMesgs: [
    {
      sport: "running",
      subSport: "road",
      startTime: new Date("2026-07-19T00:00:00.000Z"),
      totalElapsedTime: 3600,
      totalTimerTime: 3540,
      totalDistance: 10000,
      totalCalories: 700,
      avgHeartRate: 150,
      maxHeartRate: 180,
      enhancedAvgSpeed: 2.824858757,
      enhancedMaxSpeed: 4,
      avgPower: 250,
      maxPower: 410,
      avgRunningCadence: 85,
      maxRunningCadence: 95,
      totalAscent: 40,
      totalDescent: 38,
      timeInHrZone: [300, 900, 1200, 900, 240],
    },
  ],
  hrZoneMesgs: [
    { messageIndex: 0, lowBpm: 100 },
    { messageIndex: 1, lowBpm: 120 },
    { messageIndex: 2, lowBpm: 140 },
    { messageIndex: 3, lowBpm: 160 },
    { messageIndex: 4, lowBpm: 180 },
  ],
  lapMesgs: [
    {
      startTime: new Date("2026-07-19T00:00:00.000Z"),
      totalDistance: 1000,
      totalElapsedTime: 360,
      totalTimerTime: 350,
      avgHeartRate: 145,
      maxHeartRate: 155,
      avgPower: 245,
      maxPower: 300,
      avgRunningCadence: 84,
    },
  ],
};

test("normalizes decoded FIT messages into the stable schema", () => {
  const result = normalizeFitMessages(decoded, "activity.fit");

  assert.equal(result.schemaVersion, "1.0.0");
  assert.equal(result.activity.type, "running");
  assert.equal(result.summary.distance.value, 10000);
  assert.equal(result.pace.average.value, 354);
  assert.equal(result.runningDynamics.cadence.averageStepsPerMinute, 170);
  assert.equal(result.heartRate.zones[0]?.maxBpm, 119);
  assert.equal(result.laps[0]?.pace.value, 350);
});
