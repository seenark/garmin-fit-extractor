import { useMemo } from "react";
import { barY, defineChart, lineY } from "@tanstack/charts";
import { Chart } from "@tanstack/charts/react";
import { scaleLinear } from "@tanstack/charts/scales/linear";
import { scalePoint } from "@tanstack/charts/scales/point";
import { tooltip } from "@tanstack/charts/tooltip";
import type { Analysis } from "../lib/api-types";
import {
  buildElevationData,
  buildHeartRateZoneData,
  buildLapPowerData,
  buildLapHeartRateData,
  buildLapPaceData,
  buildPowerData,
  buildPowerZoneData,
  type ElevationDatum,
  type HeartRateZoneDatum,
  type LapHeartRateDatum,
  type LapPowerDatum,
  type LapPaceDatum,
  type PowerDatum,
  type PowerZoneDatum,
} from "../lib/activity-chart-data";
import {
  formatDuration,
  formatHeartRate,
  formatHeartRateBucketRange,
  formatPower,
  formatPowerRange,
  formatPace,
  formatPaceTick,
} from "../lib/formatters";

const chartTheme = {
  background: "transparent",
  foreground: "var(--color-ink)",
  grid: "var(--color-rule)",
  muted: "var(--color-muted)",
  palette: ["var(--color-accent)", "var(--color-focus)"],
};

const GARMIN_FIVE_ZONE_HEART_RATE_BUCKET_COLORS = {
  above: "var(--color-zone-red)",
  below: "var(--color-zone-gray)",
  zone1: "var(--color-zone-gray)",
  zone2: "var(--color-zone-blue)",
  zone3: "var(--color-zone-green)",
  zone4: "var(--color-zone-orange)",
  zone5: "var(--color-zone-red)",
} as const;

const FALLBACK_HEART_RATE_ZONE_COLORS = [
  "var(--color-zone-gray)",
  "var(--color-zone-blue)",
  "var(--color-zone-green)",
  "var(--color-zone-orange)",
  "var(--color-zone-red)",
  "var(--color-zone-purple)",
] as const;

const GARMIN_POWER_ZONE_COLORS = [
  "var(--color-zone-gray)",
  "var(--color-zone-blue)",
  "var(--color-zone-green)",
  "var(--color-zone-yellow)",
  "var(--color-zone-orange)",
  "var(--color-zone-red)",
  "var(--color-zone-purple)",
] as const;

function garminZoneColor(zone: number, palette: readonly string[]): string {
  if (!Number.isFinite(zone)) return palette[0] ?? "var(--color-zone-gray)";
  const index = Math.max(0, Math.min(palette.length - 1, Math.round(zone) - 1));
  return palette[index] ?? palette[0] ?? "var(--color-zone-gray)";
}

function heartRateBucketColor(datum: HeartRateZoneDatum): string {
  if (datum.mappingState !== "mapped") return "var(--color-zone-gray)";
  if (datum.zoneCount !== 5) {
    return garminZoneColor(datum.bucketIndex + 1, FALLBACK_HEART_RATE_ZONE_COLORS);
  }
  if (datum.label === "Below Z1") return GARMIN_FIVE_ZONE_HEART_RATE_BUCKET_COLORS.below;
  if (datum.label === "Above Z5") return GARMIN_FIVE_ZONE_HEART_RATE_BUCKET_COLORS.above;

  switch (datum.zone) {
    case 1:
      return GARMIN_FIVE_ZONE_HEART_RATE_BUCKET_COLORS.zone1;
    case 2:
      return GARMIN_FIVE_ZONE_HEART_RATE_BUCKET_COLORS.zone2;
    case 3:
      return GARMIN_FIVE_ZONE_HEART_RATE_BUCKET_COLORS.zone3;
    case 4:
      return GARMIN_FIVE_ZONE_HEART_RATE_BUCKET_COLORS.zone4;
    case 5:
      return GARMIN_FIVE_ZONE_HEART_RATE_BUCKET_COLORS.zone5;
    default:
      return garminZoneColor(datum.bucketIndex + 1, FALLBACK_HEART_RATE_ZONE_COLORS);
  }
}

