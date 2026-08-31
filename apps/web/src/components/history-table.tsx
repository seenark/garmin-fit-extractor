import { Link } from "@tanstack/react-router";
import type { ExtractionPage, ExtractionSummary } from "../lib/api-types";
import { formatDate, formatDateTime, formatFileSize } from "../lib/formatters";

interface HistoryTableProps {
  page: ExtractionPage;
  order: "asc" | "desc";
  deletingId: string | null;
  onDelete: (item: ExtractionSummary) => void;
  onPageChange: (offset: number) => void;
}

export function HistoryTable({
  page,
  order,
  deletingId,
  onDelete,
  onPageChange,
}: HistoryTableProps) {
  if (page.items.length === 0 && page.offset === 0) {
    return (
      <section className="card empty-state" aria-labelledby="empty-history-title">
        <span className="empty-state-mark" aria-hidden="true">
          <svg
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeLinecap="round"
            strokeLinejoin="round"
            strokeWidth="1.6"
          >
            <path d="M4 5h16v14H4z" />
            <path d="M8 9h8M8 13h5" />
          </svg>
        </span>
        <h2 id="empty-history-title">No uploads yet.</h2>
        <p>Upload a ZIP file to see extracted activity data here.</p>
        <Link className="button secondary" to="/upload">
          Upload a ZIP file
        </Link>
      </section>
    );
  }

  return (
    <section className="card history-card" aria-labelledby="history-table-title">
      <div className="section-heading">
        <div>
          <h2 id="history-table-title">Recent extractions</h2>
          <p className="section-note">
            Activity records are ordered by the date recorded in each archive.
          </p>
        </div>
        <span className="history-total">
          {page.total} saved {page.total === 1 ? "extraction" : "extractions"}
        </span>
      </div>
      <div className="table-wrap">
        <table data-testid="history-table">
          <thead>
            <tr>
              <th>File name</th>
              <th>File size</th>
              <th>Activity date</th>
              <th>Activity type</th>
              <th>Uploaded</th>
              <th>Status</th>
              <th>Actions</th>
            </tr>
          </thead>
          <tbody>
            {page.items.map((item) => {
              const succeeded = item.status === "succeeded";
              return (
                <tr key={item.id}>
                  <td className="file-cell" data-label="File name">
                    {item.fileName}
                  </td>
                  <td data-label="File size">{formatFileSize(item.fileSizeBytes)}</td>
                  <td data-label="Activity date">{formatDate(item.activityDate)}</td>
                  <td data-label="Activity type">
                    {item.activityType ?? "Activity type not recorded"}
                  </td>
                  <td data-label="Uploaded">{formatDateTime(item.createdAt)}</td>
                  <td data-label="Status">
                    <span className={`status-badge ${succeeded ? "success" : "failed"}`}>
                      <span className="status-dot" aria-hidden="true" />
                      {succeeded ? "Succeeded" : "Failed"}
                    </span>
                    {item.error ? (
                      <span className="table-error">{item.error.message}</span>
                    ) : null}
                  </td>
                  <td data-label="Actions">
                    <div className="table-actions">
                      <Link
                        className="button secondary"
                        to="/extractions/$id"
                        params={{ id: item.id }}
                        search={{ offset: page.offset, order }}
                      >
                        Open
                      </Link>
                      <button
                        className="danger"
                        type="button"
                        disabled={deletingId !== null}
                        aria-label={`Delete ${item.fileName}`}
                        onClick={() => onDelete(item)}
                      >
                        Delete
                      </button>
                    </div>
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
      </div>
      <div className="pagination">
        <button
          className="secondary"
          type="button"
          disabled={page.offset === 0 || deletingId !== null}
          onClick={() => onPageChange(Math.max(0, page.offset - page.limit))}
        >
          Previous
        </button>
        <span>
          {page.offset + 1}–{Math.min(page.offset + page.items.length, page.total)} of {page.total}
        </span>
        <button
          className="secondary"
          type="button"
          disabled={
            page.offset + page.limit >= page.total || deletingId !== null
          }
          onClick={() => onPageChange(page.offset + page.limit)}
        >
          Next
        </button>
      </div>
    </section>
  );
}
