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
      timeInPowerZone: [300, 900, 1200, 900, 240, 100, 10],
    },
  ],
  hrZoneMesgs: [
    { messageIndex: 0, lowBpm: 100 },
    { messageIndex: 1, lowBpm: 120 },
    { messageIndex: 2, lowBpm: 140 },
    { messageIndex: 3, lowBpm: 160 },
    { messageIndex: 4, lowBpm: 180 },
  ],
  powerZoneMesgs: [
    { messageIndex: 0, highValue: 150 },
    { messageIndex: 1, highValue: 220 },
    { messageIndex: 2, highValue: 280 },
    { messageIndex: 3, highValue: 340 },
    { messageIndex: 4, highValue: 410 },
    { messageIndex: 5, highValue: 500 },
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
  recordMesgs: [
    {
      timestamp: new Date("2026-07-19T00:00:00.000Z"),
      heartRate: 140,
      power: 0,
    },
    {
      timestamp: new Date("2026-07-19T00:00:01.000Z"),
      heartRate: 145,
      power: 245,
    },
    {
      timestamp: new Date("2026-07-19T00:00:02.000Z"),
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
  assert.equal(result.power.zones[0]?.minWatts, 0);
  assert.equal(result.power.zones[0]?.maxWatts, 150);
  assert.equal(result.power.zones[6]?.minWatts, 501);
  assert.equal(result.power.zones[6]?.maxWatts, null);
  assert.equal(result.samples.length, 2);
  assert.equal(result.samples[0]?.elapsedSeconds, 0);
  assert.equal(result.samples[0]?.powerWatts, 0);
  assert.equal(result.samples[1]?.elapsedSeconds, 1);
  assert.equal(result.samples[1]?.heartRateBpm, 145);
  assert.equal(result.samples[1]?.powerWatts, 245);
  assert.equal(result.laps[0]?.pace.value, 350);
});
