import type { Metric } from "./api-types";

const numberFormatter = new Intl.NumberFormat("th-TH", {
  maximumFractionDigits: 2,
});
const fileNumberFormatter = new Intl.NumberFormat("th-TH", {
  maximumFractionDigits: 1,
});
const dateFormatter = new Intl.DateTimeFormat("th-TH", {
  dateStyle: "medium",
});
const dateTimeFormatter = new Intl.DateTimeFormat("th-TH", {
  dateStyle: "medium",
  timeStyle: "short",
});

const activityTypeLabels: Record<string, string> = {
  cardio: "คาร์ดิโอ",
  cardio_training: "คาร์ดิโอ",
  cycling: "ปั่นจักรยาน",
  elliptical: "เครื่องเดินวงรี",
  hiking: "เดินเขา",
  indoor_running: "วิ่งในร่ม",
  other: "กิจกรรมอื่น ๆ",
  rowing: "พายเรือ",
  running: "วิ่ง",
  strength_training: "เวทเทรนนิง",
  swimming: "ว่ายน้ำ",
  treadmill_running: "วิ่งบนลู่วิ่ง",
  walking: "เดิน",
};

function isFiniteNumber(value: number | null): value is number {
  return value !== null && Number.isFinite(value);
}

function quantity(value: number, label: string): string {
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
  return isFiniteNumber(value) ? numberFormatter.format(value) : "ไม่มีข้อมูล";
}

export function formatDuration(value: number | null): string {
  if (!isFiniteNumber(value)) return "ไม่มีข้อมูล";

  const totalSeconds = Math.max(0, Math.round(value));
  const hours = Math.floor(totalSeconds / 3_600);
  const minutes = Math.floor((totalSeconds % 3_600) / 60);
  const seconds = totalSeconds % 60;
  const parts: string[] = [];

  if (hours > 0) parts.push(quantity(hours, "ชั่วโมง"));
  if (minutes > 0) parts.push(quantity(minutes, "นาที"));
  if (seconds > 0 || parts.length === 0) parts.push(quantity(seconds, "วินาที"));

  return parts.join(" ");
}

function paceClock(value: number): string {
  const totalSeconds = Math.max(0, Math.round(value));
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = String(totalSeconds % 60).padStart(2, "0");
  return `${minutes}:${seconds}`;
}

export function formatPace(value: number | null): string {
  if (!isFiniteNumber(value)) return "ไม่มีข้อมูล";
  return `${paceClock(value)} นาทีต่อกิโลเมตร`;
}

export function formatPaceTick(value: number | null): string {
  return isFiniteNumber(value) ? paceClock(value) : "—";
}

export function formatMetric(metric: Metric): string {
  if (!isFiniteNumber(metric.value)) return "ไม่มีข้อมูล";

  switch (metric.unit) {
    case "seconds":
      return formatDuration(metric.value);
    case "seconds_per_kilometer":
      return formatPace(metric.value);
    case "meters":
      return quantity(metric.value, "เมตร");
    case "millimeters":
      return quantity(metric.value, "มิลลิเมตร");
    case "milliseconds":
      return quantity(metric.value, "มิลลิวินาที");
    case "percent":
      return quantity(metric.value, "เปอร์เซ็นต์");
    case "kcal":
      return quantity(metric.value, "กิโลแคลอรี");
    default:
      return `${numberFormatter.format(metric.value)} หน่วย (${metric.unit.replaceAll("_", " ")})`;
  }
}

export function formatCalories(value: number | null): string {
  return isFiniteNumber(value) ? quantity(value, "กิโลแคลอรี") : "ไม่มีข้อมูล";
}

export function formatHeartRate(value: number | null): string {
  return isFiniteNumber(value)
    ? `${numberFormatter.format(value)} ครั้งต่อนาที`
    : "ไม่มีข้อมูล";
}

