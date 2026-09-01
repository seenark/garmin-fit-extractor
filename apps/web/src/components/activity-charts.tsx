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
  buildLapHeartRateData,
  buildLapPaceData,
  type ElevationDatum,
  type HeartRateZoneDatum,
  type LapHeartRateDatum,
  type LapPaceDatum,
} from "../lib/activity-chart-data";
import {
  formatDuration,
  formatHeartRate,
  formatHeartRateRange,
  formatPace,
} from "../lib/formatters";

const chartTheme = {
  background: "transparent",
  foreground: "var(--color-ink)",
  grid: "var(--color-rule)",
  muted: "var(--color-muted)",
  palette: ["var(--color-accent)", "var(--color-focus)"],
};

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
        axis: { label: "เพซ (วินาทีต่อกิโลเมตร)" },
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
        fill: "var(--color-accent)",
        id: "heart-rate-zones",
        radius: 4,
        x: "zone",
        y: "durationSeconds",
      }),
    ],
    scales: {
      x: {
        axis: {
          label: "โซนอัตราการเต้นหัวใจ",
          ticks: { format: (value) => `โซน ${value}` },
        },
        scale: () => scalePoint<number>().padding(0.25),
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
        return `โซน ${datum.zone}: ${formatDuration(datum.durationSeconds)} · ${formatHeartRateRange(datum.minBpm, datum.maxBpm)}`;
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
          ariaDescription="แกนแนวนอนคือรอบ แกนตั้งคือวินาทีต่อกิโลเมตร และแกนถูกกลับด้านเพื่อให้เพซที่เร็วกว่าอยู่ด้านบน"
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
    () => createHeartRateZoneDefinition(data),
    [data],
  );

  return (
    <ChartCard
      className={wide ? "activity-chart-card--wide" : ""}
      description="เปรียบเทียบเวลาที่บันทึกได้ในแต่ละโซน หัวใจเต้นอยู่ในช่วงไหนนานที่สุด"
      testId="activity-chart-heart-rate-zones"
      title="เวลาใน Heart-rate zone"
    >
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
  const lapHeartRateData = useMemo(
    () => buildLapHeartRateData(analysis.laps),
    [analysis.laps],
  );
  const elevationData = useMemo(
    () => buildElevationData(analysis.elevation),
    [analysis.elevation],
  );
  const chartCount =
    Number(paceData.length > 0) +
    Number(heartRateZoneData.length > 0) +
    Number(lapHeartRateData.length > 0) +
    Number(elevationData.length > 0);
  const compactChartCount =
    Number(heartRateZoneData.length > 0) + Number(elevationData.length > 0);

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
          แสดงเฉพาะข้อมูลที่ไฟล์ FIT บันทึกไว้ · มีกราฟ {chartCount} รายการ
        </p>
      </div>
      {chartCount === 0 ? (
        <p className="activity-chart-empty muted">
          ไฟล์นี้ยังไม่มีข้อมูลที่เหมาะสำหรับสร้างกราฟ
        </p>
      ) : (
        <div className="activity-chart-grid" data-testid="activity-chart-grid">
          {paceData.length > 0 && <LapPaceChart data={paceData} />}
          {heartRateZoneData.length > 0 && (
            <HeartRateZoneChart
              data={heartRateZoneData}
              wide={compactChartCount === 1}
            />
          )}
          {lapHeartRateData.length > 0 && (
            <LapHeartRateChart data={lapHeartRateData} />
          )}
          {elevationData.length > 0 && (
            <ElevationChart
              data={elevationData}
              wide={compactChartCount === 1}
            />
          )}
        </div>
      )}
      <p className="activity-charts-footnote muted">
        ค่าตัวเลขแบบละเอียดและรายการรอบยังแสดงอยู่ในการ์ดสรุปและตารางด้านล่าง
      </p>
    </section>
  );
}
