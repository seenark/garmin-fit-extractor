import type {
  ActivitySample,
  Analysis,
  HeartRateZone,
  Metric,
  PowerZone,
} from "./api-types";

type Lap = Analysis["laps"][number];

export interface LapPaceDatum {
  lap: number;
  paceSecondsPerKm: number;
}

export interface HeartRateZoneDatum {
  bucketIndex: number;
  label: string;
  mappingState: "mapped" | "unmapped";
  zone: number | null;
  zoneCount: number | null;
  durationSeconds: number;
  lowerBoundBpm: number | null;
  upperBoundBpmExclusive: number | null;
}

export interface PowerZoneDatum {
  zone: number;
  durationSeconds: number;
  minWatts: number | null;
  maxWatts: number | null;
}

export interface PowerDatum {
  seconds: number;
  watts: number;
}

export interface LapPowerDatum {
  lap: number;
  averageWatts: number | null;
  maximumWatts: number | null;
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

function nonnegativeOptional(value: number | null): number | null {
  return isFiniteNumber(value) && value >= 0 ? value : null;
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
        bucketIndex: zone.bucketIndex,
        label: zone.label,
        mappingState: zone.mappingState,
        zone: zone.zone,
        zoneCount: zone.zoneCount,
        durationSeconds: zone.durationSeconds,
        lowerBoundBpm: zone.lowerBoundBpm,
        upperBoundBpmExclusive: zone.upperBoundBpmExclusive,
      },
    ];
  });
}

export function buildPowerZoneData(zones: PowerZone[]): PowerZoneDatum[] {
  return zones.flatMap((zone) => {
    if (!isFiniteNumber(zone.durationSeconds) || zone.durationSeconds < 0) {
      return [];
    }
    return [
      {
        zone: zone.zone,
        durationSeconds: zone.durationSeconds,
        minWatts: zone.minWatts,
        maxWatts: zone.maxWatts,
      },
    ];
  });
}

export function buildPowerData(samples: ActivitySample[]): PowerDatum[] {
  return samples.flatMap((sample) => {
    const seconds = nonnegativeOptional(sample.elapsedSeconds);
    const watts = nonnegativeOptional(sample.powerWatts);
    return seconds === null || watts === null ? [] : [{ seconds, watts }];
  });
}

export function buildLapPowerData(laps: Lap[]): LapPowerDatum[] {
  return laps.flatMap((lap) => {
    const averageWatts = nonnegativeOptional(lap.power.averageWatts);
    const maximumWatts = nonnegativeOptional(lap.power.maximumWatts);
    return averageWatts === null && maximumWatts === null
      ? []
      : [{ lap: lap.index, averageWatts, maximumWatts }];
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
