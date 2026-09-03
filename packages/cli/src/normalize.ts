import type { FitMessage, FitMessages } from "./fit.js";
import type { Analysis } from "./schema.js";

const round = (value: number | null, digits = 2): number | null => {
  if (value === null || !Number.isFinite(value)) return null;
  const factor = 10 ** digits;
  return Math.round(value * factor) / factor;
};

const numberOrNull = (value: unknown): number | null =>
  typeof value === "number" && Number.isFinite(value) ? value : null;

const integerOrNull = (value: unknown): number | null => {
  const number = numberOrNull(value);
  return number === null ? null : Math.round(number);
};

const stringOrNull = (value: unknown): string | null =>
  typeof value === "string" && value.length > 0 ? value : null;

const dateOrNull = (value: unknown): string | null => {
  if (value instanceof Date && !Number.isNaN(value.getTime())) return value.toISOString();
  if (typeof value === "string") {
    const date = new Date(value);
    if (!Number.isNaN(date.getTime())) return date.toISOString();
  }
  return null;
};

const messages = (input: FitMessages, key: string): FitMessage[] => {
  const value = input[key];
  return Array.isArray(value) ? (value as FitMessage[]) : [];
};

const firstNumber = (message: FitMessage, keys: string[]): number | null => {
  for (const key of keys) {
    const value = numberOrNull(message[key]);
    if (value !== null) return value;
  }
  return null;
};

const firstString = (message: FitMessage, keys: string[]): string | null => {
  for (const key of keys) {
    const value = stringOrNull(message[key]);
    if (value !== null) return value;
  }
  return null;
};

const firstReferenceMessage = (messages: FitMessage[], reference: string): FitMessage =>
  messages.find((message) => firstString(message, ["referenceMesg"]) === reference) ?? {};

const matchingReferenceMessages = (messages: FitMessage[], reference: string): FitMessage[] =>
  messages.filter((message) => firstString(message, ["referenceMesg"]) === reference);

const speedToPace = (speedMetersPerSecond: number | null): number | null => {
  if (speedMetersPerSecond === null || speedMetersPerSecond <= 0) return null;
  return round(1000 / speedMetersPerSecond, 2);
};

const derivePace = (distanceMeters: number | null, seconds: number | null): number | null => {
  if (distanceMeters === null || distanceMeters <= 0 || seconds === null || seconds <= 0) {
    return null;
  }
  return round((seconds / distanceMeters) * 1000, 2);
};

const averageRecordValue = (records: FitMessage[], keys: string[]): number | null => {
  const values = records
    .map((record) => firstNumber(record, keys))
    .filter((value): value is number => value !== null);
  if (values.length === 0) return null;
  return values.reduce((sum, value) => sum + value, 0) / values.length;
};

const minRecordValue = (records: FitMessage[], keys: string[]): number | null => {
  const values = records
    .map((record) => firstNumber(record, keys))
    .filter((value): value is number => value !== null);
  return values.length === 0 ? null : Math.min(...values);
};

const maxRecordValue = (records: FitMessage[], keys: string[]): number | null => {
  const values = records
    .map((record) => firstNumber(record, keys))
    .filter((value): value is number => value !== null);
  return values.length === 0 ? null : Math.max(...values);
};

const normalizeCadence = (value: number | null): number | null => {
  if (value === null) return null;
  // FIT running cadence can be stored as cycles/minute. Garmin commonly displays steps/minute.
  return value < 130 ? value * 2 : value;
};

