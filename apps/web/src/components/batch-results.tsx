import { Link } from "@tanstack/react-router";

import type { BatchCreateResponse } from "../lib/api-types";
import { formatApiError } from "../lib/copy";

export function BatchResults({ result }: { result: BatchCreateResponse }) {
  return (
    <section className="card results-card" aria-labelledby="batch-results-title">
      <div className="section-heading">
        <div>
          <h2 id="batch-results-title">ผลการแยกข้อมูลจาก ZIP</h2>
          <p className="section-note">
            เปิดรายการที่สำเร็จเพื่อดูผลวิเคราะห์ หรือดาวน์โหลดข้อมูลต้นฉบับ
          </p>
        </div>
        <span className="selection-count">
          {result.items.length} ไฟล์
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
                    {succeeded ? " · สำเร็จ" : " · ไม่สำเร็จ"}
                  </span>
                  {!succeeded && item.error ? (
                    <span className="result-error">{formatApiError(item.error)}</span>
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
                  ดูรายละเอียด
                </Link>
              ) : null}
            </li>
          );
        })}
      </ul>
    </section>
  );
}
