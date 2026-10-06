import { Link } from "@tanstack/react-router";
import type { RunOrder, RunPage, RunSummary } from "../lib/runs-types";
import { formatDateTime, formatDuration, formatPace, formatNumber } from "../lib/formatters";
import { RunProcessingStatus } from "./run-processing-status";
interface HistoryTableProps {
  page: RunPage;
  search: { offset: number; order: RunOrder };
  deletingId: string | null;
  selected: ReadonlySet<string>;
  onSelect: (id: string, selected: boolean) => void;
  onDelete: (item: RunSummary) => void;
  onPageChange: (offset: number) => void;
}
export function HistoryTable({ page, search, deletingId, selected, onSelect, onDelete, onPageChange }: HistoryTableProps) {
  if (page.items.length === 0 && page.offset === 0) return <section className="card empty-state"><h2>ยังไม่มีกิจกรรมวิ่ง</h2><p>เพิ่ม Original FIT หรือ ZIP จาก Garmin แล้วกลับมาเลือกกิจกรรมที่นี่</p><Link className="button secondary" to="/upload">เพิ่มข้อมูลวิ่ง</Link></section>;
  return <section className="card history-card" aria-labelledby="history-table-title">
    <div className="section-heading"><div><h2 id="history-table-title">กิจกรรมวิ่ง</h2><p className="section-note">เรียงตามเวลาของกิจกรรม ไม่ใช่เวลาอัปโหลด · การเลือกส่งออกไม่เปลี่ยนหลักฐาน LT</p></div></div>
    <div className="table-wrap"><table data-testid="history-table"><thead><tr><th>เลือก</th><th>วันที่กิจกรรม</th><th>ระยะทาง</th><th>Timer time</th><th>Pace</th><th>สถานะ</th><th>การทำงาน</th></tr></thead><tbody>
      {page.items.map(item => <tr key={item.id}>
        <td data-label="เลือก"><label className="runs-check"><input type="checkbox" aria-label={`เลือกกิจกรรม ${item.id}`} checked={selected.has(item.id)} onChange={event => onSelect(item.id, event.target.checked)} /><span>เลือก</span></label></td>
        <td data-label="วันที่กิจกรรม">{formatDateTime(item.startTime)}{item.possibleDuplicate ? <p className="section-note">อาจเป็นการวิ่งเดียวกัน · ยังเป็นกิจกรรมแยก</p> : null}</td>
        <td data-label="ระยะทาง">{item.summary.distanceMeters === null ? "ไม่มีข้อมูล" : `${formatNumber(item.summary.distanceMeters / 1000)} km`}</td>
        <td data-label="Timer time">{formatDuration(item.summary.timerTimeSeconds)}</td><td data-label="Pace">{formatPace(item.summary.averagePaceSecondsPerKm)}</td>
        <td data-label="สถานะ"><RunProcessingStatus processing={item.processing} sourceUnavailable={item.sourceUnavailable} /></td>
        <td data-label="การทำงาน"><div className="table-actions"><Link className="button secondary" to="/extractions/$id" params={{ id: item.id }} search={search}>เปิดดู</Link><button type="button" className="danger" disabled={deletingId !== null} aria-label={`ลบกิจกรรม ${item.id}`} onClick={() => onDelete(item)}>ลบ</button></div></td>
      </tr>)}
    </tbody></table></div>
    <div className="pagination"><button className="secondary" type="button" disabled={page.offset === 0 || deletingId !== null} onClick={() => onPageChange(Math.max(0, page.offset - page.limit))}>ก่อนหน้า</button><span>{page.items.length ? page.offset + 1 : 0}–{Math.min(page.offset + page.items.length, page.total)} จาก {page.total}</span><button className="secondary" type="button" disabled={page.offset + page.limit >= page.total || deletingId !== null} onClick={() => onPageChange(page.offset + page.limit)}>ถัดไป</button></div>
  </section>;
}