function ChartCard({
  children,
  className = "",
  description,
  title,
  testId,
}: {
  children: React.ReactNode;
  className?: string;
  description: string;
  title: string;
  testId: string;
}) {
  return (
    <article className={`activity-chart-card ${className}`} data-testid={testId}>
      <header className="activity-chart-card-header">
        <h4>{title}</h4>
        <p>{description}</p>
      </header>
      {children}
    </article>
  );
}

function ChartHost({ children }: { children: React.ReactNode }) {
  return (
    <div className="activity-chart-host">
      {children}
      <p className="activity-chart-note">ใช้เมาส์หรือแป้นพิมพ์เพื่อดูค่าของจุดข้อมูล</p>
    </div>
  );
}

function ChartLegend({
  items,
}: {
  items: Array<{ label: string; style: "accent" | "focus" | "dashed" }>;
}) {
  return (
    <ul className="activity-chart-legend" aria-label="คำอธิบายกราฟ">
      {items.map((item) => (
        <li key={item.label}>
          <span
            aria-hidden="true"
            className={`activity-chart-swatch activity-chart-swatch--${item.style}`}
          />
          {item.label}
        </li>
      ))}
    </ul>
  );
}

function ZoneLegend({
  items,
}: {
  items: Array<{ key: string; color: string; title: string; label: string }>;
}) {
  return (
    <ul className="activity-chart-legend activity-chart-zone-legend" aria-label="สีของโซน">
      {items.map((item) => (
        <li key={item.key} data-zone={item.key}>
          <span
            aria-hidden="true"
            className="activity-chart-zone-swatch"
            style={{ backgroundColor: item.color }}
          />
          <span>{item.title}</span>
          <span className="activity-chart-zone-range">{item.label}</span>
        </li>
      ))}
    </ul>
  );
}

function ChartEmptyState({ children }: { children: string }) {
  return <p className="activity-chart-empty muted">{children}</p>;
}

function createLapPaceDefinition(data: readonly LapPaceDatum[]) {
  return defineChart({
    marks: [
      lineY(data, {
        id: "lap-pace",
        points: true,
        stroke: "var(--color-accent-strong)",
        strokeWidth: 2.5,
        x: "lap",
        y: "paceSecondsPerKm",
      }),
    ],
    scales: {
      x: {
        axis: {
          label: "รอบ",
          ticks: { format: (value) => `รอบ ${value}` },
        },
        scale: () => scalePoint<number>().padding(0.2),
      },
      y: {
        axis: {
          label: "เพซ (นาที/กม.)",
          ticks: { format: (value) => formatPaceTick(Number(value)) },
        },
        grid: true,
        nice: true,
        reverse: true,
        scale: scaleLinear,
      },
    },
    theme: chartTheme,
    focus: "nearest-x",
    tooltip: {
      use: tooltip,
      format: (point) => {
        const datum = point.datum;
        return `รอบ ${datum.lap}: ${formatPace(datum.paceSecondsPerKm)}`;
      },
    },
  });
}

function createHeartRateZoneDefinition(data: readonly HeartRateZoneDatum[]) {
  return defineChart({
    marks: [
      barY(data, {
        fill: (datum) => heartRateBucketColor(datum),
        id: "heart-rate-zones",
        radius: 4,
        x: "label",
        y: "durationSeconds",
      }),
    ],
    scales: {
      x: {
        axis: {
          label: "โซนอัตราการเต้นหัวใจ",
          ticks: { format: (value) => String(value) },
        },
        scale: () => scalePoint<string>().padding(0.25),
      },
      y: {
        axis: { label: "เวลา (วินาที)" },
        grid: true,
        nice: true,
        scale: scaleLinear,
      },
    },
    theme: chartTheme,
    focus: "nearest-x",
    tooltip: {
      use: tooltip,
      format: (point) => {
        const datum = point.datum;
        return `${datum.label}: ${formatDuration(datum.durationSeconds)} · ${formatHeartRateBucketRange(datum.lowerBoundBpm, datum.upperBoundBpmExclusive, datum.mappingState)}`;
      },
    },
  });
}

