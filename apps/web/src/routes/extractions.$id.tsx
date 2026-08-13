import { Link, createFileRoute } from "@tanstack/react-router";
import { useState } from "react";

import { AnalysisSummary } from "../components/analysis-summary";
import { RawJsonView } from "../components/raw-json-view";
import { ApiError, downloadExtraction, getExtraction } from "../lib/api";

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
          ? cause.message
          : "The JSON download could not be completed.",
      );
    } finally {
      setDownloading(null);
    }
  }

  if (detail.status === "failed") {
    return (
      <section className="card">
        <h1>{detail.fileName}</h1>
        <p className="failed">Failed</p>
        <p>{new Date(detail.createdAt).toLocaleString()}</p>
        <div className="error" role="alert">
          {detail.error?.message ?? "This extraction failed."}
        </div>
        <p>
          <Link to="/history" search={search}>
            Return to history
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
          <p className="success">Succeeded</p>
        </div>
        <Link to="/history" search={search}>
          Back to history
        </Link>
      </div>
      {error ? (
        <div className="error" role="alert">
          {error}
        </div>
      ) : null}
      <div className="tabs" role="tablist" aria-label="Extraction views">
        <button
          type="button"
          role="tab"
          aria-selected={tab === "summary"}
          aria-controls="summary-panel"
          onClick={() => setTab("summary")}
        >
          Summary
        </button>
        <button
          type="button"
          role="tab"
          aria-selected={tab === "raw"}
          aria-controls="raw-panel"
          onClick={() => setTab("raw")}
        >
          Raw
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
              Download normalized JSON
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
              Download raw JSON
            </button>
          </div>
          <RawJsonView records={detail.raw!} />
        </div>
      )}
    </section>
  );
}
