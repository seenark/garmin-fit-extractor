import { useEffect, useRef, useState } from "react";
import { ApiError } from "../lib/api";
import { ClipboardExportError, createRunExportSession, ExportChangedError, ExportExpiredError } from "../lib/run-export";
import type { RunExportPreview, RunExportSession } from "../lib/run-export";
import type { RunExportMode, RunExportSnapshot } from "../lib/runs-types";

interface ExportProps { activityIds: string[]; historyReady?: boolean; }
interface ModeProps extends ExportProps { mode: RunExportMode; includeLocation: boolean; includeDeviceIdentifiers: boolean; }
type Action = "prepare" | "preview" | "copy" | "download";

function exportError(error: unknown): string {
  if (error instanceof ClipboardExportError) return "คัดลอกไม่สำเร็จ ดาวน์โหลด snapshot เดิมได้ ข้อมูลที่เลือกยังอยู่ครบ";
  if (error instanceof ExportExpiredError || (error instanceof ApiError && error.status === 410)) return "Snapshot หมดอายุแล้ว กดสร้าง snapshot ใหม่ก่อนส่งออกอีกครั้ง";
  if (error instanceof ApiError) return `${error.message} (${error.code}) — ยังไม่ส่งออกบางส่วนและยังคงรายการที่เลือกไว้`;
  return "ส่งออกไม่สำเร็จ ลองอีกครั้ง รายการที่เลือกยังอยู่ครบ";
}

function ModeExport({ activityIds, mode, includeLocation, includeDeviceIdentifiers, historyReady = true }: ModeProps) {
  const session = useRef<RunExportSession | null>(null);
  const selectionKey = JSON.stringify(activityIds);
  const [snapshot, setSnapshot] = useState<RunExportSnapshot>();
  const [preview, setPreview] = useState<RunExportPreview>();
  const [busy, setBusy] = useState<Action>();
  const [completed, setCompleted] = useState<Action>();
  const [message, setMessage] = useState("");
  const [error, setError] = useState("");
  const [expired, setExpired] = useState(false);
  useEffect(() => {
    const current = createRunExportSession(activityIds, includeLocation, includeDeviceIdentifiers);
    session.current = current;
    return () => { current.invalidate(); session.current = null; };
  }, [selectionKey, includeLocation, includeDeviceIdentifiers]);
  useEffect(() => {
    if (!snapshot) return;
    const deadline = Date.parse(snapshot.expiresAt);
    const timer = window.setTimeout(() => setExpired(deadline <= Date.now()), Math.min(2_147_483_647, Math.max(0, deadline - Date.now())));
    return () => window.clearTimeout(timer);
  }, [snapshot]);

  async function perform(action: Action) {
    const current = session.current;
    if (!current) return;
    setBusy(action);
    setCompleted(undefined);
    setMessage("");
    setError("");
    try {
      if (action === "prepare") setSnapshot(await current.prepare(mode));
      if (action === "preview") setPreview(await current.preview(mode));
      if (action === "copy") {
        await current.copy(mode, (text) => navigator.clipboard.writeText(text));
        setMessage("คัดลอก JSON จาก snapshot นี้แล้ว");
      }
      if (action === "download") {
        const blob = await current.download(mode);
        const url = URL.createObjectURL(blob);
        const link = document.createElement("a");
        link.href = url;
        link.download = `runs-${mode}-${snapshot!.generatedAt.replace(/[^0-9TZ]/g, "")}.json`;
        document.body.append(link);
        link.click();
        link.remove();
        window.setTimeout(() => URL.revokeObjectURL(url), 30_000);
        setMessage("ส่งไฟล์ให้เบราว์เซอร์ดาวน์โหลดแล้ว");
      }
      setCompleted(action);
    } catch (cause) {
      if (cause instanceof ExportChangedError) return;
      if (cause instanceof ExportExpiredError || (cause instanceof ApiError && cause.status === 410)) setExpired(true);
      setError(exportError(cause));
    } finally {
      if (session.current === current) setBusy(undefined);
    }
  }

  return <section className="run-export-mode" aria-label={`ส่งออก ${mode === "coach" ? "Coach JSON" : "Full JSON"}`}>
    <h3>{mode === "coach" ? "Coach JSON" : "Full JSON"}</h3>
    <p>{mode === "coach" ? "ข้อมูลสำหรับโค้ช พร้อมการรวม samples ที่เปิดเผยไว้ในข้อมูลส่งออก" : "ข้อมูล decoded, normalized และผลวิเคราะห์ ตามส่วนที่ decoder รองรับ"}</p>
    {!snapshot ? <button type="button" className="secondary" disabled={!!busy || activityIds.length === 0 || !historyReady} aria-busy={busy === "prepare"} onClick={() => perform("prepare")}>
      {busy === "prepare" ? "กำลังสร้าง snapshot…" : `เตรียม ${mode === "coach" ? "Coach" : "Full"} JSON`}
    </button> : <>
      <dl className="run-export-metadata">
        <div><dt>รายการที่ตรึงไว้</dt><dd>{activityIds.length.toLocaleString("th-TH")} กิจกรรม</dd></div>
        <div><dt>ขนาด JSON</dt><dd>{snapshot.byteLength.toLocaleString("th-TH")} bytes</dd></div>
        <div><dt>generatedAt</dt><dd><time dateTime={snapshot.generatedAt}>{snapshot.generatedAt}</time></dd></div>
        <div><dt>หมดอายุ</dt><dd><time dateTime={snapshot.expiresAt}>{snapshot.expiresAt}</time></dd></div>
      </dl>
      <p className="run-export-note">Snapshot ตรึง revisions และ generatedAt ไว้ Copy และ Download ใช้ snapshot เดียวกัน แม้ข้อมูลกิจกรรมจะประมวลผลใหม่</p>
      {!preview ? <>
        <p className="muted">ยังไม่ได้โหลดรายการที่ละไว้ ดาวน์โหลดได้โดยไม่โหลด JSON ทั้งหมดเป็นข้อความ ต้องตรวจรายการก่อน Copy</p>
        <button type="button" className="secondary" disabled={!!busy || expired} aria-busy={busy === "preview"} onClick={() => perform("preview")}>
          {busy === "preview" ? "กำลังโหลดรายการที่ละไว้…" : "ตรวจรายการที่ละไว้ก่อน Copy"}
        </button>
      </> : <div className="run-export-omissions">
        <h4>รายการที่ละไว้จริงใน snapshot</h4>
        {preview.omissions.length === 0 ? <p>ไม่มีรายการที่ละไว้ตามข้อมูลจากเซิร์ฟเวอร์</p> : <ul>{preview.omissions.map((omission, index) => <li key={`${omission.category}-${omission.pathPattern}-${index}`}>
          <strong>{omission.category}</strong> · {omission.count.toLocaleString("th-TH")} ค่า
          <code>{omission.pathPattern}</code><span>{omission.reason}</span>
        </li>)}</ul>}
      </div>}
      <div className="run-export-buttons">
        <button type="button" className="secondary" disabled={!preview || !!busy || expired} aria-busy={busy === "copy"} data-state={completed === "copy" ? "success" : error ? "error" : undefined} onClick={() => perform("copy")}>
          {busy === "copy" ? "กำลังคัดลอก…" : `Copy ${mode === "coach" ? "Coach" : "Full"} JSON`}
        </button>
        <button type="button" className="secondary" disabled={!!busy || expired} aria-busy={busy === "download"} data-state={completed === "download" ? "success" : error ? "error" : undefined} onClick={() => perform("download")}>
          {busy === "download" ? "กำลังดาวน์โหลด…" : `Download ${mode === "coach" ? "Coach" : "Full"} JSON`}
        </button>
      </div>
      {expired && <p role="alert" className="error-text">Snapshot หมดอายุแล้ว ต้องสร้าง snapshot ใหม่ ไม่ส่งออกข้อมูลจาก token ที่หมดอายุ</p>}
    </>}
    <p role="status" className="run-export-status">{message}</p>
    {error && <p role="alert" className="error-text">{error}</p>}
  </section>;
}

