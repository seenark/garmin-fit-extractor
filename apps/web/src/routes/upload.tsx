import { Link, createFileRoute, useRouter } from "@tanstack/react-router";
import { useState } from "react";

import { BatchResults } from "../components/batch-results";
import { UploadDropzone } from "../components/upload-dropzone";
import { ApiError, createExtractions } from "../lib/api";
import type { BatchCreateResponse } from "../lib/api-types";
import { formatApiError } from "../lib/copy";

export const Route = createFileRoute("/upload")({ component: UploadPage });

function UploadPage() {
  const router = useRouter();
  const [files, setFiles] = useState<File[]>([]);
  const [result, setResult] = useState<BatchCreateResponse | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [uploading, setUploading] = useState(false);

  async function submit() {
    setError(null);
    setUploading(true);
    try {
      setResult(await createExtractions(files));
      await router.invalidate();
    } catch (cause) {
      setError(
        cause instanceof ApiError
          ? formatApiError(cause)
          : "อัปโหลดไม่สำเร็จ ลองใหม่อีกครั้ง",
      );
    } finally {
      setUploading(false);
    }
  }

  function handleFilesChange(nextFiles: File[]) {
    setFiles(nextFiles);
    setResult(null);
    setError(null);
  }

  return (
    <div className="page-stack">
      <header className="page-intro">
        <div>
          <h1>อัปโหลดไฟล์ ZIP จาก Garmin</h1>
          <p className="page-lede">
            อัปโหลดไฟล์ ZIP ได้ครั้งละ 1–10 ไฟล์ ขนาดไม่เกิน 20 เมกะไบต์ต่อไฟล์
            ระบบจะแยกไฟล์ FIT ออกมาอ่าน แล้วลบทิ้งหลังประมวลผลเสร็จ
          </p>
        </div>
        <Link
          className="button secondary page-intro-action"
          to="/history"
          search={{ offset: 0, order: "desc" }}
        >
          ดูประวัติ
        </Link>
      </header>

      <section className="download-guide card" aria-labelledby="download-guide-title">
        <div className="download-guide-intro">
          <span className="download-guide-kicker">Garmin website only</span>
          <h2 id="download-guide-title">ยังไม่มีไฟล์ ZIP? ดาวน์โหลดจาก Garmin Connect ตามนี้ได้เลย</h2>
          <p>
            ตอนนี้การเอาไฟล์เข้าระบบต้องใช้ <strong>ไฟล์ .zip</strong> ที่ดาวน์โหลดจาก
            <strong> Garmin Connect website</strong> เท่านั้น แล้วค่อยกลับมาอัปโหลดที่หน้านี้
          </p>
          <div className="download-guide-links" aria-label="ลิงก์ไปยัง Garmin Connect">
            <a href="https://connect.garmin.com/app/home" rel="noreferrer" target="_blank">
              เปิด Garmin Connect Home
            </a>
            <a
              href="https://connect.garmin.com/app/activities?activityType=running"
              rel="noreferrer"
              target="_blank"
            >
              เปิดหน้า Activities (Run)
            </a>
          </div>
        </div>

        <ol className="download-guide-steps">
          <li>
            <span className="download-guide-step-number" aria-hidden="true">
              01
            </span>
            <div>
              <h3>Login เข้า Garmin Connect</h3>
              <p>เปิดหน้า Home แล้วเข้าสู่ระบบด้วยบัญชี Garmin ของคุณ</p>
            </div>
          </li>
          <li>
            <span className="download-guide-step-number" aria-hidden="true">
              02
            </span>
            <div>
              <h3>ไปที่ Activities แล้วเลือก Run</h3>
              <p>ใช้ตัวกรองประเภทกิจกรรมเป็น running เพื่อหา session ที่ต้องการ</p>
            </div>
          </li>
          <li>
            <span className="download-guide-step-number" aria-hidden="true">
              03
            </span>
            <div>
              <h3>คลิกชื่อรายการวิ่งใน column Title</h3>
              <p>เลือกรายการตามวันที่ แล้วเปิดหน้า detail ของกิจกรรมวิ่งครั้งนั้น</p>
            </div>
          </li>
          <li>
            <span className="download-guide-step-number" aria-hidden="true">
              04
            </span>
            <div>
              <h3>มองหาไอคอนรูปเฟืองที่มุมขวาบน</h3>
              <p>ไอคอนนี้ค่อนข้างเล็ก ให้สังเกตบริเวณมุมขวาของหน้ารายละเอียดกิจกรรม</p>
            </div>
          </li>
          <li>
            <span className="download-guide-step-number" aria-hidden="true">
              05
            </span>
            <div>
              <h3>เลือก Export File</h3>
              <p>ในเมนูจะมีหลายตัวเลือก ให้เลือก Export File แบบปกติเพื่อดาวน์โหลด .zip</p>
            </div>
          </li>
          <li>
            <span className="download-guide-step-number" aria-hidden="true">
              06
            </span>
            <div>
              <h3>กลับมาอัปโหลดไฟล์ .zip ที่นี่</h3>
              <p>เมื่อได้ไฟล์แล้ว คุณสามารถลากมาวางหรือกดเลือกไฟล์จากคอมพิวเตอร์ได้ทันที</p>
            </div>
          </li>
        </ol>

        <div className="download-guide-note" role="note">
          <strong>หมายเหตุ</strong>
          <p>
            ถ้าหาเมนู export ไม่เจอ ให้เริ่มจากมองหา <strong>รูปเฟือง</strong> ก่อน
            จากนั้นเลือก <strong>Export File</strong> แบบธรรมดา ไม่ต้องเลือก export ชนิดอื่น
          </p>
        </div>
      </section>

      <div className="workbench-grid">
        <UploadDropzone
          files={files}
          disabled={uploading}
          onFilesChange={handleFilesChange}
          onSubmit={submit}
        />
        <aside className="constraints-panel" aria-labelledby="constraints-title">
          <h2 id="constraints-title">ข้อกำหนดการอัปโหลด</h2>
          <p>ไฟล์ ZIP ต้นฉบับจะถูกเก็บเป็นส่วนตัว ไฟล์ FIT ที่แยกออกมาจะถูกลบทิ้งหลังประมวลผล</p>
          <ul className="constraint-list">
            <li>
              <span className="constraint-label">ไฟล์ที่รับ</span>
              <span className="constraint-value">ไฟล์ ZIP</span>
            </li>
            <li>
              <span className="constraint-label">จำนวนไฟล์ต่อครั้ง</span>
              <span className="constraint-value">1–10 ไฟล์</span>
            </li>
            <li>
              <span className="constraint-label">ขนาดสูงสุด</span>
              <span className="constraint-value">20 เมกะไบต์ต่อไฟล์</span>
            </li>
          </ul>
        </aside>
      </div>

      {error ? (
        <div className="error" role="alert">
          <span>{error}</span>
        </div>
      ) : null}
      {result ? <BatchResults result={result} /> : null}
    </div>
  );
}