function createPowerZoneDefinition(data: readonly PowerZoneDatum[]) {
  return defineChart({
    marks: [
      barY(data, {
        fill: (datum) => garminZoneColor(datum.zone, GARMIN_POWER_ZONE_COLORS),
        id: "power-zones",
        radius: 4,
        x: "zone",
        y: "durationSeconds",
      }),
    ],
    scales: {
      x: {
        axis: {
          label: "Power zone",
          ticks: { format: (value) => `โซน ${value}` },
        },
        scale: () => scalePoint<number>().padding(0.2),
      },
      y: {
        axis: { label: "เวลา (วินาที)" },
        grid: true,
        nice: true,
        scale: scaleLinear,
      },
    },
    theme: chartTheme,
    focus: "nearest-x",
    tooltip: {
      use: tooltip,
      format: (point) => {
        const datum = point.datum;
        return `โซน ${datum.zone}: ${formatDuration(datum.durationSeconds)} · ${formatPowerRange(datum.minWatts, datum.maxWatts)}`;
      },
    },
  });
}
type LapHeartRateSeriesDatum = {
  lap: number;
  bpm: number;
};

function createLapHeartRateDefinition(
  averageData: readonly LapHeartRateSeriesDatum[],
  maximumData: readonly LapHeartRateSeriesDatum[],
) {
  return defineChart({
    marks: [
      lineY(averageData, {
        id: "lap-heart-rate-average",
        points: true,
        stroke: "var(--color-focus)",
        strokeWidth: 2.5,
        x: "lap",
        y: "bpm",
      }),
      lineY(maximumData, {
        id: "lap-heart-rate-maximum",
        points: true,
        stroke: "var(--color-ink-2)",
        strokeDasharray: "6 4",
        strokeWidth: 2,
        x: "lap",
        y: "bpm",
      }),
    ],
    scales: {
      x: {
        axis: {
          label: "รอบ",
          ticks: { format: (value) => `รอบ ${value}` },
        },
        scale: () => scalePoint<number>().padding(0.2),
      },
      y: {
        axis: { label: "อัตราการเต้นหัวใจ (ครั้งต่อนาที)" },
        grid: true,
        nice: true,
        scale: scaleLinear,
      },
    },
    theme: chartTheme,
    focus: "group-x",
    tooltip: {
      use: tooltip,
      formatGroup: (points) => {
        const firstPoint = points[0];
        if (!firstPoint) return "";

        const rows = points.map((point) => {
          const label =
            point.markId === "lap-heart-rate-average" ? "ค่าเฉลี่ย" : "ค่าสูงสุด";
          return `${label}: ${formatHeartRate(point.datum.bpm)}`;
        });

        return [`รอบ ${firstPoint.datum.lap}`, ...rows].join("\n");
      },
    },
  });
}

function createPowerDefinition(data: readonly PowerDatum[]) {
  return defineChart({
    marks: [
      lineY(data, {
        id: "activity-power",
        points: false,
        stroke: "var(--color-zone-blue)",
        strokeWidth: 2.5,
        x: "seconds",
        y: "watts",
      }),
    ],
    scales: {
      x: {
        axis: {
          label: "เวลา",
          ticks: { format: (value) => formatDuration(Number(value)) },
        },
        scale: scaleLinear,
      },
      y: {
        axis: { label: "กำลัง (วัตต์)" },
        grid: true,
        nice: true,
        scale: scaleLinear,
      },
    },
    theme: chartTheme,
    focus: "nearest-x",
    tooltip: {
      use: tooltip,
      format: (point) => {
        const datum = point.datum;
        return `${formatDuration(datum.seconds)} · ${formatPower(datum.watts)}`;
      },
    },
  });
}

type LapPowerSeriesDatum = {
  lap: number;
  watts: number;
};