export function RunExportActions({ activityIds, historyReady = true }: ExportProps) {
  const [includeLocation, setIncludeLocation] = useState(false);
  const [includeDeviceIdentifiers, setIncludeDeviceIdentifiers] = useState(false);
  const [generation, setGeneration] = useState(0);
  const key = JSON.stringify([activityIds, includeLocation, includeDeviceIdentifiers, generation]);
  return <section className="run-export-actions" aria-label="ส่งออกกิจกรรมที่เลือก">
    <div className="section-heading"><h2>ส่งออก JSON</h2><span className="muted">เลือกไว้ {activityIds.length.toLocaleString("th-TH")} กิจกรรม</span></div>
    {activityIds.length === 0 && <p>เลือกกิจกรรมอย่างน้อย 1 รายการเพื่อส่งออก ไม่มีการเลือกทั้งหมดให้อัตโนมัติ</p>}
    {!historyReady && <p role="status">ผลวิเคราะห์ประวัติของรายการที่เลือกยังไม่พร้อมสำหรับ snapshot ใหม่ ต้องรอให้การประมวลผลประวัติพร้อมก่อน ไม่ใช่สถานะข้อมูลไม่พอประเมิน LT รายการที่เลือกยังอยู่ครบ และ snapshot ที่เตรียมไว้ยังคง revisions และ generatedAt เดิม</p>}
    <fieldset className="run-export-privacy">
      <legend>ข้อมูลส่วนตัวใน JSON</legend>
      <label><input type="checkbox" checked={includeLocation} onChange={(event) => setIncludeLocation(event.target.checked)} /> รวมข้อมูลตำแหน่ง</label>
      <label><input type="checkbox" checked={includeDeviceIdentifiers} onChange={(event) => setIncludeDeviceIdentifiers(event.target.checked)} /> รวมตัวระบุอุปกรณ์</label>
      <p>ทั้งสองตัวเลือกปิดไว้เริ่มต้น เปลี่ยนรายการที่เลือกหรือตัวเลือกนี้แล้วต้องเตรียม snapshot ใหม่ รายการที่ละไว้ไม่แสดงค่าที่ถูกซ่อน</p>
    </fieldset>
    <div key={key} className="run-export-modes">
      {(["coach", "full"] as const).map((mode) => <ModeExport key={mode} mode={mode} activityIds={activityIds} historyReady={historyReady} includeLocation={includeLocation} includeDeviceIdentifiers={includeDeviceIdentifiers} />)}
    </div>
    <button type="button" className="quiet" disabled={activityIds.length === 0 || !historyReady} onClick={() => setGeneration((value) => value + 1)}>สร้าง snapshot ใหม่</button>
  </section>;
}