export function formatHeartRateRange(
  minimum: number | null,
  maximum: number | null,
): string {
  if (isFiniteNumber(minimum) && isFiniteNumber(maximum)) {
    return `${numberFormatter.format(minimum)}–${numberFormatter.format(maximum)} ครั้งต่อนาที`;
  }
  if (isFiniteNumber(minimum)) return `ขั้นต่ำ ${formatHeartRate(minimum)}`;
  if (isFiniteNumber(maximum)) return `สูงสุด ${formatHeartRate(maximum)}`;
  return "ไม่มีข้อมูล";
}

export function formatHeartRateBucketRange(
  minimumInclusive: number | null,
  maximumExclusive: number | null,
  mappingState: "mapped" | "unmapped",
): string {
  if (mappingState === "unmapped") return "unknown boundaries";
  if (isFiniteNumber(minimumInclusive) && isFiniteNumber(maximumExclusive)) {
    return `${numberFormatter.format(minimumInclusive)}–${numberFormatter.format(maximumExclusive - 1)} ครั้งต่อนาที`;
  }
  if (isFiniteNumber(minimumInclusive)) {
    return `ตั้งแต่ ${numberFormatter.format(minimumInclusive)} ครั้งต่อนาที`;
  }
  if (isFiniteNumber(maximumExclusive)) {
    return `ต่ำกว่า ${numberFormatter.format(maximumExclusive)} ครั้งต่อนาที`;
  }
  return "ไม่มีข้อมูล";
}

export function formatLapHeartRate(
  average: number | null,
  maximum: number | null,
): string {
  if (isFiniteNumber(average) && isFiniteNumber(maximum)) {
    return `เฉลี่ย ${numberFormatter.format(average)} ครั้งต่อนาที, สูงสุด ${numberFormatter.format(maximum)} ครั้งต่อนาที`;
  }
  if (isFiniteNumber(average)) return `เฉลี่ย ${formatHeartRate(average)}`;
  if (isFiniteNumber(maximum)) return `สูงสุด ${formatHeartRate(maximum)}`;
  return "ไม่มีข้อมูล";
}

export function formatCadence(value: number | null): string {
  return isFiniteNumber(value)
    ? `${numberFormatter.format(value)} ก้าวต่อนาที`
    : "ไม่มีข้อมูล";
}

export function formatPower(value: number | null): string {
  return isFiniteNumber(value) ? quantity(value, "วัตต์") : "ไม่มีข้อมูล";
}

export function formatPowerRange(
  minimum: number | null,
  maximum: number | null,
): string {
  if (isFiniteNumber(minimum) && isFiniteNumber(maximum)) {
    return `${numberFormatter.format(minimum)}–${numberFormatter.format(maximum)} วัตต์`;
  }
  if (isFiniteNumber(minimum)) return `ตั้งแต่ ${formatPower(minimum)}`;
  if (isFiniteNumber(maximum)) return `ไม่เกิน ${formatPower(maximum)}`;
  return "ไม่มีข้อมูล";
}

export function formatTemperature(value: number | null): string {
  return isFiniteNumber(value)
    ? `${numberFormatter.format(value)} องศาเซลเซียส`
    : "ไม่มีข้อมูล";
}

export function formatFileSize(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes < 0) return "ไม่ทราบขนาด";
  if (bytes < 1_024) return quantity(bytes, "ไบต์");
  if (bytes < 1_024 * 1_024) {
    return `${fileNumberFormatter.format(bytes / 1_024)} กิโลไบต์`;
  }
  const megabytes = bytes / (1_024 * 1_024);
  return `${fileNumberFormatter.format(megabytes)} เมกะไบต์`;
}

export function formatActivityType(value: string | null): string {
  if (!value) return "กิจกรรม";
  return activityTypeLabels[value.trim().toLowerCase()] ?? value;
}

export function formatDate(value: string | null): string {
  const date = parseDate(value);
  return date ? dateFormatter.format(date) : "ยังไม่มีวันที่บันทึก";
}

export function formatDateTime(value: string | null): string {
  const date = parseDate(value);
  return date ? dateTimeFormatter.format(date) : "ยังไม่มีเวลาบันทึก";
}