function createLapPowerDefinition(
  averageData: readonly LapPowerSeriesDatum[],
  maximumData: readonly LapPowerSeriesDatum[],
) {
  return defineChart({
    marks: [
      lineY(averageData, {
        id: "lap-power-average",
        points: true,
        stroke: "var(--color-zone-blue)",
        strokeWidth: 2.5,
        x: "lap",
        y: "watts",
      }),
      lineY(maximumData, {
        id: "lap-power-maximum",
        points: true,
        stroke: "var(--color-zone-orange)",
        strokeDasharray: "6 4",
        strokeWidth: 2,
        x: "lap",
        y: "watts",
      }),
    ],
    scales: {
      x: {
        axis: {
          label: "รอบ",
          ticks: { format: (value) => `รอบ ${value}` },
        },
        scale: () => scalePoint<number>().padding(0.2),
      },
      y: {
        axis: { label: "กำลัง (วัตต์)" },
        grid: true,
        nice: true,
        scale: scaleLinear,
      },
    },
    theme: chartTheme,
    focus: "group-x",
    tooltip: {
      use: tooltip,
      formatGroup: (points) => {
        const firstPoint = points[0];
        if (!firstPoint) return "";

        const rows = points.map((point) => {
          const label = point.markId === "lap-power-average" ? "ค่าเฉลี่ย" : "ค่าสูงสุด";
          return `${label}: ${formatPower(point.datum.watts)}`;
        });

        return [`รอบ ${firstPoint.datum.lap}`, ...rows].join("\n");
      },
    },
  });
}

function createElevationDefinition(data: readonly ElevationDatum[]) {
  return defineChart({
    marks: [
      barY(data, {
        fill: (datum) =>
          datum.direction === "ascent"
            ? "var(--color-accent)"
            : "var(--color-focus)",
        id: "elevation-balance",
        radius: 4,
        x: "direction",
        y: "meters",
      }),
    ],
    scales: {
      x: {
        axis: {
          label: "ทิศทาง",
          ticks: {
            format: (value) => (value === "ascent" ? "ขึ้น" : "ลง"),
          },
        },
        scale: () => scalePoint<string>().padding(0.35),
      },
      y: {
        axis: { label: "เมตร" },
        grid: true,
        nice: true,
        scale: scaleLinear,
      },
    },
    theme: chartTheme,
    focus: "nearest-x",
    tooltip: {
      use: tooltip,
      format: (point) => {
        const datum = point.datum;
        const label = datum.direction === "ascent" ? "ไต่ระดับขึ้น" : "ไต่ระดับลง";
        return `${label}: ${datum.meters.toLocaleString("th-TH")} เมตร`;
      },
    },
  });
}

function LapPaceChart({ data }: { data: readonly LapPaceDatum[] }) {
  const definition = useMemo(() => createLapPaceDefinition(data), [data]);

  return (
    <ChartCard
      className="activity-chart-card--wide"
      description="ดูว่าเพซเปลี่ยนไปอย่างไรในแต่ละรอบ ยิ่งจุดอยู่สูงยิ่งเร็วกว่า"
      testId="activity-chart-pace"
      title="เพซต่อรอบ"
    >
      <ChartHost>
        <Chart
          ariaDescription="แกนแนวนอนคือรอบ แกนตั้งคือเพซในรูปแบบนาทีต่อกิโลเมตร และแกนถูกกลับด้านเพื่อให้เพซที่เร็วกว่าอยู่ด้านบน"
          ariaLabel="กราฟเพซต่อรอบ"
          className="activity-chart"
          definition={definition}
          height={250}
          idPrefix="lap-pace"
          initialWidth={640}
          tabIndex={0}
        />
      </ChartHost>
    </ChartCard>
  );
}

