import { Link } from "@tanstack/react-router";

import type { BatchCreateResponse } from "../lib/api-types";

export function BatchResults({ result }: { result: BatchCreateResponse }) {
  return (
    <section className="card results-card" aria-labelledby="batch-results-title">
      <div className="section-heading">
        <div>
          <h2 id="batch-results-title">ZIP extraction results</h2>
          <p className="section-note">
            Open a successful extraction for normalized analysis and raw decoded JSON.
          </p>
        </div>
        <span className="selection-count">
          {result.items.length} {result.items.length === 1 ? "archive" : "archives"}
        </span>
      </div>
      <ul className="result-list">
        {result.items.map((item) => {
          const succeeded = item.status === "succeeded";
          return (
            <li
              key={item.id}
              className="result-row"
              data-testid="batch-result"
            >
              <div className="result-summary">
                <span
                  className={`status-dot ${succeeded ? "success" : "failed"}`}
                  aria-hidden="true"
                />
                <span className="result-copy">
                  <strong>{item.fileName}</strong>
                  <span className={succeeded ? "success" : "failed"}>
                    {succeeded ? " · Succeeded" : " · Failed"}
                  </span>
                  {!succeeded && item.error ? (
                    <span className="result-error">{item.error.message}</span>
                  ) : null}
                </span>
              </div>
              {succeeded ? (
                <Link
                  className="button secondary"
                  to="/extractions/$id"
                  params={{ id: item.id }}
                  search={{ offset: 0, order: "desc" }}
                >
                  View details
                </Link>
              ) : null}
            </li>
          );
        })}
      </ul>
    </section>
  );
}
