import { Link, createFileRoute, useLocation, useRouter } from "@tanstack/react-router";
import { useEffect, useState } from "react";
import { RunCharts } from "../components/run-charts";
import { RunShareCard } from "../components/run-share-card";
import { RunExportActions } from "../components/run-export-actions";
import { CoachPromptEditor } from "../components/coach-prompt-editor";
import { RunProcessingStatus } from "../components/run-processing-status";
import { ThresholdStatus } from "../components/threshold-status";
import { ApiError } from "../lib/api";
import { getRun, reprocessRun, validateHistorySearch } from "../lib/runs-api";
import { formatApiError } from "../lib/copy";
import { formatDateTime } from "../lib/formatters";
import { formatRunShareMetric } from "../lib/run-share-card";
export const Route = createFileRoute("/_authenticated/extractions/$id")({
  validateSearch: validateHistorySearch,
  loader: ({ params }) => getRun(params.id),
  component: ExtractionDetailPage,
});
function ExtractionDetailPage() {
  const detail = Route.useLoaderData();
  const search = validateHistorySearch(useLocation().search);
  const router = useRouter();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const processing = detail.processing.status === "queued" || detail.processing.status === "processing";
  useEffect(() => {
    if (!processing && detail.processing.historyStatus !== "pending") return;
    const timer = window.setInterval(() => { void router.invalidate(); }, 3000);
    return () => window.clearInterval(timer);
  }, [processing, detail.processing.historyStatus, router]);
  async function reprocess() {
    setBusy(true); setError(null);
    try { await reprocessRun(detail.id); await router.invalidate(); }
    catch (cause) { setError(cause instanceof ApiError ? formatApiError(cause) : "ส่งงานประมวลผลใหม่ไม่สำเร็จ"); }
    finally { setBusy(false); }
  }
  const metrics = [
    formatRunShareMetric("distance", detail.summary.distanceMeters),
    formatRunShareMetric("timer", detail.summary.timerTimeSeconds),
    formatRunShareMetric("pace", detail.summary.averagePaceSecondsPerKm),
    formatRunShareMetric("heartRate", detail.summary.averageHeartRateBpm),
    formatRunShareMetric("power", detail.summary.averagePowerWatts),
  ];
  return <div className="page-stack">
    <header className="page-intro"><div><h1>กิจกรรมวิ่ง</h1><p className="page-lede">{formatDateTime(detail.startTime)}</p><p className="section-note">Revision {detail.revisionId ?? "ไม่มี coherent revision"}</p></div><Link className="button quiet" to="/history" search={search}>กลับไปประวัติ</Link></header>
    <section className="card"><div className="actions"><RunProcessingStatus processing={detail.processing} sourceUnavailable={detail.sourceUnavailable} /><button type="button" className="secondary" disabled={busy || processing || detail.sourceUnavailable} aria-busy={busy} onClick={() => { void reprocess(); }}>{busy ? "กำลังส่งงาน…" : "ประมวลผลใหม่"}</button></div>
      {error ? <p className="error" role="alert">{error}</p> : null}
      {detail.fidelityWarnings?.length ? <p className="section-note">{detail.fidelityWarnings.join(" · ")}</p> : null}
      <dl className="runs-metrics">{metrics.map(metric => <div key={metric.label}><dt>{metric.label}</dt><dd>{metric.value} <span>{metric.unit}</span></dd></div>)}</dl>
    </section>
    {detail.normalized ? <RunCharts key={`${detail.id}:${detail.revisionId}`} normalized={detail.normalized} segments={detail.analysis?.segments ?? []} /> : <section className="card"><h2>ข้อมูลกราฟ</h2><p>{detail.sourceUnavailable ? "ข้อมูลเดิมไม่มี normalized streams ที่ผ่าน Runs v2 จึงไม่สร้างกราฟหรือข้อมูลที่ขาดขึ้นเอง" : processing ? "กราฟพร้อมเมื่อ coherent revision ประมวลผลสำเร็จ" : "ยังไม่มี normalized streams สำหรับกิจกรรมนี้"}</p></section>}
    <ThresholdStatus result={detail.historicalThresholds} recorded={detail.normalized?.deviceReportedThresholds} processing={detail.processing} />
    {detail.analysis ? <section className="card"><h2>คุณภาพข้อมูลและข้อจำกัด</h2><details><summary>ตรวจคุณภาพและ transformations</summary><pre className="raw-json">{JSON.stringify({ quality: detail.analysis.quality, transformations: detail.analysis.transformations, warnings: detail.normalized?.warnings }, null, 2)}</pre></details></section> : null}
    <RunExportActions activityIds={[detail.id]} historyReady={detail.sourceUnavailable || !detail.processing.historyStatus || detail.processing.historyStatus === "ready"} />
    <CoachPromptEditor />
    {detail.revisionId && !detail.sourceUnavailable ? <RunShareCard key={detail.id} run={{ revisionId: detail.revisionId, startTime: detail.startTime, summary: detail.summary }} /> : <section className="card"><h2>PNG</h2><p>ต้องมี coherent revision ของ Runs v2 ก่อนสร้างภาพจากข้อมูลที่ตรึงไว้</p></section>}
  </div>;
}