function HeartRateZoneChart({
  data,
  wide = false,
}: {
  data: readonly HeartRateZoneDatum[];
  wide?: boolean;
}) {
  const definition = useMemo(
    () => (data.length === 0 ? null : createHeartRateZoneDefinition(data)),
    [data],
  );

  return (
    <ChartCard
      className={wide ? "activity-chart-card--wide" : ""}
      description="เปรียบเทียบเวลาที่บันทึกได้ในแต่ละโซน หัวใจเต้นอยู่ในช่วงไหนนานที่สุด"
      testId="activity-chart-heart-rate-zones"
      title="เวลาใน Heart-rate zone"
    >
      {definition ? (
        <>
          <ZoneLegend
            items={data.map((datum) => ({
              key: datum.label,
              color: heartRateBucketColor(datum),
              title: datum.label,
              label: formatHeartRateBucketRange(
                datum.lowerBoundBpm,
                datum.upperBoundBpmExclusive,
                datum.mappingState,
              ),
            }))}
          />
          <ChartHost>
            <Chart
              ariaDescription="แกนแนวนอนคือโซนอัตราการเต้นหัวใจ แกนตั้งคือเวลาเป็นวินาที"
              ariaLabel="กราฟเวลาใน Heart-rate zone"
              className="activity-chart"
              definition={definition}
              height={250}
              idPrefix="heart-rate-zones"
              initialWidth={520}
              tabIndex={0}
            />
          </ChartHost>
        </>
      ) : (
        <ChartEmptyState>
          ไฟล์ FIT นี้ไม่มีข้อมูลเวลาใน Heart-rate zone
        </ChartEmptyState>
      )}
    </ChartCard>
  );
}

function PowerZoneChart({
  data,
  wide = false,
}: {
  data: readonly PowerZoneDatum[];
  wide?: boolean;
}) {
  const definition = useMemo(
    () => (data.length === 0 ? null : createPowerZoneDefinition(data)),
    [data],
  );

  return (
    <ChartCard
      className={wide ? "activity-chart-card--wide" : ""}
      description="เปรียบเทียบเวลาที่บันทึกได้ในแต่ละ Power zone ตามขีดจำกัดกำลังของไฟล์ FIT"
      testId="activity-chart-power-zones"
      title="เวลาใน Power zone"
    >
      {definition ? (
        <>
          <ZoneLegend
            items={data.map((datum) => ({
              key: `power-zone-${datum.zone}`,
              color: garminZoneColor(datum.zone, GARMIN_POWER_ZONE_COLORS),
              title: `โซน ${datum.zone}`,
              label: formatPowerRange(datum.minWatts, datum.maxWatts),
            }))}
          />
          <ChartHost>
            <Chart
              ariaDescription="แกนแนวนอนคือ Power zone แกนตั้งคือเวลาเป็นวินาที และสีของแท่งตรงกับสีของ Garmin zone"
              ariaLabel="กราฟเวลาใน Power zone"
              className="activity-chart"
              definition={definition}
              height={250}
              idPrefix="power-zones"
              initialWidth={520}
              tabIndex={0}
            />
          </ChartHost>
        </>
      ) : (
        <ChartEmptyState>
          ไฟล์ FIT นี้ไม่มีข้อมูลเวลาใน Power zone
        </ChartEmptyState>
      )}
    </ChartCard>
  );
}

function LapHeartRateChart({ data }: { data: readonly LapHeartRateDatum[] }) {
  const averageData = useMemo(
    () =>
      data.flatMap((datum) =>
        datum.averageBpm === null
          ? []
          : [{ lap: datum.lap, bpm: datum.averageBpm }],
      ),
    [data],
  );
  const maximumData = useMemo(
    () =>
      data.flatMap((datum) =>
        datum.maximumBpm === null
          ? []
          : [{ lap: datum.lap, bpm: datum.maximumBpm }],
      ),
    [data],
  );
  const definition = useMemo(
    () => createLapHeartRateDefinition(averageData, maximumData),
    [averageData, maximumData],
  );

  return (
    <ChartCard
      className="activity-chart-card--wide"
      description="ดูค่าเฉลี่ยและค่าสูงสุดของอัตราการเต้นหัวใจในแต่ละรอบ เส้นประคือค่าสูงสุด"
      testId="activity-chart-heart-rate"
      title="อัตราการเต้นหัวใจต่อรอบ"
    >
      <ChartLegend
        items={[
          { label: "ค่าเฉลี่ย", style: "focus" },
          { label: "ค่าสูงสุด", style: "dashed" },
        ]}
      />
      <ChartHost>
        <Chart
          ariaDescription="เส้นทึบแสดงค่าเฉลี่ย เส้นประแสดงค่าสูงสุด และค่าที่หายไปจะเว้นว่าง"
          ariaLabel="กราฟอัตราการเต้นหัวใจต่อรอบ"
          className="activity-chart"
          definition={definition}
          height={250}
          idPrefix="lap-heart-rate"
          initialWidth={640}
          tabIndex={0}
        />
      </ChartHost>
    </ChartCard>
  );
}

