import { Link, createFileRoute } from "@tanstack/react-router";
import { useState } from "react";

import { AnalysisSummary } from "../components/analysis-summary";
import { RawJsonView } from "../components/raw-json-view";
import { ApiError, downloadExtraction, getExtraction } from "../lib/api";
import { formatApiError } from "../lib/copy";
import { formatDateTime } from "../lib/formatters";

export const Route = createFileRoute("/extractions/$id")({
  validateSearch: (search: Record<string, unknown>) => ({
    offset:
      typeof search.offset === "number" &&
      Number.isInteger(search.offset) &&
      search.offset >= 0
        ? search.offset
        : 0,
    order: search.order === "asc" ? ("asc" as const) : ("desc" as const),
  }),
  loader: ({ params }) => getExtraction(params.id),
  component: ExtractionDetailPage,
});

function ExtractionDetailPage() {
  const detail = Route.useLoaderData();
  const search = Route.useSearch();
  const [tab, setTab] = useState<"summary" | "raw">("summary");
  const [error, setError] = useState<string | null>(null);
  const [downloading, setDownloading] = useState<"normalized" | "raw" | null>(
    null,
  );

  async function download(view: "normalized" | "raw") {
    setError(null);
    setDownloading(view);
    try {
      const blob = await downloadExtraction(detail.id, view);
      const link = document.createElement("a");
      const stem =
        detail.fileName.replace(/\.(?:fit|zip)$/i, "") || "extraction";
      link.href = URL.createObjectURL(blob);
      link.download = `${stem}.${view}.json`;
      link.click();
      URL.revokeObjectURL(link.href);
    } catch (cause) {
      setError(
        cause instanceof ApiError
          ? formatApiError(cause)
          : "ดาวน์โหลด JSON ไม่สำเร็จ ลองใหม่อีกครั้ง",
      );
    } finally {
      setDownloading(null);
    }
  }

  if (detail.status === "failed") {
    return (
      <section className="card">
        <h1>{detail.fileName}</h1>
        <p className="failed">ไม่สำเร็จ</p>
        <p>{formatDateTime(detail.createdAt)}</p>
        <div className="error" role="alert">
          {detail.error
            ? formatApiError(detail.error)
            : "การแยกข้อมูลรายการนี้ไม่สำเร็จ"}
        </div>
        <p>
          <Link className="button quiet" to="/history" search={search}>
            กลับไปประวัติ
          </Link>
        </p>
      </section>
    );
  }

  return (
    <section className="card">
      <div className="actions">
        <div>
          <h1>{detail.fileName}</h1>
          <p className="success">สำเร็จ</p>
        </div>
        <Link className="button quiet" to="/history" search={search}>
          กลับไปประวัติ
        </Link>
      </div>
      {error ? (
        <div className="error" role="alert">
          {error}
        </div>
      ) : null}
      <div className="tabs" role="tablist" aria-label="มุมมองข้อมูล">
        <button
          type="button"
          role="tab"
          aria-selected={tab === "summary"}
          aria-controls="summary-panel"
          onClick={() => setTab("summary")}
        >
          วิเคราะห์
        </button>
        <button
          type="button"
          role="tab"
          aria-selected={tab === "raw"}
          aria-controls="raw-panel"
          onClick={() => setTab("raw")}
        >
          ข้อมูลดิบ
        </button>
      </div>
      {tab === "summary" ? (
        <div id="summary-panel" role="tabpanel">
          <div className="actions">
            <button
              type="button"
              disabled={downloading !== null}
              onClick={() => download("normalized")}
            >
              ดาวน์โหลด JSON แบบวิเคราะห์
            </button>
          </div>
          <AnalysisSummary analysis={detail.normalized!} />
        </div>
      ) : (
        <div id="raw-panel" role="tabpanel">
          <div className="actions">
            <button
              type="button"
              disabled={downloading !== null}
              onClick={() => download("raw")}
            >
              ดาวน์โหลด Raw JSON
            </button>
          </div>
          <RawJsonView records={detail.raw!} />
        </div>
      )}
    </section>
  );
}
