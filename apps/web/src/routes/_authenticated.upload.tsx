import { Link, createFileRoute, useRouter } from "@tanstack/react-router";
import { useState } from "react";

import { BatchResults } from "../components/batch-results";
import { UploadDropzone } from "../components/upload-dropzone";
import { ApiError, startGoogleLogin } from "../lib/api";
import { importRuns } from "../lib/runs-api";
import type { RunImportResult } from "../lib/runs-types";
import { formatApiError } from "../lib/copy";
import { Route as RootRoute } from "./__root";

export const Route = createFileRoute("/_authenticated/upload")({ component: UploadPage });

function UploadPage() {
  const router = useRouter();
  const { user } = RootRoute.useLoaderData();
  const [files, setFiles] = useState<File[]>([]);
  const [result, setResult] = useState<RunImportResult | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [uploading, setUploading] = useState(false);

  async function submit() {
    setError(null);
    setResult(null);
    setUploading(true);
    try {
      setResult(await importRuns(files));
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
          <h1>อัปโหลดไฟล์ FIT หรือ ZIP จาก Garmin</h1>
          <p className="page-lede">
            อัปโหลดไฟล์ FIT โดยตรงหรือ ZIP ได้ครั้งละ 1–10 ไฟล์ ไฟล์ต้องไม่ว่างและไม่เกิน 20 MiB ต่อไฟล์
            ระบบเก็บ OriginalFIT เป็นส่วนตัวเพื่อประมวลผลใหม่ ส่วน ZIP ใช้รับไฟล์เข้าระบบเท่านั้น
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
          <span className="download-guide-kicker">Garmin Connect</span>
          <h2 id="download-guide-title">ยังไม่มีไฟล์? ดาวน์โหลด Original จาก Garmin Connect</h2>
          <p>
            รับทั้ง <strong>ไฟล์ .fit</strong> โดยตรงและ <strong>ไฟล์ .zip</strong> ที่มี FIT อยู่ภายใน
            ถ้าต้องการดาวน์โหลดจาก <strong>Garmin Connect website</strong> ให้ทำตามขั้นตอนนี้
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
              <h3>เลือก Export Original</h3>
              <p>เลือก Export Original เพื่อดาวน์โหลดไฟล์ต้นฉบับ ซึ่งอาจเป็น .fit หรือ .zip ที่มี FIT อยู่ภายใน</p>
            </div>
          </li>
          <li>
            <span className="download-guide-step-number" aria-hidden="true">
              06
            </span>
            <div>
              <h3>กลับมาอัปโหลดไฟล์ .fit หรือ .zip ที่นี่</h3>
              <p>เมื่อได้ไฟล์แล้ว คุณสามารถลากมาวางหรือกดเลือกไฟล์จากคอมพิวเตอร์ได้ทันที</p>
            </div>
          </li>
        </ol>

        <div className="download-guide-note" role="note">
          <strong>หมายเหตุ</strong>
          <p>
            ถ้าหาเมนู export ไม่เจอ ให้เริ่มจากมองหา <strong>รูปเฟือง</strong> ก่อน
            จากนั้นเลือก <strong>Export Original</strong> ไม่ใช่ GPX หรือ TCX
          </p>
        </div>
      </section>

      <div className="workbench-grid">
        {user ? (
          <UploadDropzone
            files={files}
            disabled={uploading}
            onFilesChange={handleFilesChange}
            onSubmit={submit}
          />
        ) : (
          <section className="card upload-access-card" aria-labelledby="upload-access-title">
            <div>
              <p className="upload-access-label">พร้อมอัปโหลดเมื่อคุณพร้อม</p>
              <h2 id="upload-access-title">อ่าน guide ได้ก่อน โดยยังไม่ต้องเข้าสู่ระบบ</h2>
              <p>
                เมื่อมีไฟล์ FIT หรือ ZIP จาก Garmin แล้ว ค่อยเข้าสู่ระบบด้วย Google
                เพื่อเลือกไฟล์และนำเข้าข้อมูลวิ่ง
              </p>
            </div>
            <button type="button" onClick={startGoogleLogin}>
              เข้าสู่ระบบเพื่ออัปโหลด
            </button>
          </section>
        )}
        <aside className="constraints-panel" aria-labelledby="constraints-title">
          <h2 id="constraints-title">ข้อกำหนดการอัปโหลด</h2>
          <p>OriginalFIT เก็บเป็นส่วนตัวเพื่อประมวลผลใหม่ ZIP ใช้รับไฟล์เข้าระบบเท่านั้น การนำเข้าสำเร็จไม่ได้หมายความว่าผลวิเคราะห์พร้อมแล้ว</p>
          <ul className="constraint-list">
            <li>
              <span className="constraint-label">ไฟล์ที่รับ</span>
              <span className="constraint-value">FIT หรือ ZIP ที่มี FIT</span>
            </li>
            <li>
              <span className="constraint-label">จำนวนไฟล์ต่อครั้ง</span>
              <span className="constraint-value">1–10 ไฟล์</span>
            </li>
            <li>
              <span className="constraint-label">ขนาดสูงสุด</span>
              <span className="constraint-value">มากกว่า 0 และไม่เกิน 20 MiB ต่อไฟล์</span>
            </li>
            <li>
              <span className="constraint-label">ชื่อไฟล์</span>
              <span className="constraint-value">ไม่เกิน 255 ไบต์ UTF-8 และไม่มีอักขระควบคุม</span>
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
