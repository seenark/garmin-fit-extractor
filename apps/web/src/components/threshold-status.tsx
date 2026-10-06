import { useId, useState } from "react";
import type {
  RunHistoricalThresholds,
  RunLatestThresholds,
  RunProcessing,
  RunThresholdTarget,
  RunTrend,
} from "../lib/runs-types";
import { formatHeartRate } from "../lib/formatters";

const targets = ["lt1", "lt2"] as const;

function DataValue({ value }: { value: unknown }) {
  if (value === null || value === undefined) return <span className="muted">ไม่มีข้อมูล</span>;
  if (typeof value === "string" || typeof value === "number") return <>{value}</>;
  if (typeof value === "boolean") return <code>{String(value)}</code>;
  return <code>{JSON.stringify(value)}</code>;
}

function ThresholdValue({ value }: { value: unknown }) {
  if (value !== null && typeof value === "object" && "heartRateBpm" in value && typeof value.heartRateBpm === "number") {
    return <>{formatHeartRate(value.heartRateBpm)}</>;
  }
  return <DataValue value={value} />;
}

function Inspector({ label, value }: { label: string; value: unknown }) {
  return (
    <details className="threshold-inspector">
      <summary>{label}</summary>
      <pre aria-label={label}>{JSON.stringify(value, null, 2)}</pre>
    </details>
  );
}

function EvidenceDates({ cutoff, computed }: { cutoff: string | null; computed: string | null }) {
  return (
    <dl className="threshold-metadata">
      <div><dt>วันตัดหลักฐาน (evidenceCutoff)</dt><dd>{cutoff ? <time dateTime={cutoff}>{cutoff}</time> : "ไม่มีข้อมูล"}</dd></div>
      <div><dt>เวลาคำนวณ (computedAt)</dt><dd>{computed ? <time dateTime={computed}>{computed}</time> : "ไม่มีข้อมูล"}</dd></div>
    </dl>
  );
}

function Suggestions({ target, suggestions }: { target: string; suggestions: unknown[] }) {
  const [dismissed, setDismissed] = useState(false);
  const id = useId();
  if (suggestions.length === 0) return null;
  return (
    <aside className="threshold-suggestions" aria-label={`คำแนะนำเสริม ${target}`}>
      <h4>คำแนะนำเสริม · {target}</h4>
      <div id={id} hidden={dismissed}>
      <p className="muted">เลือกข้ามได้ ไม่ใช่คำสั่งให้ทดสอบความหนักสูงสุด</p>
      <ul>{suggestions.map((suggestion, index) => <li key={index}>
        {suggestion !== null && typeof suggestion === "object" && !Array.isArray(suggestion)
          ? <dl className="threshold-metadata">{Object.entries(suggestion).map(([field, value]) => <div key={field}><dt>{field}</dt><dd><DataValue value={value} /></dd></div>)}</dl>
          : <DataValue value={suggestion} />}
      </li>)}</ul>
      </div>
      <button type="button" className="quiet" onClick={() => setDismissed(!dismissed)} aria-label={`${dismissed ? "แสดงคำแนะนำอีกครั้ง" : "ซ่อนคำแนะนำ"} ${target}`} aria-expanded={!dismissed} aria-controls={id}>{dismissed ? "แสดงคำแนะนำอีกครั้ง" : "ซ่อนคำแนะนำ"}</button>
    </aside>
  );
}

