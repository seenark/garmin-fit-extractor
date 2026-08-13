import { createFileRoute, useRouter } from "@tanstack/react-router";
import { useEffect, useState } from "react";
import { ConfirmDeleteDialog } from "../components/confirm-delete-dialog";
import { HistoryTable } from "../components/history-table";
import {
  ApiError,
  clearExtractions,
  deleteExtraction,
  listExtractions,
} from "../lib/api";
import type { ExtractionSummary } from "../lib/api-types";

export const Route = createFileRoute("/history")({
  validateSearch: (search: Record<string, unknown>) => ({
    offset:
      typeof search.offset === "number" &&
      Number.isInteger(search.offset) &&
      search.offset >= 0
        ? search.offset
        : 0,
    order: search.order === "asc" ? ("asc" as const) : ("desc" as const),
  }),
  loaderDeps: ({ search }) => search,
  loader: ({ deps }) =>
    listExtractions({
      limit: 50,
      offset: deps.offset,
      order: deps.order,
    }),
  component: HistoryPage,
});

function HistoryPage() {
  const router = useRouter();
  const navigate = Route.useNavigate();
  const loadedPage = Route.useLoaderData();
  const search = Route.useSearch();
  const [page, setPage] = useState(loadedPage);
  const [target, setTarget] = useState<ExtractionSummary | "all" | null>(
    null,
  );
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => setPage(loadedPage), [loadedPage]);

  async function confirmDelete() {
    if (!target) return;
    setBusy(true);
    setError(null);
    try {
      if (target === "all") {
        await clearExtractions();
        setTarget(null);
        await navigate({
          to: "/history",
          search: { offset: 0, order: search.order },
          replace: true,
        });
      } else {
        await deleteExtraction(target.id);
        setTarget(null);
        if (page.items.length === 1 && search.offset > 0) {
          await navigate({
            to: "/history",
            search: {
              offset: Math.max(0, search.offset - page.limit),
              order: search.order,
            },
            replace: true,
          });
        }
      }
      await router.invalidate();
    } catch (cause) {
      setError(
        cause instanceof ApiError
          ? cause.message
          : "History could not be updated.",
      );
    } finally {
      setBusy(false);
    }
  }

  return (
    <>
      <div className="actions">
        <label>
          Order{" "}
          <select
            value={search.order}
            onChange={(event) =>
              navigate({
                to: "/history",
                search: {
                  offset: 0,
                  order: event.target.value as "asc" | "desc",
                },
              })
            }
          >
            <option value="desc">Newest activity first</option>
            <option value="asc">Oldest activity first</option>
          </select>
        </label>
        <button
          className="danger"
          type="button"
          disabled={busy || page.total === 0}
          onClick={() => setTarget("all")}
        >
          Clear history
        </button>
      </div>
      {error ? (
        <div className="error" role="alert">
          {error}
        </div>
      ) : null}
      <HistoryTable
        page={page}
        order={search.order}
        deletingId={busy && target !== "all" ? (target?.id ?? null) : null}
        onDelete={setTarget}
        onPageChange={(offset) =>
          navigate({
            to: "/history",
            search: { offset, order: search.order },
          })
        }
      />
      {target ? (
        <ConfirmDeleteDialog
          title={
            target === "all" ? "Clear history?" : `Delete ${target.fileName}?`
          }
          description={
            target === "all"
              ? "This permanently removes every extraction from history."
              : "This permanently removes this extraction and both stored JSON views."
          }
          confirmLabel={target === "all" ? "Clear history" : "Delete"}
          busy={busy}
          onConfirm={confirmDelete}
          onCancel={() => setTarget(null)}
        />
      ) : null}
    </>
  );
}
