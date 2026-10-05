import { useId, useMemo, useRef, useState, type KeyboardEvent, type PointerEvent } from "react";
import { defineChart, lineY, rect, ruleX, type ChartRenderContext } from "@tanstack/charts";
import { Chart } from "@tanstack/charts/react";
import { scaleLinear } from "@tanstack/charts/scales/linear";
import type { NormalizedRun, RunLap, RunSegment } from "../lib/runs-types";
import { buildRunChartData, inspectRunSample, type RunChartData, type RunChartMetric, type RunChartRow } from "../lib/run-chart-data";
import { formatDuration, formatHeartRate, formatNumber, formatPace, formatPaceTick, formatPower } from "../lib/formatters";

const metrics: { key: RunChartMetric; label: string; unit: string; color: string }[] = [
  { key: "pace", label: "Pace", unit: "นาที/กม.", color: "var(--color-accent)" },
  { key: "heartRate", label: "HR", unit: "bpm", color: "var(--color-danger)" },
  { key: "power", label: "Power", unit: "W", color: "var(--color-focus)" },
];
const clock = (seconds: number) => {
  const value = Math.max(0, Math.round(seconds));
  return `${Math.floor(value / 60)}:${String(value % 60).padStart(2, "0")}`;
};
const eligible = (segment: RunSegment) => segment.eligibility.lt1.accepted || segment.eligibility.lt2.accepted;
const numeric = (value: number | null): value is number => typeof value === "number" && Number.isFinite(value);
const exact = (value: number | null, unit: string) => numeric(value) ? `${value} ${unit}` : "ไม่มีข้อมูล";

function Timeline({ data, metric, domain, selected, laps, segments, onInspect, onKeyDown }: {
  data: RunChartData;
  metric: typeof metrics[number];
  domain: [number, number];
  selected: number | null;
  laps: readonly RunLap[];
  segments: readonly RunSegment[];
  onInspect: (seconds: number) => void;
  onKeyDown: (event: KeyboardEvent<HTMLDivElement>) => void;
}) {
  const rendered = useRef<ChartRenderContext<RunChartRow | RunSegment, number, number> | null>(null);
  const rows = useMemo(() => data.rows.filter((row) => row.seconds >= domain[0] && row.seconds <= domain[1]), [data.rows, domain]);
  const definition = useMemo(() => {
    let minimum = Infinity;
    let maximum = -Infinity;
    for (const row of rows) {
      const value = row[metric.key];
      if (value !== null) { minimum = Math.min(minimum, value); maximum = Math.max(maximum, value); }
    }
    if (!Number.isFinite(minimum)) { minimum = 0; maximum = 1; }
    const padding = Math.max((maximum - minimum) * 0.08, 1);
    const low = Math.max(0, minimum - padding);
    const high = maximum + padding;
    return defineChart({
      marks: [
        rect(segments.filter(eligible), { id: "accepted-segments", x1: "startElapsedSeconds", x2: "endElapsedSeconds", y1: () => low, y2: () => high, fill: "var(--color-success)", fillOpacity: 0.09 }),
        rect(segments.filter((segment) => !eligible(segment)), { id: "rejected-segments", x1: "startElapsedSeconds", x2: "endElapsedSeconds", y1: () => low, y2: () => high, fill: "var(--color-danger)", fillOpacity: 0.05, stroke: "var(--color-danger)", strokeWidth: 0.5 }),
        lineY(rows, { id: metric.key, x: "seconds", y: metric.key, points: true, stroke: metric.color, strokeWidth: 2 }),
        ruleX(laps.flatMap((lap) => [lap.startElapsedSeconds, lap.endElapsedSeconds]).filter(numeric), { id: "garmin-laps", stroke: "var(--color-ink-2)", strokeDasharray: "4 5", strokeOpacity: 0.6 }),
        ruleX(selected === null ? [] : [selected], { id: "source-inspector", stroke: "var(--color-focus)", strokeWidth: 1.5 }),
      ],
      scales: {
        x: { scale: scaleLinear().domain(domain), axis: { ticks: { format: (value) => clock(Number(value)) } } },
        y: { scale: scaleLinear().domain([low, high]), reverse: metric.key === "pace", grid: true, axis: { ticks: { format: (value) => metric.key === "pace" ? formatPaceTick(Number(value)) : String(Math.round(Number(value))) } } },
      },
      theme: { background: "transparent", foreground: "var(--color-ink)", muted: "var(--color-muted)", grid: "var(--color-rule)" },
      margin: { left: 58, right: 16, top: 12, bottom: 32 },
      clip: true,
      focus: false,
      tooltip: false,
    });
  }, [rows, domain, metric, laps, segments, selected]);
  const inspectPointer = (event: PointerEvent<HTMLDivElement>) => {
    const context = rendered.current;
    if (!context) return;
    const bounds = context.svg.getBoundingClientRect();
    const x = (event.clientX - bounds.left) * context.scene.width / bounds.width;
    const y = (event.clientY - bounds.top) * context.scene.height / bounds.height;
    const plot = context.scene.chart;
    if (x < plot.x || x > plot.x + plot.width || y < plot.y || y > plot.y + plot.height) return;
    const seconds = context.scene.scales.x?.invert?.(x);
    if (typeof seconds === "number") onInspect(seconds);
  };
  const hasData = rows.some((row) => row[metric.key] !== null);
  return <section className="runs-chart-plot" aria-label={`${metric.label} timeline`}>
    <div className="runs-chart-heading"><h3>{metric.label}</h3><span className="muted">{metric.unit}</span></div>
    <div className="runs-chart-interaction" tabIndex={0} role="group" aria-label={`ตรวจ ${metric.label} ตามเวลาที่ผ่านไป`} aria-description="ลูกศรซ้าย/ขวาเลือกค่าต้นฉบับ เครื่องหมายบวก/ลบซูม ปุ่ม R คืนช่วงทั้งหมด" onKeyDown={onKeyDown} onPointerMove={inspectPointer} onPointerDown={inspectPointer}>
      {hasData ? <Chart definition={definition} ariaLabel={`${metric.label} · เวลาที่ผ่านไป`} ariaDescription="เส้นประคือ Garmin Lap พื้นสีคือช่วงที่ระบบตรวจพบ ช่องว่างไม่ถูกเชื่อมเส้น" height={180} tabIndex={-1} onRender={(context) => { rendered.current = context; }} /> : <p className="muted runs-chart-empty">ไม่มีข้อมูล {metric.label} ในช่วงนี้ ไม่แทนค่าที่ขาดด้วยศูนย์</p>}
    </div>
  </section>;
}

