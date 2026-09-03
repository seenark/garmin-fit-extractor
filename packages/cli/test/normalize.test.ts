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

test("maps custom Garmin HR buckets from session-level boundaries", () => {
  const result = normalizeFitMessages(
    {
      sessionMesgs: [{ hrCalcType: "custom" }],
      timeInZoneMesgs: [
        {
          referenceMesg: "session",
          referenceIndex: 0,
          hrCalcType: "custom",
          hrZoneHighBoundary: [120, 136, 151, 166, 181, 204],
          timeInHrZone: [57.204, 145, 1393.968, 785.242, 178.992, 0, 0],
        },
      ],
      hrZoneMesgs: [],
      powerZoneMesgs: [],
      lapMesgs: [],
      recordMesgs: [],
    },
    "activity.fit",
  );

  assert.equal(result.heartRate.calculationType, "custom");
  assert.deepEqual(result.heartRate.zones, [
    {
      bucketIndex: 0,
      label: "Below Z1",
      mappingState: "mapped",
      zone: null,
      zoneCount: 5,
      lowerBoundBpm: null,
      upperBoundBpmExclusive: 120,
      durationSeconds: 57.2,
    },
    {
      bucketIndex: 1,
      label: "Z1",
      mappingState: "mapped",
      zone: 1,
      zoneCount: 5,
      lowerBoundBpm: 120,
      upperBoundBpmExclusive: 136,
      durationSeconds: 145,
    },
    {
      bucketIndex: 2,
      label: "Z2",
      mappingState: "mapped",
      zone: 2,
      zoneCount: 5,
      lowerBoundBpm: 136,
      upperBoundBpmExclusive: 151,
      durationSeconds: 1393.97,
    },
    {
      bucketIndex: 3,
      label: "Z3",
      mappingState: "mapped",
      zone: 3,
      zoneCount: 5,
      lowerBoundBpm: 151,
      upperBoundBpmExclusive: 166,
      durationSeconds: 785.24,
    },
    {
      bucketIndex: 4,
      label: "Z4",
      mappingState: "mapped",
      zone: 4,
      zoneCount: 5,
      lowerBoundBpm: 166,
      upperBoundBpmExclusive: 181,
      durationSeconds: 178.99,
    },
    {
      bucketIndex: 5,
      label: "Z5",
      mappingState: "mapped",
      zone: 5,
      zoneCount: 5,
      lowerBoundBpm: 181,
      upperBoundBpmExclusive: 204,
      durationSeconds: 0,
    },
    {
      bucketIndex: 6,
      label: "Above Z5",
      mappingState: "mapped",
      zone: null,
      zoneCount: 5,
      lowerBoundBpm: 204,
      upperBoundBpmExclusive: null,
      durationSeconds: 0,
    },
  ]);
});

test("preserves historical percent-max-hr boundaries from the FIT activity", () => {
  const result = normalizeFitMessages(
    {
      sessionMesgs: [{ hrCalcType: "percent_max_hr" }],
      timeInZoneMesgs: [
        {
          referenceMesg: "session",
          referenceIndex: 0,
          hrCalcType: "percent_max_hr",
          hrZoneHighBoundary: [100, 121, 141, 163, 184, 204],
          timeInHrZone: [10, 20, 30, 40, 50, 60, 70],
        },
      ],
      hrZoneMesgs: [],
      powerZoneMesgs: [],
      lapMesgs: [],
      recordMesgs: [],
    },
    "activity.fit",
  );

  assert.equal(result.heartRate.calculationType, "percent_max_hr");
  assert.deepEqual(
    result.heartRate.zones.map((zone) => ({
      label: zone.label,
      lowerBoundBpm: zone.lowerBoundBpm,
      upperBoundBpmExclusive: zone.upperBoundBpmExclusive,
    })),
    [
      { label: "Below Z1", lowerBoundBpm: null, upperBoundBpmExclusive: 100 },
      { label: "Z1", lowerBoundBpm: 100, upperBoundBpmExclusive: 121 },
      { label: "Z2", lowerBoundBpm: 121, upperBoundBpmExclusive: 141 },
      { label: "Z3", lowerBoundBpm: 141, upperBoundBpmExclusive: 163 },
      { label: "Z4", lowerBoundBpm: 163, upperBoundBpmExclusive: 184 },
      { label: "Z5", lowerBoundBpm: 184, upperBoundBpmExclusive: 204 },
      { label: "Above Z5", lowerBoundBpm: 204, upperBoundBpmExclusive: null },
    ],
  );
});