function TargetStatus({ name, result }: { name: string; result: RunThresholdTarget }) {
  return (
    <section className="threshold-target" aria-label={`ผล ${name}`}>
      <h4>{name} · <code data-threshold-status={result.status}>{result.status}</code></h4>
      <dl className="threshold-metadata">
        <div><dt>ค่าประมาณของระบบ</dt><dd><ThresholdValue value={result.value} /></dd></div>
        <div><dt>ความไม่แน่นอน</dt><dd><DataValue value={result.uncertainty} /></dd></div>
        <div><dt>สถานะ engine</dt><dd><DataValue value={result.engineStatus} /></dd></div>
        <div><dt>Research gate (researchBlocked)</dt><dd><DataValue value={result.researchBlocked} /></dd></div>
        <div><dt>บริบทสำคัญที่ยืนยันไม่ได้</dt><dd><DataValue value={result.requiredContextUnprovable} /></dd></div>
        <div><dt>บริบทที่ยังไม่ยืนยัน</dt><dd><DataValue value={result.contextUnverified} /></dd></div>
        {result.evidence !== null && typeof result.evidence === "object" && "ageDays" in result.evidence &&
          <div><dt>ความเก่าของหลักฐาน (ageDays)</dt><dd><DataValue value={result.evidence.ageDays} /></dd></div>}
        {result.freshness !== undefined && <div><dt>ความใหม่ของหลักฐาน</dt><dd><DataValue value={result.freshness} /></dd></div>}
        {result.stale !== undefined && <div><dt>ผลเก่า (stale)</dt><dd><DataValue value={result.stale} /></dd></div>}
        <div><dt>วิธีคำนวณ</dt><dd><DataValue value={result.method} /></dd></div>
        <div><dt>นิยามเป้าหมาย</dt><dd><DataValue value={result.targetDefinition} /></dd></div>
      </dl>
      {result.reasons.length > 0 && <div><h5>เหตุผลจาก engine</h5><ul>{result.reasons.map((reason, index) => <li key={index}><DataValue value={reason} /></li>)}</ul></div>}
      <Inspector label={`หลักฐาน ${name} · วันที่และความใหม่ของหลักฐาน`} value={result.evidence} />
      {result.trace !== null && typeof result.trace === "object" && "counts" in result.trace &&
        <Inspector label={`จำนวนหลักฐาน ${name} (JSON)`} value={result.trace.counts} />}
      <Inspector label={`ตรวจสอบ ${name} · formula / trace / input revision / hash / limitations`} value={result} />
      <Suggestions key={JSON.stringify(result.suggestions)} target={name} suggestions={result.suggestions} />
    </section>
  );
}

export function ThresholdStatus({ result, recorded, processing }: {
  result: RunHistoricalThresholds | null;
  recorded?: unknown;
  processing?: RunProcessing;
}) {
  return (
    <section className="threshold-status" aria-label="ผล threshold ย้อนหลัง">
      <h3>LT1 / LT2 · ผลระบบย้อนหลัง</h3>
      <p className="muted">ค่าระบบเป็น VT proxy ไม่ใช่ผลตรวจ lactate ที่ผ่านการยืนยัน และแยกจากค่าที่อุปกรณ์บันทึก</p>
      {processing && <dl className="threshold-metadata" aria-label="สถานะงานประมวลผล">
        <div><dt>สถานะงาน</dt><dd><code>{processing.status}</code></dd></div>
        {processing.historyStatus && <div><dt>สถานะงานหลักฐานย้อนหลัง</dt><dd><code>{processing.historyStatus}</code>{processing.historyStatus !== "ready" ? " · ผลที่มีอยู่ยังไม่ใช่หลักฐานย้อนหลังปัจจุบัน" : null}</dd></div>}
        <div><dt>ผลเดิมระหว่างประมวลผล (stale)</dt><dd><DataValue value={processing.stale} /></dd></div>
        <div><dt>อัปเดตล้มเหลว</dt><dd><DataValue value={processing.updateFailed} /></dd></div>
        {processing.errorCode && <div><dt>ข้อผิดพลาดของงาน</dt><dd><code>{processing.errorCode}</code></dd></div>}
      </dl>}
      {result === null ? <p className="muted">ยังไม่มีผลวิเคราะห์ / ยังไม่มีผลให้แสดง สถานะงานไม่ใช่การวินิจฉัยความเพียงพอของข้อมูลสรีรวิทยา</p> : <>
        <EvidenceDates cutoff={result.evidenceCutoff} computed={result.computedAt} />
        <div className="threshold-targets">{targets.map((target) => <TargetStatus key={target} name={target.toUpperCase()} result={result[target]} />)}</div>
        <Inspector label="ข้อมูลผลย้อนหลังทั้งหมดจาก engine (JSON)" value={result} />
      </>}
      <section aria-label="threshold ที่อุปกรณ์บันทึก" className="threshold-recorded">
        <h4>ค่าที่อุปกรณ์บันทึก · แยกจากผลระบบ</h4>
        {recorded === undefined || recorded === null || (Array.isArray(recorded) && recorded.length === 0)
          ? <p className="muted">ไม่มีค่า threshold ที่อุปกรณ์บันทึกให้แสดง</p>
          : <Inspector label="ค่าจากอุปกรณ์และ provenance (JSON)" value={recorded} />}
      </section>
    </section>
  );
}

