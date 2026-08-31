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
