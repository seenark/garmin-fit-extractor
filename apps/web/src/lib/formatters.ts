import type { Metric } from "./api-types";

const numberFormatter = new Intl.NumberFormat("en-US", {
  maximumFractionDigits: 2,
});
const fileNumberFormatter = new Intl.NumberFormat("en-US", {
  maximumFractionDigits: 1,
});
const dateFormatter = new Intl.DateTimeFormat(undefined, {
  dateStyle: "medium",
});
const dateTimeFormatter = new Intl.DateTimeFormat(undefined, {
  dateStyle: "medium",
  timeStyle: "short",
});

function isFiniteNumber(value: number | null): value is number {
  return value !== null && Number.isFinite(value);
}

function quantity(value: number, singular: string, plural = `${singular}s`): string {
  const label = Math.abs(value) === 1 ? singular : plural;
  return `${numberFormatter.format(value)} ${label}`;
}

function parseDate(value: string | null): Date | null {
  if (!value) return null;
  const date = /^\d{4}-\d{2}-\d{2}$/.test(value)
    ? new Date(`${value}T00:00:00`)
    : new Date(value);
  return Number.isNaN(date.getTime()) ? null : date;
}

export function formatNumber(value: number | null): string {
  return isFiniteNumber(value) ? numberFormatter.format(value) : "Not available";
}

export function formatDuration(value: number | null): string {
  if (!isFiniteNumber(value)) return "Not available";

  const totalSeconds = Math.max(0, Math.round(value));
  const hours = Math.floor(totalSeconds / 3_600);
  const minutes = Math.floor((totalSeconds % 3_600) / 60);
  const seconds = totalSeconds % 60;
  const parts: string[] = [];

  if (hours > 0) parts.push(quantity(hours, "hour"));
  if (minutes > 0) parts.push(quantity(minutes, "minute"));
  if (seconds > 0 || parts.length === 0) parts.push(quantity(seconds, "second"));

  return parts.join(" ");
}

export function formatPace(value: number | null): string {
  if (!isFiniteNumber(value)) return "Not available";

  const totalSeconds = Math.max(0, Math.round(value));
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = String(totalSeconds % 60).padStart(2, "0");
  return `${minutes}:${seconds} minutes per kilometer`;
}

export function formatMetric(metric: Metric): string {
  if (!isFiniteNumber(metric.value)) return "Not available";

  switch (metric.unit) {
    case "seconds":
      return formatDuration(metric.value);
    case "seconds_per_kilometer":
      return formatPace(metric.value);
    case "meters":
      return quantity(metric.value, "meter");
    case "millimeters":
      return quantity(metric.value, "millimeter");
    case "milliseconds":
      return quantity(metric.value, "millisecond");
    case "percent":
      return `${numberFormatter.format(metric.value)} percent`;
    case "kcal":
      return quantity(metric.value, "calorie");
    default:
      return `${numberFormatter.format(metric.value)} ${metric.unit.replaceAll("_", " ")}`;
  }
}

export function formatCalories(value: number | null): string {
  return isFiniteNumber(value) ? quantity(value, "calorie") : "Not available";
}

export function formatHeartRate(value: number | null): string {
  return isFiniteNumber(value)
    ? `${numberFormatter.format(value)} beats per minute`
    : "Not available";
}

export function formatHeartRateRange(
  minimum: number | null,
  maximum: number | null,
): string {
  if (isFiniteNumber(minimum) && isFiniteNumber(maximum)) {
    return `${numberFormatter.format(minimum)}–${numberFormatter.format(maximum)} beats per minute`;
  }
  if (isFiniteNumber(minimum)) return `Minimum ${formatHeartRate(minimum)}`;
  if (isFiniteNumber(maximum)) return `Maximum ${formatHeartRate(maximum)}`;
  return "Not available";
}

export function formatLapHeartRate(
  average: number | null,
  maximum: number | null,
): string {
  if (isFiniteNumber(average) && isFiniteNumber(maximum)) {
    return `Average ${numberFormatter.format(average)}; maximum ${numberFormatter.format(maximum)} beats per minute`;
  }
  if (isFiniteNumber(average)) return `Average ${formatHeartRate(average)}`;
  if (isFiniteNumber(maximum)) return `Maximum ${formatHeartRate(maximum)}`;
  return "Not available";
}

export function formatCadence(value: number | null): string {
  return isFiniteNumber(value)
    ? `${numberFormatter.format(value)} steps per minute`
    : "Not available";
}

export function formatPower(value: number | null): string {
  return isFiniteNumber(value) ? quantity(value, "watt") : "Not available";
}

export function formatTemperature(value: number | null): string {
  return isFiniteNumber(value)
    ? `${numberFormatter.format(value)} degrees Celsius`
    : "Not available";
}

export function formatFileSize(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes < 0) return "Unknown size";
  if (bytes < 1_024) return quantity(bytes, "byte");
  if (bytes < 1_024 * 1_024) {
    return `${fileNumberFormatter.format(bytes / 1_024)} ${bytes / 1_024 === 1 ? "kilobyte" : "kilobytes"}`;
  }
  const megabytes = bytes / (1_024 * 1_024);
  return `${fileNumberFormatter.format(megabytes)} ${megabytes === 1 ? "megabyte" : "megabytes"}`;
}

export function formatDate(value: string | null): string {
  const date = parseDate(value);
  return date ? dateFormatter.format(date) : "Date not recorded";
}

export function formatDateTime(value: string | null): string {
  const date = parseDate(value);
  return date ? dateTimeFormatter.format(date) : "Time not recorded";
}