function PowerChart({ data }: { data: readonly PowerDatum[] }) {
  const definition = useMemo(
    () => (data.length === 0 ? null : createPowerDefinition(data)),
    [data],
  );

  return (
    <ChartCard
      className="activity-chart-card--wide"
      description="ดูการเปลี่ยนแปลงของกำลังตามเวลาตลอดกิจกรรมจาก record samples"
      testId="activity-chart-power"
      title="กำลังตลอดกิจกรรม"
    >
      {definition ? (
        <ChartHost>
          <Chart
            ariaDescription="กราฟเส้นแสดงกำลังเป็นวัตต์ตามเวลาที่ผ่านไป"
            ariaLabel="กราฟกำลังตลอดกิจกรรม"
            className="activity-chart"
            definition={definition}
            height={250}
            idPrefix="activity-power"
            initialWidth={640}
            tabIndex={0}
          />
        </ChartHost>
      ) : (
        <ChartEmptyState>
          ไฟล์ FIT นี้ไม่มี record samples ที่มีข้อมูลกำลังสำหรับกราฟ
        </ChartEmptyState>
      )}
    </ChartCard>
  );
}

function LapPowerChart({ data }: { data: readonly LapPowerDatum[] }) {
  const averageData = useMemo(
    () =>
      data.flatMap((datum) =>
        datum.averageWatts === null
          ? []
          : [{ lap: datum.lap, watts: datum.averageWatts }],
      ),
    [data],
  );
  const maximumData = useMemo(
    () =>
      data.flatMap((datum) =>
        datum.maximumWatts === null
          ? []
          : [{ lap: datum.lap, watts: datum.maximumWatts }],
      ),
    [data],
  );
  const definition = useMemo(
    () => createLapPowerDefinition(averageData, maximumData),
    [averageData, maximumData],
  );

  return (
    <ChartCard
      className="activity-chart-card--wide"
      description="ดูค่าเฉลี่ยและค่าสูงสุดของกำลังในแต่ละรอบ เพราะไฟล์เก่าอาจไม่มี record samples"
      testId="activity-chart-power"
      title="กำลังต่อรอบ"
    >
      <ChartLegend
        items={[
          { label: "ค่าเฉลี่ย", style: "focus" },
          { label: "ค่าสูงสุด", style: "dashed" },
        ]}
      />
      <ChartHost>
        <Chart
          ariaDescription="เส้นทึบแสดงค่าเฉลี่ย เส้นประแสดงค่าสูงสุด และค่าที่หายไปจะเว้นว่าง"
          ariaLabel="กราฟกำลังต่อรอบ"
          className="activity-chart"
          definition={definition}
          height={250}
          idPrefix="lap-power"
          initialWidth={640}
          tabIndex={0}
        />
      </ChartHost>
    </ChartCard>
  );
}

function ElevationChart({
  data,
  wide = false,
}: {
  data: readonly ElevationDatum[];
  wide?: boolean;
}) {
  const definition = useMemo(() => createElevationDefinition(data), [data]);

  return (
    <ChartCard
      className={wide ? "activity-chart-card--wide" : ""}
      description="เปรียบเทียบระยะไต่ระดับขึ้นและลงของกิจกรรมนี้"
      testId="activity-chart-elevation"
      title="สมดุลระดับความสูง"
    >
      <ChartLegend
        items={[
          { label: "ไต่ระดับขึ้น", style: "accent" },
          { label: "ไต่ระดับลง", style: "focus" },
        ]}
      />
      <ChartHost>
        <Chart
          ariaDescription="แกนแนวนอนแบ่งเป็นไต่ระดับขึ้นและไต่ระดับลง แกนตั้งมีหน่วยเป็นเมตร และแต่ละแท่งแสดงค่าของทิศทางนั้น"
          ariaLabel="กราฟสมดุลระดับความสูง"
          className="activity-chart"
          definition={definition}
          height={250}
          idPrefix="elevation-balance"
          initialWidth={520}
          tabIndex={0}
        />
      </ChartHost>
    </ChartCard>
  );
}

