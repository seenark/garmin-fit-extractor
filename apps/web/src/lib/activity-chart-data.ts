import type { Analysis, HeartRateZone, Metric } from "./api-types";

type Lap = Analysis["laps"][number];

export interface LapPaceDatum {
  lap: number;
  paceSecondsPerKm: number;
}

export interface HeartRateZoneDatum {
  zone: number;
  durationSeconds: number;
  minBpm: number | null;
  maxBpm: number | null;
}

export interface LapHeartRateDatum {
  lap: number;
  averageBpm: number | null;
  maximumBpm: number | null;
}

export interface ElevationDatum {
  direction: "ascent" | "descent";
  meters: number;
}

function isFiniteNumber(value: number | null): value is number {
  return value !== null && Number.isFinite(value);
}

function positiveMetricValue(metric: Metric): number | null {
  return isFiniteNumber(metric.value) && metric.value > 0 ? metric.value : null;
}

function nonnegativeMetricValue(metric: Metric): number | null {
  return isFiniteNumber(metric.value) && metric.value >= 0 ? metric.value : null;
}

function finiteOptional(value: number | null): number | null {
  return isFiniteNumber(value) && value > 0 ? value : null;
}

export function buildLapPaceData(laps: Lap[]): LapPaceDatum[] {
  return laps.flatMap((lap) => {
    const paceSecondsPerKm = positiveMetricValue(lap.pace);
    return paceSecondsPerKm === null ? [] : [{ lap: lap.index, paceSecondsPerKm }];
  });
}

export function buildHeartRateZoneData(
  zones: HeartRateZone[],
): HeartRateZoneDatum[] {
  return zones.flatMap((zone) => {
    if (!isFiniteNumber(zone.durationSeconds) || zone.durationSeconds < 0) {
      return [];
    }
    return [
      {
        zone: zone.zone,
        durationSeconds: zone.durationSeconds,
        minBpm: zone.minBpm,
        maxBpm: zone.maxBpm,
      },
    ];
  });
}

export function buildLapHeartRateData(laps: Lap[]): LapHeartRateDatum[] {
  return laps.flatMap((lap) => {
    const averageBpm = finiteOptional(lap.heartRate.averageBpm);
    const maximumBpm = finiteOptional(lap.heartRate.maximumBpm);
    return averageBpm === null && maximumBpm === null
      ? []
      : [{ lap: lap.index, averageBpm, maximumBpm }];
  });
}

export function buildElevationData(
  elevation: Analysis["elevation"],
): ElevationDatum[] {
  const ascent = nonnegativeMetricValue(elevation.ascent);
  const descent = nonnegativeMetricValue(elevation.descent);
  if ((ascent ?? 0) === 0 && (descent ?? 0) === 0) return [];

  return [
    ...(ascent === null ? [] : [{ direction: "ascent" as const, meters: ascent }]),
    ...(descent === null ? [] : [{ direction: "descent" as const, meters: descent }]),
  ];
}
