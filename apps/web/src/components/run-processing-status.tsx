import type { RunProcessing } from "../lib/runs-types";
const labels: Record<RunProcessing["status"], string> = { queued: "รอประมวลผล", processing: "กำลังประมวลผล", ready: "พร้อมอ่าน", failed: "ประมวลผลไม่สำเร็จ" };
export function RunProcessingStatus({ processing, sourceUnavailable = false }: { processing: RunProcessing; sourceUnavailable?: boolean }) {
  return <div className="runs-processing" aria-live="polite">
    <span className={`status-badge ${processing.status === "failed" ? "failed" : processing.status === "ready" ? "success" : ""}`}>{labels[processing.status]}</span>
    {sourceUnavailable ? <p>sourceUnavailable · ข้อมูลเดิมแบบอ่านอย่างเดียว ไม่มี Original FIT สำหรับประมวลผลใหม่หรือรับรองข้อมูลครบ</p> : null}
    {processing.stale ? <p>กำลังแสดง revision ก่อนหน้า · ข้อมูลยังไม่เป็นปัจจุบัน</p> : null}
    {processing.updateFailed ? <p role="alert">ประมวลผล revision ใหม่ไม่สำเร็จ ข้อมูลก่อนหน้ายังอยู่</p> : null}
    {processing.errorCode ? <p className="table-error">{processing.errorCode}</p> : null}
  </div>;
}
