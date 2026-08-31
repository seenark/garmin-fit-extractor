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
import { formatApiError } from "../lib/copy";

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
          ? formatApiError(cause)
          : "อัปเดตประวัติไม่สำเร็จ ลองใหม่อีกครั้ง",
      );
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="page-stack">
      <header className="page-intro">
        <div>
          <h1>ประวัติ</h1>
          <p className="page-lede">
            เปิดดูข้อมูลกิจกรรมที่แยกไว้แล้ว ดูรายละเอียด
            หรือลบผลลัพธ์ที่บันทึกไว้ในบัญชีนี้
          </p>
        </div>
        <span className="history-total" aria-label={`${page.total} รายการ`}>
          {page.total} รายการ
        </span>
      </header>

      <div className="history-toolbar">
        <label className="field-label" htmlFor="history-order">
          <span>เรียงลำดับ</span>
          <select
            id="history-order"
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
            <option value="desc">กิจกรรมใหม่สุดก่อน</option>
            <option value="asc">กิจกรรมเก่าสุดก่อน</option>
          </select>
        </label>
        <button
          className="danger"
          type="button"
          disabled={busy || page.total === 0}
          onClick={() => setTarget("all")}
        >
          ล้างประวัติ
        </button>
      </div>

      {error ? (
        <div className="error" role="alert">
          <span>{error}</span>
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
            target === "all" ? "ล้างประวัติทั้งหมดไหม?" : `ลบ ${target.fileName} ไหม?`
          }
          description={
            target === "all"
              ? "รายการที่แยกข้อมูลไว้ทั้งหมดจะถูกลบออกจากประวัติอย่างถาวร"
              : "รายการนี้และ JSON ทั้งสองแบบจะถูกลบอย่างถาวร"
          }
          confirmLabel={target === "all" ? "ล้างประวัติ" : "ลบรายการ"}
          busy={busy}
          onConfirm={confirmDelete}
          onCancel={() => setTarget(null)}
        />
      ) : null}
    </div>
  );
}