export interface RunChartsProps { normalized: NormalizedRun; segments: readonly RunSegment[]; }

export function RunCharts({ normalized, segments }: RunChartsProps) {
  const id = useId();
  const data = useMemo(() => buildRunChartData(normalized, segments), [normalized, segments]);
  const [window, setWindow] = useState<[number, number] | null>(null);
  const [inspected, setInspected] = useState<number | null>(null);
  const [showSegments, setShowSegments] = useState(true);
  const full = data.domain;
  const domain: [number, number] = window ?? (full ? [full[0], Math.max(full[1], full[0] + 1)] : [0, 1]);
  const sample = inspectRunSample(data, inspected ?? domain[0]);
  const seconds = sample?.elapsedSeconds ?? null;
  const selectedPosition = sample ? data.inspectable.indexOf(sample) : 0;
  const select = (value: number) => {
    const next = inspectRunSample(data, Math.min(domain[1], Math.max(domain[0], value)));
    if (next) setInspected(next.elapsedSeconds);
  };
  const zoom = (factor: number) => {
    if (!full || full[0] === full[1]) return;
    const width = Math.min(full[1] - full[0], Math.max(1, (domain[1] - domain[0]) * factor));
    const center = seconds ?? (domain[0] + domain[1]) / 2;
    const start = Math.min(full[1] - width, Math.max(full[0], center - width / 2));
    setWindow([start, start + width]);
  };
  const reset = () => { setWindow(null); };
  const keys = (event: KeyboardEvent<HTMLDivElement>) => {
    if (event.key === "ArrowLeft" || event.key === "ArrowRight") {
      event.preventDefault();
      const direction = event.key === "ArrowLeft" ? -1 : 1;
      const next = data.inspectable[Math.min(data.inspectable.length - 1, Math.max(0, selectedPosition + direction))];
      if (next?.elapsedSeconds != null) {
        setInspected(next.elapsedSeconds);
        if (next.elapsedSeconds < domain[0] || next.elapsedSeconds > domain[1]) reset();
      }
    } else if (event.key === "+" || event.key === "=") { event.preventDefault(); zoom(0.5); }
    else if (event.key === "-") { event.preventDefault(); zoom(2); }
    else if (event.key.toLowerCase() === "r" || event.key === "Escape") { event.preventDefault(); reset(); }
    else if (event.key === "Home" || event.key === "End") { event.preventDefault(); reset(); setInspected(event.key === "Home" ? full?.[0] ?? null : full?.[1] ?? null); }
  };
  const focusInterval = (start: number | null, end: number | null) => {
    if (!full || !numeric(start) || !numeric(end) || end <= start) return;
    const left = Math.max(full[0], start);
    const right = Math.min(full[1], end);
    if (right <= left) return;
    setWindow([left, right]);
    setInspected(left);
  };
  const inSegments = numeric(seconds) ? segments.filter((segment) => seconds >= segment.startElapsedSeconds && seconds <= segment.endElapsedSeconds) : [];
  return <section className="runs-charts" aria-labelledby={`${id}-title`}>
    <div className="runs-chart-toolbar"><div><h2 id={`${id}-title`}>Pace / HR / Power</h2><p className="muted">ข้อมูล normalized จากกิจกรรมนี้ · แกนเวลาเดียวกัน · เวลาที่ผ่านไป รวมเวลาหยุด</p></div>
      <div className="runs-chart-actions"><button type="button" className="secondary" disabled={!full || full[0] === full[1]} onClick={() => zoom(0.5)}>ซูมเข้า</button><button type="button" className="secondary" disabled={!window} onClick={() => zoom(2)}>ซูมออก</button><button type="button" className="quiet" disabled={!window} onClick={reset}>คืนช่วงทั้งหมด</button></div>
    </div>
    <p className="runs-chart-metadata muted">ต้นฉบับ {data.metadata.originalCount.toLocaleString("th-TH")} จุด · แสดง {data.metadata.displayCount.toLocaleString("th-TH")} จุด · {data.metadata.method}. ช่องว่างเกิน {data.metadata.gapSeconds} วินาทีและ timer pause ตัดเส้น · ไม่ทราบเวลา {data.metadata.untimedCount} จุด</p>
    <div className="runs-chart-legend"><span className="runs-chart-lap-key">เส้นประ: Garmin Lap ที่บันทึก</span><label><input type="checkbox" checked={showSegments} onChange={(event) => setShowSegments(event.target.checked)} /> แสดง Detected Segments</label><span>พื้นเขียว: ผ่านอย่างน้อยหนึ่ง target · พื้นแดงมีกรอบ: ไม่ผ่านทั้งสอง target</span></div>
    {full ? <>
      <p className="muted">ช่วง {clock(domain[0])}–{clock(domain[1])} · แตะหรือเลื่อนบนกราฟเพื่อตรวจค่า · ลูกศรเลือกจุด · + / − ซูม · R คืนช่วง</p>
      <div className="runs-chart-timelines">{metrics.map((metric) => <Timeline key={metric.key} data={data} metric={metric} domain={domain} selected={seconds} laps={normalized.laps} segments={showSegments ? segments : []} onInspect={select} onKeyDown={keys} />)}</div>
      <label className="runs-chart-sample-control" htmlFor={`${id}-sample`}>ตรวจจุดต้นฉบับตามเวลา
        <input id={`${id}-sample`} type="range" min={0} max={Math.max(0, data.inspectable.length - 1)} value={selectedPosition} aria-valuetext={sample ? `${clock(seconds!)} · จุด ${sample.index}` : "ไม่มีข้อมูล"} onChange={(event) => { const next = data.inspectable[Number(event.target.value)]; if (next) { setInspected(next.elapsedSeconds); if (next.elapsedSeconds! < domain[0] || next.elapsedSeconds! > domain[1]) reset(); } }} />
      </label>
    </> : <p className="runs-chart-empty">ไม่มี sample ที่ทราบเวลา จึงไม่สร้างกราฟจากค่าเฉลี่ยหรือ Garmin Laps</p>}
    {sample && <div className="runs-chart-inspector" role="status" aria-live="polite" aria-atomic="true">
      <h3>จุดต้นฉบับ {sample.index} · {clock(seconds!)} <span className="muted">({exact(seconds, "s")})</span></h3>
      <dl><div><dt>Pace</dt><dd>{formatPace(sample.paceSecondsPerKm)} <small>{exact(sample.paceSecondsPerKm, "s/km")}</small></dd></div><div><dt>HR</dt><dd>{exact(sample.heartRateBpm, "bpm")}</dd></div><div><dt>Power</dt><dd>{exact(sample.powerWatts, "W")}</dd></div><div><dt>Speed</dt><dd>{exact(sample.speedMps, "m/s")}</dd></div></dl>
      {sample.timerRunning === false && <p>Timer หยุด: ค่าต้นฉบับยังตรวจได้ แต่ไม่เชื่อมเส้นกราฟผ่านช่วงหยุด</p>}
      <details><summary>ที่มาของค่าจุดนี้</summary><p>{sample.timestamp ?? "ไม่มี timestamp"}</p><pre>{JSON.stringify(sample.sourceReferences, null, 2)}</pre></details>
      {inSegments.map((segment) => <p key={segment.index}>Detected Segment {segment.index + 1}: {segment.kind} · LT1 {segment.eligibility.lt1.accepted ? "ผ่าน" : "ไม่ผ่าน"}: {segment.eligibility.lt1.reasons.join(", ") || "ไม่มีเหตุผลเพิ่มเติม"} · LT2 {segment.eligibility.lt2.accepted ? "ผ่าน" : "ไม่ผ่าน"}: {segment.eligibility.lt2.reasons.join(", ") || "ไม่มีเหตุผลเพิ่มเติม"}</p>)}
    </div>}
    <section className="runs-chart-laps" aria-labelledby={`${id}-laps`}><h3 id={`${id}-laps`}>Garmin Laps ที่บันทึก</h3><p className="muted">ค่าเฉลี่ยจาก recorded lap summary ไม่ใช่ช่วงที่ระบบตรวจพบ</p>
      {normalized.laps.length ? <div className="runs-chart-table-scroll"><table><thead><tr><th scope="col">Lap</th><th scope="col">เวลาที่ผ่านไป</th><th scope="col">ระยะ (m)</th><th scope="col">Timer</th><th scope="col">Pace</th><th scope="col">HR</th><th scope="col">Power</th></tr></thead><tbody>{normalized.laps.map((lap) => <tr key={lap.index}><th scope="row"><button className="quiet" type="button" onClick={() => focusInterval(lap.startElapsedSeconds, lap.endElapsedSeconds)} disabled={!full || !numeric(lap.startElapsedSeconds) || !numeric(lap.endElapsedSeconds) || lap.endElapsedSeconds <= lap.startElapsedSeconds}>Lap {lap.index + 1}</button></th><td data-label="เวลาที่ผ่านไป">{numeric(lap.startElapsedSeconds) ? clock(lap.startElapsedSeconds) : "ไม่มีข้อมูล"}–{numeric(lap.endElapsedSeconds) ? clock(lap.endElapsedSeconds) : "ไม่มีข้อมูล"}</td><td data-label="ระยะ (m)">{formatNumber(lap.summary.distanceMeters)}</td><td data-label="Timer">{formatDuration(lap.summary.timerTimeSeconds)}</td><td data-label="Pace">{formatPace(lap.summary.averagePaceSecondsPerKm)}</td><td data-label="HR">{formatHeartRate(lap.summary.averageHeartRateBpm)}</td><td data-label="Power">{formatPower(lap.summary.averagePowerWatts)}</td></tr>)}</tbody></table></div> : <p className="muted">ไม่มี Garmin Lap ที่บันทึก</p>}
    </section>
    <section className="runs-chart-segments" aria-labelledby={`${id}-segments`}><h3 id={`${id}-segments`}>Detected Segments</h3><p className="muted">ผล derived analysis ไม่ยืนยัน workout intent และไม่แทน Garmin Laps</p>
      {segments.length ? segments.map((segment) => <details key={segment.index}><summary>{segment.kind} · {clock(segment.startElapsedSeconds)}–{clock(segment.endElapsedSeconds)} · LT1 {segment.eligibility.lt1.accepted ? "ผ่าน" : "ไม่ผ่าน"} / LT2 {segment.eligibility.lt2.accepted ? "ผ่าน" : "ไม่ผ่าน"}</summary><p>LT1: {segment.eligibility.lt1.reasons.join(", ") || "ไม่มีเหตุผลเพิ่มเติม"}<br />LT2: {segment.eligibility.lt2.reasons.join(", ") || "ไม่มีเหตุผลเพิ่มเติม"}</p><p>Policy {segment.version} · {segment.timeBasis} · {exact(segment.durationSeconds, "s")}</p><pre>{JSON.stringify(segment.features, null, 2)}</pre><button className="secondary" type="button" disabled={!full} onClick={() => focusInterval(segment.startElapsedSeconds, segment.endElapsedSeconds)}>ดูช่วงนี้</button></details>) : <p className="muted">ไม่มีช่วงที่ตรวจพบ</p>}
    </section>
  </section>;
}
