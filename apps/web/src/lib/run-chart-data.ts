import type { NormalizedRun, RunSample, RunSegment } from "./runs-types";

export type RunChartMetric = "pace" | "heartRate" | "power";
export interface RunChartRow {
  seconds: number;
  pace: number | null;
  heartRate: number | null;
  power: number | null;
  sourcePosition: number | null;
}
export interface RunChartData {
  rows: RunChartRow[];
  original: readonly RunSample[];
  inspectable: RunSample[];
  domain: [number, number] | null;
  metadata: { originalCount: number; displayCount: number; method: string; gapSeconds: number; untimedCount: number };
}

const finite = (value: number | null | undefined): value is number => typeof value === "number" && Number.isFinite(value);
const positive = (value: number | null): number | null => finite(value) && value > 0 ? value : null;

/** Display data only. The original normalized samples remain untouched. */
export function buildRunChartData(
  normalized: Pick<NormalizedRun, "samples" | "laps" | "timerEvents">,
  segments: readonly RunSegment[] = [],
  maxPoints = 1200,
): RunChartData {
  const gapSeconds = 10;
  const rows: RunChartRow[] = [];
  const events = normalized.timerEvents.filter((event) => finite(event.elapsedSeconds) && event.event === "timer")
    .sort((left, right) => left.elapsedSeconds! - right.elapsedSeconds!);
  let eventPosition = 0;
  let timerRunning: boolean | null = null;
  let previous: number | null = null;
  let untimedCount = 0;
  for (const [sourcePosition, sample] of normalized.samples.entries()) {
    if (!finite(sample.elapsedSeconds) || sample.elapsedSeconds < 0) {
      previous = null;
      untimedCount++;
      continue;
    }
    const seconds = sample.elapsedSeconds;
    let crossedPause = false;
    while (eventPosition < events.length && events[eventPosition]!.elapsedSeconds! <= seconds) {
      const type = events[eventPosition++]!.eventType;
      if (type === "start") timerRunning = true;
      else if (type?.startsWith("stop")) {
        timerRunning = false;
        crossedPause = true;
      }
    }
    if (rows.length && (crossedPause || previous === null || seconds <= previous || seconds - previous > gapSeconds)) {
      rows.push({ seconds, pace: null, heartRate: null, power: null, sourcePosition: null });
    }
    const paused = sample.timerRunning === false || (sample.timerRunning == null && timerRunning === false);
    rows.push({
      seconds,
      pace: paused || (finite(sample.speedMps) && sample.speedMps <= 0) ? null : positive(sample.paceSecondsPerKm),
      heartRate: paused ? null : positive(sample.heartRateBpm),
      power: !paused && finite(sample.powerWatts) && sample.powerWatts >= 0 ? sample.powerWatts : null,
      sourcePosition,
    });
    previous = seconds;
  }
  const inspectable = normalized.samples.filter((sample) => finite(sample.elapsedSeconds) && sample.elapsedSeconds >= 0)
    .sort((left, right) => left.elapsedSeconds! - right.elapsedSeconds!);
  const kept = new Set<number>();
  if (rows.length > Math.max(2, maxPoints)) {
    kept.add(0);
    kept.add(rows.length - 1);
    // Boundary samples may exceed the display budget; fidelity wins over a hard cap.
    for (let index = 1; index < rows.length; index++) {
      const left = rows[index - 1]!;
      const right = rows[index]!;
      if (left.sourcePosition === null || right.sourcePosition === null ||
          (["pace", "heartRate", "power"] as const).some((metric) => (left[metric] === null) !== (right[metric] === null))) {
        kept.add(index - 1);
        kept.add(index);
      }
    }
    const boundaries = [
      ...normalized.laps.flatMap((lap) => [lap.startElapsedSeconds, lap.endElapsedSeconds]),
      ...segments.flatMap((segment) => [segment.startElapsedSeconds, segment.endElapsedSeconds]),
      ...events.map((event) => event.elapsedSeconds),
    ];
    for (const boundary of boundaries) {
      if (!finite(boundary)) continue;
      // Source order is retained, including discontinuities.
      let before = -1;
      let after = -1;
      for (let index = 0; index < rows.length; index++) {
        const row = rows[index]!;
        if (row.seconds <= boundary && (before < 0 || row.seconds > rows[before]!.seconds)) before = index;
        if (row.seconds >= boundary && (after < 0 || row.seconds < rows[after]!.seconds)) after = index;
      }
      if (before >= 0) kept.add(before);
      if (after >= 0) kept.add(after);
    }
    const bucketSize = Math.max(1, Math.ceil(rows.length / Math.max(1, Math.floor(maxPoints / 8))));
    for (let start = 0; start < rows.length; start += bucketSize) {
      const end = Math.min(rows.length, start + bucketSize);
      kept.add(start);
      kept.add(end - 1);
      for (const metric of ["pace", "heartRate", "power"] as const) {
        let minimum = -1;
        let maximum = -1;
        for (let index = start; index < end; index++) {
          const value = rows[index]![metric];
          if (value === null) continue;
          if (minimum < 0 || value < rows[minimum]![metric]!) minimum = index;
          if (maximum < 0 || value > rows[maximum]![metric]!) maximum = index;
        }
        if (minimum >= 0) kept.add(minimum);
        if (maximum >= 0) kept.add(maximum);
      }
    }
  }
  const displayRows = kept.size ? rows.filter((_, index) => kept.has(index)) : rows;
  const domain: [number, number] | null = inspectable.length ? [inspectable[0]!.elapsedSeconds!, inspectable.at(-1)!.elapsedSeconds!] : null;
  return {
    rows: displayRows, original: normalized.samples, inspectable, domain,
    metadata: { originalCount: normalized.samples.length, displayCount: displayRows.filter((row) => row.sourcePosition !== null).length, method: kept.size ? "boundary-preserving min/max buckets; no smoothing" : "none", gapSeconds, untimedCount },
  };
}

/** Inspection always resolves against full-resolution source, never display rows. */
export function inspectRunSample(data: RunChartData, seconds: number): RunSample | null {
  const samples = data.inspectable;
  if (!samples.length || !Number.isFinite(seconds)) return null;
  let low = 0;
  let high = samples.length;
  while (low < high) {
    const middle = (low + high) >>> 1;
    if (samples[middle]!.elapsedSeconds! < seconds) low = middle + 1;
    else high = middle;
  }
  const after = samples[Math.min(low, samples.length - 1)]!;
  const before = samples[Math.max(0, low - 1)]!;
  return seconds - before.elapsedSeconds! <= after.elapsedSeconds! - seconds ? before : after;
}
