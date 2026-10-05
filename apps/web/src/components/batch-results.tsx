import { Link } from "@tanstack/react-router";

import type { RunImportResult } from "../lib/runs-types";

export function BatchResults({ result }: { result: RunImportResult }) {
  const { imported, duplicate, unsupported, failed } = result.counts;
  const warningCount = result.items.reduce((count, item) => count + item.warnings.length, 0);
  const partialSuccess = imported + duplicate > 0 && unsupported + failed > 0;
  const labels = {
    imported: "นำเข้าแล้ว",
    duplicate: "ซ้ำกับกิจกรรมที่มีอยู่",
    unsupported: "ไม่รองรับ",
    failed: "นำเข้าไม่สำเร็จ",
  };

  return (
    <section className="card results-card" aria-labelledby="batch-results-title" aria-live="polite">
      <div className="section-heading">
        <div>
          <h2 id="batch-results-title">{partialSuccess ? "นำเข้าสำเร็จบางส่วน" : "ผลการนำเข้า"}</h2>
          <p className="section-note">
            นำเข้า {imported} · ซ้ำ {duplicate} · ไม่รองรับ {unsupported} · ไม่สำเร็จ {failed} · คำเตือน {warningCount}
          </p>
          {imported > 0 ? (
            <p className="section-note">
              รับกิจกรรมที่นำเข้าแล้วเข้าคิวประมวลผล ผลวิเคราะห์อาจยังไม่พร้อม เปิดรายละเอียดเพื่อตรวจสอบสถานะ
            </p>
          ) : null}
        </div>
        <span className="selection-count">{result.items.length} รายการ</span>
      </div>
      <ul className="result-list">
        {result.items.map((item) => {
          const statusClass = item.status === "imported"
            ? "success"
            : item.status === "failed" ? "failed" : "section-note";
          const activityId = item.status === "imported" || item.status === "duplicate" ? item.activityId : null;
          return (
            <li
              key={`${result.batchId}-${item.index}`}
              className="result-row"
              data-testid="batch-result"
              data-status={item.status}
            >
              <div className="result-summary">
                <span className={`status-dot ${statusClass}`} aria-hidden="true" />
                <div className="result-copy">
                  <strong>{item.name}</strong>
                  <span className={statusClass}> · {labels[item.status]}</span>
                  {item.reason ? (
                    <div className={item.status === "failed" ? "result-error" : "section-note"}>{item.reason}</div>
                  ) : null}
                  {item.warnings.length > 0 ? (
                    <ul className="section-note" aria-label={`คำเตือนของ ${item.name}`}>
                      {item.warnings.map((warning, index) => <li key={index}>{warning}</li>)}
                    </ul>
                  ) : null}
                </div>
              </div>
              {activityId ? (
                <Link
                  className="button secondary"
                  to="/extractions/$id"
                  params={{ id: activityId }}
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
