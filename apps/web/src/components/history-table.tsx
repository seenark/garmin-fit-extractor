import { Link } from "@tanstack/react-router";

import type { ExtractionPage, ExtractionSummary } from "../lib/api-types";

interface HistoryTableProps {
  page: ExtractionPage;
  deletingId: string | null;
  onDelete: (item: ExtractionSummary) => void;
  onPageChange: (offset: number) => void;
}

function formatSize(bytes: number): string {
  return bytes < 1024 * 1024 ? `${Math.round(bytes / 1024)} KiB` : `${(bytes / (1024 * 1024)).toFixed(1)} MiB`;
}

export function HistoryTable({ page, deletingId, onDelete, onPageChange }: HistoryTableProps) {
  if (page.items.length === 0 && page.offset === 0) {
    return <section className="card"><h1>History</h1><p>No uploads yet. <Link to="/">Upload a FIT file</Link> to begin.</p></section>;
  }
  return (
    <section className="card" aria-labelledby="history-title">
      <h1 id="history-title">History</h1>
      <div className="table-wrap">
        <table data-testid="history-table">
          <thead><tr><th>File</th><th>Size</th><th>Created</th><th>Status</th><th>Activity</th><th>Actions</th></tr></thead>
          <tbody>
            {page.items.map((item) => {
              const succeeded = item.status === "succeeded";
              return <tr key={item.id}>
                <td>{item.fileName}</td><td>{formatSize(item.fileSizeBytes)}</td><td>{new Date(item.createdAt).toLocaleString()}</td>
                <td><span className={succeeded ? "success" : "failed"}>{succeeded ? "Succeeded" : "Failed"}</span>{item.error ? <><br />{item.error.message}</> : null}</td>
                <td>{item.activityType ?? "—"}<br /><span className="muted">{item.activityDate ?? "—"}</span></td>
                <td className="actions"><Link to="/extractions/$id" params={{ id: item.id }}>Open</Link><button className="danger" type="button" disabled={deletingId !== null} aria-label={`Delete ${item.fileName}`} onClick={() => onDelete(item)}>Delete</button></td>
              </tr>;
            })}
          </tbody>
        </table>
      </div>
      <div className="actions">
        <button className="secondary" type="button" disabled={page.offset === 0 || deletingId !== null} onClick={() => onPageChange(Math.max(0, page.offset - page.limit))}>Previous</button>
        <span>{page.offset + 1}–{Math.min(page.offset + page.items.length, page.total)} of {page.total}</span>
        <button className="secondary" type="button" disabled={page.offset + page.limit >= page.total || deletingId !== null} onClick={() => onPageChange(page.offset + page.limit)}>Next</button>
      </div>
    </section>
  );
}