export function LatestThresholds({ latest, trend }: { latest: RunLatestThresholds; trend: RunTrend }) {
  const chronological = [...trend.items].sort((left, right) => {
    const leftTime = Date.parse(left.evidenceCutoff ?? "");
    const rightTime = Date.parse(right.evidenceCutoff ?? "");
    return Number.isFinite(leftTime) && Number.isFinite(rightTime) ? leftTime - rightTime : 0;
  });
  return (
    <section className="threshold-latest" aria-label="ผล threshold ล่าสุดและแนวโน้ม">
      <h2>LT1 / LT2 · ผลล่าสุดและแนวโน้ม</h2>
      <dl className="threshold-metadata">
        <div><dt>วันตัดหลักฐานที่ขอล่าสุด</dt><dd><DataValue value={latest.evidenceCutoff} /></dd></div>
        <div><dt>สถานะ engine</dt><dd><DataValue value={latest.engineStatus} /></dd></div>
        <div><dt>มีผลเก่า (stale)</dt><dd><DataValue value={latest.stale} /></dd></div>
      </dl>
      <h3>การคำนวณครั้งล่าสุด · latestAttempt</h3>
      <ThresholdStatus result={latest.latestAttempt} />
      <h3>ค่าตัวเลขล่าสุดที่มีของแต่ละเป้าหมาย · lastAvailable</h3>
      <p className="muted">แต่ละเป้าหมายอาจมาจากคนละกิจกรรมและคนละวัน ผลเดิมไม่ใช่ค่าปัจจุบัน ณ วันตัดหลักฐานล่าสุด</p>
      <div className="threshold-targets">{targets.map((target) => {
        const available = latest.lastAvailable[target];
        return <section key={target} className="threshold-available" aria-label={`lastAvailable ${target.toUpperCase()}`}>
          <h4>{target.toUpperCase()} · ผลล่าสุดที่มีตัวเลข</h4>
          {available === null ? <p className="muted">ยังไม่มีค่าตัวเลขที่ใช้ได้สำหรับ {target.toUpperCase()}</p> : <>
            <EvidenceDates cutoff={available.evidenceCutoff} computed={available.computedAt} />
            <p>กิจกรรม: <DataValue value={available.activityId} /></p>
            <p>ผลเก่า (stale): <DataValue value={available.stale} /></p>
            <TargetStatus name={target.toUpperCase()} result={available} />
          </>}
        </section>;
      })}</div>
      <h3>แนวโน้มตามเวลาหลักฐาน</h3>
      <p className="muted">เรียงตาม evidenceCutoff ของกิจกรรม ไม่ใช่เวลาที่คำนวณใหม่ แสดงสถานะและช่องว่างตามผลจริง</p>
      {chronological.length === 0 ? <p className="muted">ยังไม่มีประวัติแนวโน้ม</p> : <div className="table-wrap"><table className="threshold-trend">
        <caption>LT1 / LT2 ตามลำดับวันตัดหลักฐาน</caption>
        <thead><tr><th scope="col">วันตัดหลักฐาน</th><th scope="col">เวลาคำนวณ</th><th scope="col">กิจกรรม</th><th scope="col">LT1</th><th scope="col">LT2</th></tr></thead>
        <tbody>{chronological.map((item) => <tr key={`${item.activityId}:${item.evidenceCutoff}`}>
          <td data-label="วันตัดหลักฐาน"><DataValue value={item.evidenceCutoff} /></td>
          <td data-label="เวลาคำนวณ"><DataValue value={item.computedAt} /></td>
          <td data-label="กิจกรรม"><DataValue value={item.activityId} /></td>
          {targets.map((target) => <td key={target} data-label={target.toUpperCase()}><code>{item[target].status}</code><br /><DataValue value={item[target].value} /></td>)}
        </tr>)}</tbody>
      </table></div>}
      <Inspector label="ข้อมูลล่าสุดและแนวโน้มทั้งหมด (JSON)" value={{ latest, trend }} />
    </section>
  );
}