test("supports activities with a different number of actual HR zones", () => {
  const result = normalizeFitMessages(
    {
      sessionMesgs: [],
      timeInZoneMesgs: [
        {
          referenceMesg: "session",
          referenceIndex: 0,
          hrZoneHighBoundary: [90, 110, 130],
          timeInHrZone: [5, 10, 15, 20],
        },
      ],
      hrZoneMesgs: [],
      powerZoneMesgs: [],
      lapMesgs: [],
      recordMesgs: [],
    },
    "activity.fit",
  );

  assert.deepEqual(
    result.heartRate.zones.map((zone) => ({
      label: zone.label,
      zone: zone.zone,
      zoneCount: zone.zoneCount,
    })),
    [
      { label: "Below Z1", zone: null, zoneCount: 2 },
      { label: "Z1", zone: 1, zoneCount: 2 },
      { label: "Z2", zone: 2, zoneCount: 2 },
      { label: "Above Z2", zone: null, zoneCount: 2 },
    ],
  );
});

test("falls back to unmapped buckets when time and boundary arrays do not align", () => {
  const result = normalizeFitMessages(
    {
      sessionMesgs: [],
      timeInZoneMesgs: [
        {
          referenceMesg: "session",
          referenceIndex: 0,
          hrZoneHighBoundary: [120, 136],
          timeInHrZone: [10, 20, 30, 40],
        },
      ],
      hrZoneMesgs: [
        { messageIndex: 0, lowBpm: 120 },
        { messageIndex: 1, lowBpm: 136 },
      ],
      powerZoneMesgs: [],
      lapMesgs: [],
      recordMesgs: [],
    },
    "activity.fit",
  );

  assert.deepEqual(result.heartRate.zones, [
    {
      bucketIndex: 0,
      label: "Bucket 1",
      mappingState: "unmapped",
      zone: null,
      zoneCount: null,
      lowerBoundBpm: null,
      upperBoundBpmExclusive: null,
      durationSeconds: 10,
    },
    {
      bucketIndex: 1,
      label: "Bucket 2",
      mappingState: "unmapped",
      zone: null,
      zoneCount: null,
      lowerBoundBpm: null,
      upperBoundBpmExclusive: null,
      durationSeconds: 20,
    },
    {
      bucketIndex: 2,
      label: "Bucket 3",
      mappingState: "unmapped",
      zone: null,
      zoneCount: null,
      lowerBoundBpm: null,
      upperBoundBpmExclusive: null,
      durationSeconds: 30,
    },
    {
      bucketIndex: 3,
      label: "Bucket 4",
      mappingState: "unmapped",
      zone: null,
      zoneCount: null,
      lowerBoundBpm: null,
      upperBoundBpmExclusive: null,
      durationSeconds: 40,
    },
  ]);
});

test("falls back to unmapped buckets when boundaries are missing", () => {
  const result = normalizeFitMessages(
    {
      sessionMesgs: [{ timeInHrZone: [12, 34, 56] }],
      timeInZoneMesgs: [],
      hrZoneMesgs: [],
      powerZoneMesgs: [],
      lapMesgs: [],
      recordMesgs: [],
    },
    "activity.fit",
  );

  assert.deepEqual(
    result.heartRate.zones.map((zone) => ({
      label: zone.label,
      mappingState: zone.mappingState,
      durationSeconds: zone.durationSeconds,
    })),
    [
      { label: "Bucket 1", mappingState: "unmapped", durationSeconds: 12 },
      { label: "Bucket 2", mappingState: "unmapped", durationSeconds: 34 },
      { label: "Bucket 3", mappingState: "unmapped", durationSeconds: 56 },
    ],
  );
});