const heartRateZones = (
  session: FitMessage,
  _zoneMessages: FitMessage[],
  timeInZoneMessages: FitMessage[],
) => {
  const sessionTimeInZone = firstReferenceMessage(timeInZoneMessages, "session");
  let durations = Array.isArray(session.timeInHrZone) ? session.timeInHrZone.map(numberOrNull) : [];
  if (durations.length === 0 && Array.isArray(sessionTimeInZone.timeInHrZone)) {
    durations = sessionTimeInZone.timeInHrZone.map(numberOrNull);
  }
  if (durations.length === 0) {
    durations = aggregateZoneDurations(
      matchingReferenceMessages(timeInZoneMessages, "lap"),
      "timeInHrZone",
    );
  }

  const boundariesSource = Array.isArray(sessionTimeInZone.hrZoneHighBoundary)
    ? sessionTimeInZone.hrZoneHighBoundary
    : Array.isArray(session.hrZoneHighBoundary)
      ? session.hrZoneHighBoundary
      : [];
  const boundaries = boundariesSource.map(integerOrNull);

  const hasDurations = durations.length > 0;
  const hasBoundaries = boundaries.length > 0;
  const isMapped =
    hasDurations &&
    hasBoundaries &&
    durations.length === boundaries.length + 1 &&
    boundaries.every((value) => value !== null);

  if (!hasDurations) return [];

  if (!isMapped) {
    return durations.map((duration, index) => ({
      bucketIndex: index,
      label: `Bucket ${index + 1}`,
      mappingState: "unmapped" as const,
      zone: null,
      zoneCount: null,
      lowerBoundBpm: null,
      upperBoundBpmExclusive: null,
      durationSeconds: round(duration, 2),
    }));
  }

  const zoneCount = durations.length - 2;
  return durations.map((duration, index) => {
    const isBelow = index === 0;
    const isAbove = index === durations.length - 1;
    const zone = isBelow || isAbove ? null : index;
    const label = isBelow ? "Below Z1" : isAbove ? `Above Z${zoneCount}` : `Z${index}`;

    return {
      bucketIndex: index,
      label,
      mappingState: "mapped" as const,
      zone,
      zoneCount,
      lowerBoundBpm: isBelow ? null : (boundaries[index - 1] ?? null),
      upperBoundBpmExclusive: isAbove ? null : (boundaries[index] ?? null),
      durationSeconds: round(duration, 2),
    };
  });
};

const aggregateZoneDurations = (records: FitMessage[], key: string): Array<number | null> => {
  const count = Math.max(
    0,
    ...records.map((record) =>
      Array.isArray(record[key]) ? (record[key] as unknown[]).length : 0,
    ),
  );
  return Array.from({ length: count }, (_, index) => {
    const values = records
      .map((record) =>
        Array.isArray(record[key]) ? numberOrNull((record[key] as unknown[])[index]) : null,
      )
      .filter((value): value is number => value !== null);
    return values.length === 0 ? null : values.reduce((sum, value) => sum + value, 0);
  });
};

const powerZones = (
  session: FitMessage,
  zoneMessages: FitMessage[],
  zonesTarget: FitMessage,
  laps: FitMessage[],
  timeInZoneMessages: FitMessage[],
) => {
  const sessionTimeInZone = firstReferenceMessage(timeInZoneMessages, "session");
  let durations = Array.isArray(session.timeInPowerZone)
    ? session.timeInPowerZone.map(numberOrNull)
    : [];
  if (durations.length === 0 && Array.isArray(sessionTimeInZone.timeInPowerZone)) {
    durations = sessionTimeInZone.timeInPowerZone.map(numberOrNull);
  }
  if (durations.length === 0) {
    durations = aggregateZoneDurations(
      matchingReferenceMessages(timeInZoneMessages, "lap"),
      "timeInPowerZone",
    );
  }
  if (durations.length === 0) durations = aggregateZoneDurations(laps, "timeInPowerZone");

  const sortedZones = [...zoneMessages].sort((a, b) => {
    const aIndex = firstNumber(a, ["messageIndex"]) ?? 0;
    const bIndex = firstNumber(b, ["messageIndex"]) ?? 0;
    return aIndex - bIndex;
  });
  let boundaries = sortedZones.map((zone) => integerOrNull(zone.highValue));
  if (boundaries.length === 0 && Array.isArray(zonesTarget.powerZoneHighBoundary)) {
    boundaries = zonesTarget.powerZoneHighBoundary.map(integerOrNull);
  }
  if (boundaries.length === 0 && Array.isArray(sessionTimeInZone.powerZoneHighBoundary)) {
    boundaries = sessionTimeInZone.powerZoneHighBoundary.map(integerOrNull);
  }

  const count = Math.max(durations.length, boundaries.length);
  return Array.from({ length: count }, (_, index) => ({
    zone: index + 1,
    minWatts:
      index === 0
        ? 0
        : boundaries[index - 1] === null
          ? null
          : (boundaries[index - 1] as number) + 1,
    maxWatts: boundaries[index] ?? null,
    durationSeconds: round(durations[index] ?? null, 2),
  }));
};

