import { createFileRoute, useRouter } from "@tanstack/react-router";
import { useEffect, useState } from "react";

import { ConfirmDeleteDialog } from "../components/confirm-delete-dialog";
import { HistoryTable } from "../components/history-table";
import { ApiError, clearExtractions, deleteExtraction, listExtractions } from "../lib/api";
import type { ExtractionSummary } from "../lib/api-types";

export const Route = createFileRoute("/history")({
  validateSearch: (search: Record<string, unknown>) => ({
    offset: typeof search.offset === "number" && Number.isInteger(search.offset) && search.offset >= 0 ? search.offset : 0,
  }),
  loaderDeps: ({ search }) => search,
  loader: ({ deps }) => listExtractions({ limit: 50, offset: deps.offset }),
  component: HistoryPage,
});

function HistoryPage() {
  const router = useRouter();
  const navigate = Route.useNavigate();
  const loadedPage = Route.useLoaderData();
  const [page, setPage] = useState(loadedPage);
  useEffect(() => setPage(loadedPage), [loadedPage]);
  const [target, setTarget] = useState<ExtractionSummary | "all" | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function confirmDelete() {
    if (!target) return;
    setBusy(true);
    setError(null);
    try {
      if (target === "all") {
        await clearExtractions();
        setPage({ items: [], total: 0, limit: page.limit, offset: 0 });
        setTarget(null);
        await navigate({ to: "/history", search: { offset: 0 }, replace: true });
        await router.invalidate();
      } else {
        await deleteExtraction(target.id);
        const nextItems = page.items.filter((item) => item.id !== target.id);
        const total = page.total - 1;
        setPage({ items: nextItems, total, limit: page.limit, offset: page.offset });
        setTarget(null);
        await router.invalidate();
        if (nextItems.length === 0 && page.offset > 0) {
          await navigate({ to: "/history", search: { offset: Math.max(0, page.offset - page.limit) } });
        }
      }
    } catch (cause) {
      setError(cause instanceof ApiError ? cause.message : "History could not be updated.");
    } finally {
      setBusy(false);
    }
  }

  return <>
    <div className="actions"><button className="danger" type="button" disabled={busy || page.total === 0} onClick={() => setTarget("all")}>Clear history</button></div>
    {error ? <div className="error" role="alert">{error}</div> : null}
    <HistoryTable page={page} deletingId={busy && target !== "all" ? target?.id ?? null : null} onDelete={setTarget} onPageChange={(offset) => navigate({ to: "/history", search: { offset } })} />
    {target ? <ConfirmDeleteDialog title={target === "all" ? "Clear history?" : `Delete ${target.fileName}?`} description={target === "all" ? "This permanently removes every extraction from history." : "This permanently removes this extraction and both stored JSON views."} confirmLabel={target === "all" ? "Clear history" : "Delete"} busy={busy} onConfirm={confirmDelete} onCancel={() => setTarget(null)} /> : null}
  </>;
}