test("keeps zero-duration zones in mapped output", () => {
  const result = normalizeFitMessages(
    {
      sessionMesgs: [],
      timeInZoneMesgs: [
        {
          referenceMesg: "session",
          referenceIndex: 0,
          hrZoneHighBoundary: [100, 121, 141, 163, 184, 204],
          timeInHrZone: [0, 10, 0, 20, 0, 30, 0],
        },
      ],
      hrZoneMesgs: [],
      powerZoneMesgs: [],
      lapMesgs: [],
      recordMesgs: [],
    },
    "activity.fit",
  );

  assert.deepEqual(
    result.heartRate.zones.map((zone) => zone.durationSeconds),
    [0, 10, 0, 20, 0, 30, 0],
  );
});

test("uses only the session-level time-in-zone record for activity HR summaries", () => {
  const result = normalizeFitMessages(
    {
      sessionMesgs: [],
      timeInZoneMesgs: [
        {
          referenceMesg: "session",
          referenceIndex: 0,
          hrZoneHighBoundary: [119, 139],
          timeInHrZone: [10, 20, 30],
          timeInPowerZone: [5, 15, 25],
          powerZoneHighBoundary: [150, 220, 300],
        },
        {
          referenceMesg: "lap",
          referenceIndex: 0,
          hrZoneHighBoundary: [119, 139],
          timeInHrZone: [100, 200, 300],
          timeInPowerZone: [50, 60, 70],
        },
        {
          referenceMesg: "split",
          referenceIndex: 0,
          hrZoneHighBoundary: [119, 139],
          timeInHrZone: [1000, 2000, 3000],
        },
      ],
      hrZoneMesgs: [],
      powerZoneMesgs: [],
      lapMesgs: [],
      recordMesgs: [],
    },
    "activity.fit",
  );

  assert.deepEqual(
    result.heartRate.zones.map((zone) => zone.durationSeconds),
    [10, 20, 30],
  );
  assert.deepEqual(result.power.zones, [
    { zone: 1, minWatts: 0, maxWatts: 150, durationSeconds: 5 },
    { zone: 2, minWatts: 151, maxWatts: 220, durationSeconds: 15 },
    { zone: 3, minWatts: 221, maxWatts: 300, durationSeconds: 25 },
  ]);
});

test("normalizes decoded FIT messages into the stable schema", () => {
  const result = normalizeFitMessages(decoded, "activity.fit");

  assert.equal(result.schemaVersion, "1.0.0");
  assert.equal(result.activity.type, "running");
  assert.equal(result.summary.distance.value, 10000);
  assert.equal(result.pace.average.value, 354);
  assert.equal(result.runningDynamics.cadence.averageStepsPerMinute, 170);
  assert.equal(result.heartRate.calculationType, null);
  assert.deepEqual(result.heartRate.zones, [
    {
      bucketIndex: 0,
      label: "Bucket 1",
      mappingState: "unmapped",
      zone: null,
      zoneCount: null,
      lowerBoundBpm: null,
      upperBoundBpmExclusive: null,
      durationSeconds: 300,
    },
    {
      bucketIndex: 1,
      label: "Bucket 2",
      mappingState: "unmapped",
      zone: null,
      zoneCount: null,
      lowerBoundBpm: null,
      upperBoundBpmExclusive: null,
      durationSeconds: 900,
    },
    {
      bucketIndex: 2,
      label: "Bucket 3",
      mappingState: "unmapped",
      zone: null,
      zoneCount: null,
      lowerBoundBpm: null,
      upperBoundBpmExclusive: null,
      durationSeconds: 1200,
    },
    {
      bucketIndex: 3,
      label: "Bucket 4",
      mappingState: "unmapped",
      zone: null,
      zoneCount: null,
      lowerBoundBpm: null,
      upperBoundBpmExclusive: null,
      durationSeconds: 900,
    },
    {
      bucketIndex: 4,
      label: "Bucket 5",
      mappingState: "unmapped",
      zone: null,
      zoneCount: null,
      lowerBoundBpm: null,
      upperBoundBpmExclusive: null,
      durationSeconds: 240,
    },
  ]);
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
