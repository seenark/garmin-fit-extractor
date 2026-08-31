import { Link } from "@tanstack/react-router";
import type { ExtractionPage, ExtractionSummary } from "../lib/api-types";
import { formatApiError } from "../lib/copy";
import {
  formatActivityType,
  formatDate,
  formatDateTime,
  formatFileSize,
} from "../lib/formatters";

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
        <h2 id="empty-history-title">ยังไม่มีไฟล์ที่อัปโหลด</h2>
        <p>อัปโหลด ZIP แล้วข้อมูลกิจกรรมที่แยกได้จะแสดงที่นี่</p>
        <Link className="button secondary" to="/upload">
          อัปโหลดไฟล์ ZIP
        </Link>
      </section>
    );
  }

  return (
    <section className="card history-card" aria-labelledby="history-table-title">
      <div className="section-heading">
        <div>
          <h2 id="history-table-title">รายการที่แยกข้อมูลล่าสุด</h2>
          <p className="section-note">
            รายการเรียงตามวันที่ของกิจกรรมในแต่ละไฟล์
          </p>
        </div>
        <span className="history-total">
          {page.total} รายการที่บันทึกไว้
        </span>
      </div>
      <div className="table-wrap">
        <table data-testid="history-table">
          <thead>
            <tr>
              <th>ชื่อไฟล์</th>
              <th>ขนาดไฟล์</th>
              <th>วันที่กิจกรรม</th>
              <th>ประเภทกิจกรรม</th>
              <th>อัปโหลดเมื่อ</th>
              <th>สถานะ</th>
              <th>การทำงาน</th>
            </tr>
          </thead>
          <tbody>
            {page.items.map((item) => {
              const succeeded = item.status === "succeeded";
              return (
                <tr key={item.id}>
                  <td className="file-cell" data-label="ชื่อไฟล์">
                    {item.fileName}
                  </td>
                  <td data-label="ขนาดไฟล์">{formatFileSize(item.fileSizeBytes)}</td>
                  <td data-label="วันที่กิจกรรม">{formatDate(item.activityDate)}</td>
                  <td data-label="ประเภทกิจกรรม">
                    {item.activityType
                      ? formatActivityType(item.activityType)
                      : "ไม่ได้บันทึกประเภทกิจกรรม"}
                  </td>
                  <td data-label="อัปโหลดเมื่อ">{formatDateTime(item.createdAt)}</td>
                  <td data-label="สถานะ">
                    <span className={`status-badge ${succeeded ? "success" : "failed"}`}>
                      <span className="status-dot" aria-hidden="true" />
                      {succeeded ? "สำเร็จ" : "ไม่สำเร็จ"}
                    </span>
                    {item.error ? (
                      <span className="table-error">{formatApiError(item.error)}</span>
                    ) : null}
                  </td>
                  <td data-label="การทำงาน">
                    <div className="table-actions">
                      <Link
                        className="button secondary"
                        to="/extractions/$id"
                        params={{ id: item.id }}
                        search={{ offset: page.offset, order }}
                      >
                        เปิดดู
                      </Link>
                      <button
                        className="danger"
                        type="button"
                        disabled={deletingId !== null}
                        aria-label={`ลบ ${item.fileName}`}
                        onClick={() => onDelete(item)}
                      >
                        ลบ
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
          ก่อนหน้า
        </button>
        <span>
          {page.offset + 1}–{Math.min(page.offset + page.items.length, page.total)} จาก {page.total}
        </span>
        <button
          className="secondary"
          type="button"
          disabled={
            page.offset + page.limit >= page.total || deletingId !== null
          }
          onClick={() => onPageChange(page.offset + page.limit)}
        >
          ถัดไป
        </button>
      </div>
    </section>
  );
}