export function ActivityCharts({ analysis }: { analysis: Analysis }) {
  const paceData = useMemo(() => buildLapPaceData(analysis.laps), [analysis.laps]);
  const heartRateZoneData = useMemo(
    () => buildHeartRateZoneData(analysis.heartRate.zones),
    [analysis.heartRate.zones],
  );
  const powerZoneData = useMemo(
    () => buildPowerZoneData(analysis.power.zones),
    [analysis.power.zones],
  );
  const powerData = useMemo(
    () => buildPowerData(analysis.samples),
    [analysis.samples],
  );
  const lapPowerData = useMemo(
    () => buildLapPowerData(analysis.laps),
    [analysis.laps],
  );
  const lapHeartRateData = useMemo(
    () => buildLapHeartRateData(analysis.laps),
    [analysis.laps],
  );
  const elevationData = useMemo(
    () => buildElevationData(analysis.elevation),
    [analysis.elevation],
  );
  const hasPowerChart = powerData.length > 0 || lapPowerData.length > 0;
  const chartCount =
    Number(paceData.length > 0) +
    Number(heartRateZoneData.length > 0) +
    Number(powerZoneData.length > 0) +
    Number(hasPowerChart) +
    Number(lapHeartRateData.length > 0) +
    Number(elevationData.length > 0);
  const compactChartCount =
    Number(heartRateZoneData.length > 0) +
    Number(powerZoneData.length > 0) +
    Number(elevationData.length > 0);
  const emptyChartCount =
    Number(heartRateZoneData.length === 0) +
    Number(powerZoneData.length === 0) +
    Number(!hasPowerChart);

  return (
    <section
      id="activity-charts"
      aria-labelledby="activity-charts-heading"
      className="activity-charts"
      data-testid="activity-charts"
    >
      <div className="activity-charts-heading">
        <h3 id="activity-charts-heading">ภาพรวมจากกราฟ</h3>
        <p className="muted">
          แสดงเฉพาะข้อมูลที่ไฟล์ FIT บันทึกไว้ · มีกราฟพร้อมข้อมูล {chartCount} รายการ
          {emptyChartCount > 0 && ` · ไม่มีข้อมูลสำหรับกราฟ ${emptyChartCount} รายการ`}
        </p>
      </div>
      <div className="activity-chart-grid" data-testid="activity-chart-grid">
        {paceData.length > 0 && <LapPaceChart data={paceData} />}
        <HeartRateZoneChart
          data={heartRateZoneData}
          wide={compactChartCount === 1 && heartRateZoneData.length > 0}
        />
        <PowerZoneChart
          data={powerZoneData}
          wide={compactChartCount === 1 && powerZoneData.length > 0}
        />
        {powerData.length > 0 ? (
          <PowerChart data={powerData} />
        ) : lapPowerData.length > 0 ? (
          <LapPowerChart data={lapPowerData} />
        ) : (
          <PowerChart data={[]} />
        )}
        {lapHeartRateData.length > 0 && (
          <LapHeartRateChart data={lapHeartRateData} />
        )}
        {elevationData.length > 0 && (
          <ElevationChart
            data={elevationData}
            wide={compactChartCount === 1 && elevationData.length > 0}
          />
        )}
      </div>
      <p className="activity-charts-footnote muted">
        ค่าตัวเลขแบบละเอียดและรายการรอบยังแสดงอยู่ในการ์ดสรุปและตารางด้านล่าง
      </p>
    </section>
  );
}
