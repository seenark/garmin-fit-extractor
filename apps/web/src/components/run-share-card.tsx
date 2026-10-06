import { useEffect, useRef, useState } from "react";
import { downloadRunSharePng, loadRunSharePhoto, pinRunShareInput, renderRunShareCard, runShareCanvasToPng, runShareMetrics } from "../lib/run-share-card";
import type { RunShareInput, RunShareMetric, RunShareOptions } from "../lib/run-share-card";

const metricChoices: { key: RunShareMetric; label: string }[] = [
  { key: "distance", label: "ระยะทาง" }, { key: "timer", label: "เวลาจับเวลา (Timer time)" },
  { key: "pace", label: "เพซเฉลี่ย" }, { key: "heartRate", label: "หัวใจเฉลี่ย" }, { key: "power", label: "กำลังเฉลี่ย" },
];

export function RunShareCard({ run }: { run: RunShareInput }) {
  const [pinned, setPinned] = useState(() => pinRunShareInput(run));
  const [options, setOptions] = useState<RunShareOptions>({ theme: "light", size: "square", layout: "bottom-left", metrics: ["distance", "timer", "pace"], includeDate: false, includeName: false, name: "", cropX: .5, cropY: .5 });
  const [photo, setPhoto] = useState<ImageBitmap | null>(null);
  const [loadingPhoto, setLoadingPhoto] = useState(false);
  const [rendering, setRendering] = useState(true);
  const [downloading, setDownloading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [photoError, setPhotoError] = useState<string | null>(null);
  const [status, setStatus] = useState("");
  const preview = useRef<HTMLCanvasElement>(null);
  const photoRequest = useRef(0);
  const mounted = useRef(true);
  const sameRevision = pinned.revisionId === run.revisionId;

  useEffect(() => {
    mounted.current = true;
    return () => { mounted.current = false; photoRequest.current++; };
  }, []);
  useEffect(() => () => photo?.close(), [photo]);
  useEffect(() => {
    let active = true;
    setRendering(true);
    setError(null);
    renderRunShareCard(pinned, options, photo).then((canvas) => {
      if (!active || !preview.current) return;
      const target = preview.current;
      target.width = canvas.width;
      target.height = canvas.height;
      const context = target.getContext("2d");
      if (!context) throw new Error("เบราว์เซอร์นี้ไม่รองรับ Canvas");
      context.clearRect(0, 0, target.width, target.height);
      context.drawImage(canvas, 0, 0);
      setRendering(false);
    }).catch((cause: unknown) => {
      if (!active) return;
      setError(cause instanceof Error ? cause.message : "สร้างภาพตัวอย่างไม่สำเร็จ");
      setRendering(false);
    });
    return () => { active = false; };
  }, [pinned, options, photo]);

  async function choosePhoto(file: File | undefined) {
    if (!file) return;
    const request = ++photoRequest.current;
    setLoadingPhoto(true);
    setPhotoError(null);
    try {
      const bitmap = await loadRunSharePhoto(file);
      if (request !== photoRequest.current || !mounted.current) { bitmap.close(); return; }
      setPhoto(bitmap);
      setOptions((current) => ({ ...current, cropX: .5, cropY: .5 }));
    } catch (cause) {
      if (request === photoRequest.current && mounted.current) setPhotoError(cause instanceof Error ? cause.message : "อ่านภาพไม่สำเร็จ");
    } finally {
      if (request === photoRequest.current && mounted.current) setLoadingPhoto(false);
    }
  }

  async function download() {
    setDownloading(true);
    setError(null);
    setStatus("");
    try {
      const canvas = await renderRunShareCard(pinned, options, photo);
      const blob = await runShareCanvasToPng(canvas);
      if (!mounted.current) return;
      downloadRunSharePng(blob);
      setStatus("ส่ง PNG ให้เบราว์เซอร์แล้ว ตรวจรายการดาวน์โหลด");
    } catch (cause) {
      if (mounted.current) setError(cause instanceof Error ? cause.message : "ดาวน์โหลดไม่สำเร็จ ลองอีกครั้ง");
    } finally {
      if (mounted.current) setDownloading(false);
    }
  }

  return <section className="runs-share" aria-label="สร้างภาพ PNG" data-testid="run-share-card">
    <header className="runs-share-heading"><h2>แชร์ตัวเลขการวิ่ง</h2><p>ภาพ PNG ไม่มีกราฟ เส้นทาง หรือข้อมูลบัญชี ภาพพื้นหลังใช้ในเบราว์เซอร์เท่านั้น</p></header>
    {!sameRevision && <p role="status">มี revision ใหม่ ภาพนี้ยังใช้ค่าที่ปักไว้ <button className="secondary" type="button" disabled={downloading} onClick={() => setPinned(pinRunShareInput(run))}>ใช้ revision ล่าสุด</button></p>}
    <div className="runs-share-workspace">
      <div className="runs-share-controls">
        <fieldset disabled={downloading}><legend>รูปแบบภาพ</legend>
          <label>ตำแหน่งตัวเลข<select value={options.layout} onChange={(event) => { const layout = event.currentTarget.value; if (layout === "bottom-left" || layout === "lower-center" || layout === "bottom-right") setOptions((current) => ({ ...current, layout })); }}>
            <option value="bottom-left">ซ้ายล่าง · เรียงแนวตั้ง</option><option value="lower-center">กลางล่าง · ตารางสองคอลัมน์</option><option value="bottom-right">ขวาล่าง · ชิดขวา</option>
          </select></label>
          <label>ขนาด<select value={options.size} onChange={(event) => { const size = event.currentTarget.value === "portrait" ? "portrait" : "square"; setOptions((current) => ({ ...current, size })); }}><option value="square">จัตุรัส · 1080 × 1080</option><option value="portrait">แนวตั้ง · 1080 × 1920</option></select></label>
          <label>พื้นหลัง<select value={options.theme} onChange={(event) => { const theme = event.currentTarget.value; if (theme === "light" || theme === "dark" || theme === "transparent") setOptions((current) => ({ ...current, theme })); }}><option value="light">สว่าง</option><option value="dark">มืด</option><option value="transparent">โปร่งใส · alpha จริงเมื่อไม่มีภาพ</option></select></label>
        </fieldset>
        <fieldset disabled={downloading}><legend>ค่าที่แสดง</legend>
          {metricChoices.map(({ key, label }) => <label className="runs-share-check" key={key}><input type="checkbox" checked={options.metrics.includes(key)} onChange={(event) => { const checked = event.currentTarget.checked; setOptions((current) => ({ ...current, metrics: metricChoices.filter((choice) => choice.key === key ? checked : current.metrics.includes(choice.key)).map((choice) => choice.key) })); }} />{label}</label>)}
          <p>ค่าที่ไม่มีแสดง — ไม่แทนด้วยศูนย์</p>
        </fieldset>
        <fieldset disabled={downloading}><legend>ข้อมูลเพิ่มเติม · ไม่แสดงโดยอัตโนมัติ</legend>
          <label className="runs-share-check"><input type="checkbox" checked={options.includeDate} onChange={(event) => { const includeDate = event.currentTarget.checked; setOptions((current) => ({ ...current, includeDate })); }} />วันที่วิ่ง (UTC)</label>
          <label className="runs-share-check"><input type="checkbox" checked={options.includeName} onChange={(event) => { const includeName = event.currentTarget.checked; setOptions((current) => ({ ...current, includeName })); }} />ชื่อที่กรอกเอง</label>
          {options.includeName && <label>ชื่อบนภาพ<input type="text" maxLength={240} value={options.name} onChange={(event) => { const name = event.currentTarget.value; setOptions((current) => ({ ...current, name })); }} placeholder="ชื่อที่ต้องการแชร์ ไม่ใช้ชื่อไฟล์" /></label>}
        </fieldset>
        <fieldset disabled={downloading}><legend>ภาพพื้นหลัง (ไม่จำเป็น)</legend>
          <label>เลือกภาพจากเครื่อง<input type="file" accept="image/jpeg,image/png,image/webp" disabled={loadingPhoto} onChange={(event) => { void choosePhoto(event.currentTarget.files?.[0]); event.currentTarget.value = ""; }} /></label>
          <p>JPEG / PNG / WebP ไม่เกิน 10 MiB และ 24 ล้านพิกเซล ไม่ส่งออก EXIF</p>
          {photo && <><label>ตำแหน่งแนวนอน<input type="range" min="0" max="1" step="0.01" value={options.cropX} onChange={(event) => { const cropX = Number(event.currentTarget.value); setOptions((current) => ({ ...current, cropX })); }} /></label><label>ตำแหน่งแนวตั้ง<input type="range" min="0" max="1" step="0.01" value={options.cropY} onChange={(event) => { const cropY = Number(event.currentTarget.value); setOptions((current) => ({ ...current, cropY })); }} /></label><button className="secondary" type="button" onClick={() => { photoRequest.current++; setLoadingPhoto(false); setPhotoError(null); setPhoto(null); }}>ลบภาพพื้นหลัง</button></>}
          {loadingPhoto && <p role="status">กำลังอ่านภาพ…</p>}{photoError && <p role="alert">{photoError} ภาพเดิมยังไม่เปลี่ยน</p>}
        </fieldset>
      </div>
      <div className="runs-share-output">
        <div className={`runs-share-preview ${options.theme === "transparent" ? "runs-share-alpha" : ""}`} aria-busy={rendering}><canvas ref={preview} role="img" aria-label="ภาพตัวอย่าง PNG ตามค่าด้านล่าง" hidden={Boolean(error)} /></div>
        <dl className="runs-share-values">{runShareMetrics(pinned, options.metrics).map((metric) => <div key={metric.label}><dt>{metric.label}</dt><dd>{metric.value} {metric.unit}</dd></div>)}</dl>
        <button type="button" onClick={() => void download()} disabled={rendering || loadingPhoto || downloading || !pinned.revisionId || options.metrics.length === 0} aria-busy={downloading}>{downloading ? "กำลังสร้าง PNG…" : "ดาวน์โหลด PNG"}</button>
        <p className="runs-share-status" role={error ? "alert" : "status"}>{error || status || (rendering ? "กำลังสร้างภาพตัวอย่าง…" : "ภาพตัวอย่างใช้ตัวเลขจาก revision ที่ปักไว้")}</p>
      </div>
    </div>
  </section>;
}
