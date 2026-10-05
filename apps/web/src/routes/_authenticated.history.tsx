import { createFileRoute, useLocation, useRouter } from "@tanstack/react-router";
import { useEffect, useState } from "react";
import { ConfirmDeleteDialog } from "../components/confirm-delete-dialog";
import { HistoryTable } from "../components/history-table";
import { RunExportActions } from "../components/run-export-actions";
import { CoachPromptEditor } from "../components/coach-prompt-editor";
import { LatestThresholds } from "../components/threshold-status";
import { ApiError } from "../lib/api";
import { deleteRun, getLatestThresholds, getThresholdTrend, listRuns, validateHistorySearch } from "../lib/runs-api";
import type { RunSummary } from "../lib/runs-types";
import { formatApiError } from "../lib/copy";
export const Route = createFileRoute("/_authenticated/history")({
  validateSearch: validateHistorySearch,
  loaderDeps: ({ search }) => search,
  loader: async ({ deps }) => {
    const [page, thresholds] = await Promise.all([
      listRuns({ limit: 50, ...deps }),
      Promise.all([getLatestThresholds(), getThresholdTrend()]).then(([latest, trend]) => ({ latest, trend, error: null })).catch((error: unknown) => ({ latest: null, trend: null, error: error instanceof Error ? error.message : "โหลดหลักฐาน LT ไม่สำเร็จ" })),
    ]);
    return { page, thresholds };
  },
  component: HistoryPage,
});
function HistoryPage() {
  const router = useRouter();
  const navigate = Route.useNavigate();
  const { page, thresholds } = Route.useLoaderData();
  // Use current URL state, not loader-match search which can lag during navigation.
  const search = validateHistorySearch(useLocation().search);
  const [selected, setSelected] = useState<Set<string>>(() => new Set());
  const [target, setTarget] = useState<RunSummary | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const processing = page.items.some(item => item.processing.status === "queued" || item.processing.status === "processing" || item.processing.historyStatus === "pending");
  useEffect(() => {
    if (!processing) return;
    const timer = window.setInterval(() => { void router.invalidate(); }, 3000);
    return () => window.clearInterval(timer);
  }, [processing, router]);
  function select(id: string, checked: boolean) {
    setSelected(previous => { const next = new Set(previous); if (checked) next.add(id); else next.delete(id); return next; });
  }
  async function confirmDelete() {
    if (!target) return;
    setBusy(true); setError(null);
    try {
      await deleteRun(target.id);
      setTarget(null);
      // Selection remains explicit even if a selected activity is deleted.
      if (page.items.length === 1 && search.offset > 0) await navigate({ to: "/history", search: { ...search, offset: Math.max(0, search.offset - page.limit) }, replace: true });
      await router.invalidate();
    } catch (cause) { setError(cause instanceof ApiError ? formatApiError(cause) : "ลบกิจกรรมไม่สำเร็จ ลองใหม่อีกครั้ง"); }
    finally { setBusy(false); }
  }
  return <div className="page-stack">
    <header className="page-intro"><div><h1>Runs</h1><p className="page-lede">ตรวจข้อมูลวิ่ง เลือกกิจกรรมอย่างชัดเจน แล้วส่งเฉพาะข้อมูลที่เลือกให้โค้ชของคุณ</p></div><span className="history-total">{page.total} กิจกรรม</span></header>
    {thresholds.latest && thresholds.trend ? <LatestThresholds latest={thresholds.latest} trend={thresholds.trend} /> : <section className="card"><h2>LT ล่าสุด</h2><p role="alert">{thresholds.error}</p><button type="button" className="secondary" onClick={() => { void router.invalidate(); }}>ลองโหลดอีกครั้ง</button></section>}
    <div className="history-toolbar">
      <label className="field-label" htmlFor="history-order"><span>เรียงลำดับ</span><select id="history-order" value={search.order} onChange={event => { void navigate({ to: "/history", search: { offset: 0, order: event.target.value === "asc" ? "asc" : "desc" } }); }}><option value="desc">กิจกรรมใหม่สุดก่อน</option><option value="asc">กิจกรรมเก่าสุดก่อน</option></select></label>
      <span aria-live="polite">เลือก {selected.size} กิจกรรม</span>
      <button className="secondary" type="button" disabled={page.items.length === 0} onClick={() => setSelected(previous => new Set([...previous, ...page.items.map(item => item.id)]))}>เลือกหน้านี้</button>
      <button className="quiet" type="button" disabled={selected.size === 0} onClick={() => setSelected(new Set())}>ล้างการเลือก</button>
    </div>
    {error ? <div className="error" role="alert">{error}</div> : null}
    <HistoryTable page={page} search={search} selected={selected} onSelect={select} deletingId={busy ? target?.id ?? null : null} onDelete={setTarget} onPageChange={offset => { void navigate({ to: "/history", search: { offset, order: search.order } }); }} />
    <RunExportActions activityIds={[...selected]} historyReady={page.items.every(item => !selected.has(item.id) || item.sourceUnavailable || !item.processing.historyStatus || item.processing.historyStatus === "ready")} />
    <CoachPromptEditor />
    {target ? <ConfirmDeleteDialog title="ลบกิจกรรมนี้ไหม?" description="Original FIT, revisions และข้อมูลกิจกรรมนี้จะถูกลบอย่างถาวร หลักฐาน LT ที่เกี่ยวข้องจะถูกปรับใหม่ การเลือกส่งออกจะไม่ถูกเปลี่ยนเอง" confirmLabel="ลบรายการ" busy={busy} onConfirm={confirmDelete} onCancel={() => setTarget(null)} /> : null}
  </div>;
}
