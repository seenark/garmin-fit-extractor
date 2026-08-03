import { Link } from "@tanstack/react-router";

import type { BatchCreateResponse } from "../lib/api-types";

export function BatchResults({ result }: { result: BatchCreateResponse }) {
  return (
    <section className="card" aria-labelledby="batch-results-title">
      <h2 id="batch-results-title">Extraction results</h2>
      <ul className="result-list">
        {result.items.map((item) => {
          const succeeded = item.status === "succeeded";
          return (
            <li key={item.id} data-testid="batch-result">
              <span>
                <strong>{item.fileName}</strong>
                <span className={succeeded ? "success" : "failed"}> · {succeeded ? "Succeeded" : "Failed"}</span>
                {!succeeded && item.error ? <span> — {item.error.message}</span> : null}
              </span>
              {succeeded ? <Link to="/extractions/$id" params={{ id: item.id }}>View details</Link> : null}
            </li>
          );
        })}
      </ul>
    </section>
  );
}