const activitySamples = (session: FitMessage, records: FitMessage[]) => {
  const origin =
    dateOrNull(session.startTime ?? session.timestamp) ??
    records.map((record) => dateOrNull(record.timestamp)).find((value) => value !== null) ??
    null;
  const originMillis = origin === null ? null : Date.parse(origin);

  return records.flatMap((record, index) => {
    const timestamp = dateOrNull(record.timestamp);
    const timestampMillis = timestamp === null ? null : Date.parse(timestamp);
    const elapsedSeconds =
      originMillis !== null && timestampMillis !== null && Number.isFinite(timestampMillis)
        ? (timestampMillis - originMillis) / 1000
        : null;
    const heartRateBpm = integerOrNull(record.heartRate ?? record.hr);
    const powerWatts = integerOrNull(record.power ?? record.powerWatts);
    if (heartRateBpm === null && powerWatts === null) return [];

    return [
      {
        index,
        timestamp,
        elapsedSeconds: round(
          elapsedSeconds !== null && elapsedSeconds >= 0 ? elapsedSeconds : null,
          3,
        ),
        heartRateBpm,
        powerWatts,
      },
    ];
  });
};

export function normalizeFitMessages(input: FitMessages, fileName: string): Analysis {
  const session = messages(input, "sessionMesgs")[0] ?? {};
  const activity = messages(input, "activityMesgs")[0] ?? {};
  const laps = messages(input, "lapMesgs");
  const records = messages(input, "recordMesgs");
  const zones = messages(input, "hrZoneMesgs");
  const powerZoneMessages = messages(input, "powerZoneMesgs");
  const timeInZoneMessages = messages(input, "timeInZoneMesgs");
  const zonesTarget = messages(input, "zonesTargetMesgs")[0] ?? {};

  const distance = firstNumber(session, ["totalDistance"]);
  const duration = firstNumber(session, ["totalElapsedTime"]);
  const movingTime = firstNumber(session, ["totalTimerTime"]);
  const averageSpeed = firstNumber(session, ["enhancedAvgSpeed", "avgSpeed"]);
  const maximumSpeed = firstNumber(session, ["enhancedMaxSpeed", "maxSpeed"]);

  const averageTemperature =
    firstNumber(session, ["avgTemperature"]) ?? averageRecordValue(records, ["temperature"]);
  const minimumTemperature =
    firstNumber(session, ["minTemperature"]) ?? minRecordValue(records, ["temperature"]);
  const maximumTemperature =
    firstNumber(session, ["maxTemperature"]) ?? maxRecordValue(records, ["temperature"]);

  const averageCadence = normalizeCadence(
    firstNumber(session, ["avgRunningCadence", "avgCadence"]),
  );
  const maximumCadence = normalizeCadence(
    firstNumber(session, ["maxRunningCadence", "maxCadence"]),
  );

  return {
    schemaVersion: "1.0.0",
    source: { fileName },
    activity: {
      type: firstString(session, ["sport"]) ?? firstString(activity, ["type"]),
      subType: firstString(session, ["subSport"]),
      date: dateOrNull(session.startTime ?? activity.timestamp ?? session.timestamp),
    },
    summary: {
      duration: { value: round(duration, 2), unit: "seconds" },
      movingTime: { value: round(movingTime, 2), unit: "seconds" },
      distance: { value: round(distance, 2), unit: "meters" },
      calories: { value: integerOrNull(session.totalCalories), unit: "kcal" },
    },
    heartRate: {
      averageBpm: integerOrNull(session.avgHeartRate),
      maximumBpm: integerOrNull(session.maxHeartRate),
      calculationType:
        firstString(firstReferenceMessage(timeInZoneMessages, "session"), ["hrCalcType"]) ??
        firstString(session, ["hrCalcType"]),
      zones: heartRateZones(session, zones, timeInZoneMessages),
    },
    pace: {
      average: {
        value: speedToPace(averageSpeed) ?? derivePace(distance, duration),
        unit: "seconds_per_kilometer",
      },
      moving: {
        value: derivePace(distance, movingTime),
        unit: "seconds_per_kilometer",
      },
      best: {
        value: speedToPace(maximumSpeed),
        unit: "seconds_per_kilometer",
      },
    },
    power: {
      averageWatts: integerOrNull(session.avgPower),
      maximumWatts: integerOrNull(session.maxPower),
      zones: powerZones(session, powerZoneMessages, zonesTarget, laps, timeInZoneMessages),
    },
    runningDynamics: {
      cadence: {
        averageStepsPerMinute: round(averageCadence, 2),
        maximumStepsPerMinute: round(maximumCadence, 2),
      },
      strideLength: {
        value: round(firstNumber(session, ["avgStrideLength"]), 3),
        unit: "meters",
      },
      groundContactTime: {
        value: round(firstNumber(session, ["avgStanceTime", "avgGroundContactTime"]), 2),
        unit: "milliseconds",
      },
      verticalOscillation: {
        value: round(firstNumber(session, ["avgVerticalOscillation"]), 2),
        unit: "millimeters",
      },
      verticalRatio: {
        value: round(firstNumber(session, ["avgVerticalRatio"]), 2),
        unit: "percent",
      },
    },
    elevation: {
      ascent: { value: round(firstNumber(session, ["totalAscent"]), 2), unit: "meters" },
      descent: { value: round(firstNumber(session, ["totalDescent"]), 2), unit: "meters" },
    },
    temperature: {
      averageCelsius: round(averageTemperature, 2),
      minimumCelsius: round(minimumTemperature, 2),
      maximumCelsius: round(maximumTemperature, 2),
    },
    samples: activitySamples(session, records),
    laps: laps.map((lap, index) => {
      const lapDistance = firstNumber(lap, ["totalDistance"]);
      const lapDuration = firstNumber(lap, ["totalElapsedTime"]);
      const lapMovingTime = firstNumber(lap, ["totalTimerTime"]);
      const lapSpeed = firstNumber(lap, ["enhancedAvgSpeed", "avgSpeed"]);

      return {
        index: index + 1,
        startTime: dateOrNull(lap.startTime ?? lap.timestamp),
        distance: { value: round(lapDistance, 2), unit: "meters" },
        duration: { value: round(lapDuration, 2), unit: "seconds" },
        movingTime: { value: round(lapMovingTime, 2), unit: "seconds" },
        pace: {
          value: speedToPace(lapSpeed) ?? derivePace(lapDistance, lapMovingTime ?? lapDuration),
          unit: "seconds_per_kilometer",
        },
        heartRate: {
          averageBpm: integerOrNull(lap.avgHeartRate),
          maximumBpm: integerOrNull(lap.maxHeartRate),
        },
        power: {
          averageWatts: integerOrNull(lap.avgPower),
          maximumWatts: integerOrNull(lap.maxPower),
        },
        cadence: {
          averageStepsPerMinute: round(
            normalizeCadence(firstNumber(lap, ["avgRunningCadence", "avgCadence"])),
            2,
          ),
          maximumStepsPerMinute: round(
            normalizeCadence(firstNumber(lap, ["maxRunningCadence", "maxCadence"])),
            2,
          ),
        },
      };
    }),
  };
}
