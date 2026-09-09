import { useEffect, useRef, useState } from "react";

import {
  createTranscriptEntry,
  deleteAllTranscriptEntries,
  deleteTranscriptEntry,
  listTranscriptEntries,
  updateTranscriptEntry,
  type TranscriptEntry,
  type TranscriptEntryInput,
} from "../../api/transcriptEntries";
import { ConfirmDeleteDialog } from "../../components/confirm-delete-dialog";
import { ApiError, startGoogleLogin } from "../../lib/api";
import { formatApiError } from "../../lib/copy";
import { TranscriptEntryForm } from "./TranscriptEntryForm";

type AdminView = "loading" | "ready" | "forbidden" | "unauthorized" | "error";

function messageForError(error: unknown) {
  if (error instanceof ApiError) {
    if (error.status === 401 || error.code === "AUTH_REQUIRED") return "เซสชันหมดอายุ กรุณาเข้าสู่ระบบอีกครั้ง";
    if (error.status === 403 || error.code === "ADMIN_FORBIDDEN") return "บัญชีนี้ไม่มีสิทธิ์เข้าถึงคิวถอดเสียง";
    if (error.status === 409 || error.code === "TRANSCRIPT_DUPLICATE_VIDEO") {
      return "Video ID นี้มีอยู่ในคิวแล้ว ตรวจสอบรายการเดิมก่อนบันทึก";
    }
    return formatApiError(error);
  }
  return "ระบบทำรายการไม่สำเร็จ ลองใหม่อีกครั้ง";
}

function formatTimestamp(timestamp: number) {
  const date = new Date(timestamp);
  return Number.isNaN(date.getTime()) ? "ไม่ทราบเวลา" : date.toLocaleString("th-TH");
}

export function TranscriptEntriesPage() {
  const [entries, setEntries] = useState<TranscriptEntry[]>([]);
  const [view, setView] = useState<AdminView>("loading");
  const [pageError, setPageError] = useState<string>();
  const [formError, setFormError] = useState<string>();
  const [status, setStatus] = useState("");
  const [editingEntry, setEditingEntry] = useState<TranscriptEntry>();
  const [initialChannelName, setInitialChannelName] = useState("");
  const [formRevision, setFormRevision] = useState(0);
  const [saving, setSaving] = useState(false);
  const [deletingId, setDeletingId] = useState<number>();
  const [deletingAll, setDeletingAll] = useState(false);
  const [deleteEntry, setDeleteEntry] = useState<TranscriptEntry>();
  const [deleteAllOpen, setDeleteAllOpen] = useState(false);
  const deleteTriggerRef = useRef<HTMLButtonElement | null>(null);

  useEffect(() => {
    let cancelled = false;
    void listTranscriptEntries()
      .then((items) => {
        if (cancelled) return;
        setEntries(items);
        setView("ready");
      })
      .catch((error: unknown) => {
        if (cancelled) return;
        setPageError(messageForError(error));
        setView(
          error instanceof ApiError && (error.status === 403 || error.code === "ADMIN_FORBIDDEN")
            ? "forbidden"
            : error instanceof ApiError && (error.status === 401 || error.code === "AUTH_REQUIRED")
              ? "unauthorized"
              : "error",
        );
      });

    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    if (!deleteEntry && deleteTriggerRef.current) {
      deleteTriggerRef.current.focus();
      deleteTriggerRef.current = null;
    }
  }, [deleteEntry]);

  async function handleSave(input: TranscriptEntryInput, mode: "save" | "next") {
    const editingId = editingEntry?.id;
    setSaving(true);
    setFormError(undefined);
    setStatus("");
    try {
      const saved = editingId
        ? await updateTranscriptEntry(editingId, input)
        : await createTranscriptEntry(input);
      const refreshedEntries = await listTranscriptEntries();
      if (!refreshedEntries.some((entry) => entry.id === saved.id)) {
        throw new Error("บันทึกแล้วแต่ไม่พบรายการเมื่ออ่านข้อมูลกลับจาก API");
      }
      setEntries(refreshedEntries);
      setEditingEntry(undefined);
      setInitialChannelName(!editingId && mode === "next" ? input.channelName : "");
      setFormRevision((revision) => revision + 1);
      setStatus(
        editingId
          ? "บันทึกการแก้ไขและอ่านข้อมูลกลับจาก API แล้ว"
          : mode === "next"
            ? "เพิ่มรายการแล้ว พร้อมเพิ่มคลิปถัดไป"
            : "เพิ่มรายการและอ่านข้อมูลกลับจาก API แล้ว",
      );
    } catch (error) {
      setFormError(messageForError(error));
      setStatus("บันทึกไม่สำเร็จ");
      if (error instanceof ApiError && (error.status === 401 || error.status === 403)) {
        setPageError(messageForError(error));
        setView(error.status === 401 ? "unauthorized" : "forbidden");
      }
    } finally {
      setSaving(false);
    }
  }

  async function handleDelete() {
    if (!deleteEntry) return;
    const entryToDelete = deleteEntry;
    setDeletingId(entryToDelete.id);
    setStatus("");
    try {
      await deleteTranscriptEntry(entryToDelete.id);
      const refreshedEntries = await listTranscriptEntries();
      if (refreshedEntries.some((entry) => entry.id === entryToDelete.id)) {
        throw new Error("ลบแล้วแต่ยังพบรายการเมื่ออ่านข้อมูลกลับจาก API");
      }
      setEntries(refreshedEntries);
      if (editingEntry?.id === entryToDelete.id) {
        setEditingEntry(undefined);
        setFormRevision((revision) => revision + 1);
      }
      setInitialChannelName("");
      setStatus("ลบรายการและอ่านข้อมูลกลับจาก API แล้ว");
      setDeleteEntry(undefined);
    } catch (error) {
      setStatus(messageForError(error));
      if (error instanceof ApiError && (error.status === 401 || error.status === 403)) {
        setPageError(messageForError(error));
        setView(error.status === 401 ? "unauthorized" : "forbidden");
      }
    } finally {
      setDeletingId(undefined);
    }
  }

  async function handleDeleteAll() {
    setDeletingAll(true);
    setStatus("");
    try {
      const result = await deleteAllTranscriptEntries();
      const refreshedEntries = await listTranscriptEntries();
      if (refreshedEntries.length !== 0) {
        throw new Error("ล้างรายการแล้วแต่ยังพบข้อมูลเมื่ออ่านข้อมูลกลับจาก API");
      }
      setEntries(refreshedEntries);
      setEditingEntry(undefined);
      setInitialChannelName("");
      setFormRevision((revision) => revision + 1);
      setDeleteAllOpen(false);
      setStatus(`ล้างรายการแล้ว ${result.deleted.toLocaleString("th-TH")} รายการ`);
    } catch (error) {
      setStatus(messageForError(error));
      if (error instanceof ApiError && (error.status === 401 || error.status === 403)) {
        setPageError(messageForError(error));
        setView(error.status === 401 ? "unauthorized" : "forbidden");
      }
    } finally {
      setDeletingAll(false);
    }
  }

  if (view === "loading") {
    return (
      <section className="transcript-admin-page" aria-labelledby="transcript-admin-title">
        <p className="shoe-eyebrow">ADMIN / TRANSCRIPTS</p>
        <h1 id="transcript-admin-title">คิวถอดเสียง</h1>
        <p className="muted" role="status" aria-live="polite">กำลังโหลดรายการ…</p>
      </section>
    );
  }

  if (view === "unauthorized") {
    return (
      <section className="empty-state transcript-access-state" aria-labelledby="transcript-admin-title">
        <span className="empty-state-mark" aria-hidden="true">!</span>
        <p className="shoe-eyebrow">ADMIN / SIGN IN</p>
        <h1 id="transcript-admin-title">กรุณาเข้าสู่ระบบอีกครั้ง</h1>
        <p>{pageError}</p>
        <button type="button" onClick={startGoogleLogin}>เข้าสู่ระบบด้วย Google</button>
      </section>
    );
  }

  if (view === "forbidden") {
    return (
      <section className="empty-state transcript-access-state" aria-labelledby="transcript-admin-title">
        <span className="empty-state-mark" aria-hidden="true">!</span>
        <p className="shoe-eyebrow">ADMIN / FORBIDDEN</p>
        <h1 id="transcript-admin-title">ไม่มีสิทธิ์เข้าถึงคิวถอดเสียง</h1>
        <p>{pageError}</p>
      </section>
    );
  }

  if (view === "error") {
    return (
      <section className="empty-state transcript-access-state" aria-labelledby="transcript-admin-title">
        <span className="empty-state-mark" aria-hidden="true">!</span>
        <p className="shoe-eyebrow">ADMIN / ERROR</p>
        <h1 id="transcript-admin-title">โหลดคิวถอดเสียงไม่สำเร็จ</h1>
        <p>{pageError}</p>
        <button className="secondary" type="button" onClick={() => window.location.reload()}>
          ลองใหม่
        </button>
      </section>
    );
  }

  return (
    <section className="transcript-admin-page" aria-labelledby="transcript-admin-title">
      <header className="page-intro transcript-admin-heading">
        <div>
          <p className="shoe-eyebrow">ADMIN / TRANSCRIPTS</p>
          <h1 id="transcript-admin-title">คิวถอดเสียง</h1>
          <p className="page-lede">จัดการรายการถอดเสียงที่นำเข้าจากแหล่งข้อมูลวิดีโอสำหรับทีมงานภายใน</p>
        </div>
        <p className="transcript-count" role="status" aria-live="polite">{entries.length} รายการ</p>
      </header>

      <TranscriptEntryForm
        key={`transcript-form-${editingEntry?.id ?? "new"}-${formRevision}`}
        busy={saving}
        entry={editingEntry}
        initialChannelName={initialChannelName}
        error={formError}
        onCancel={() => {
          setEditingEntry(undefined);
          setInitialChannelName("");
          setFormRevision((revision) => revision + 1);
          setFormError(undefined);
        }}
        onSubmit={handleSave}
      />

      <div className="transcript-status" aria-live="polite" role="status">
        {status}
      </div>

      <section className="transcript-queue" aria-labelledby="transcript-queue-title">
        <div className="section-heading transcript-queue-heading">
          <div>
            <p className="shoe-eyebrow">QUEUE</p>
            <h2 id="transcript-queue-title">รายการทั้งหมด</h2>
          </div>
          <button
            className="danger"
            type="button"
            disabled={saving || deletingAll || entries.length === 0}
            onClick={() => setDeleteAllOpen(true)}
          >
            ล้างรายการทั้งหมด
          </button>
        </div>
        {entries.length === 0 ? (
          <div className="empty-state transcript-empty-state">
            <span className="empty-state-mark" aria-hidden="true">—</span>
            <h3>ยังไม่มีรายการถอดเสียง</h3>
            <p>เพิ่มรายการแรกจากแบบฟอร์มด้านบน</p>
          </div>
        ) : (
          <div className="transcript-record-list">
            {entries.map((entry) => (
              <article className="card transcript-record" key={entry.id}>
                <div className="transcript-record-heading">
                  <div>
                    <p className="transcript-record-channel">{entry.channelName}</p>
                    <p className="transcript-record-meta">อัปเดต {formatTimestamp(entry.updatedAt)}</p>
                  </div>
                  <div className="table-actions">
                    <button
                      className="secondary"
                      type="button"
                      disabled={saving || deletingAll || deletingId !== undefined}
                      onClick={() => {
                        setEditingEntry(entry);
                        setInitialChannelName("");
                        setFormRevision((revision) => revision + 1);
                        setFormError(undefined);
                      }}
                    >
                      แก้ไข
                    </button>
                    <button
                      className="danger"
                      type="button"
                      disabled={saving || deletingAll || deletingId !== undefined}
                      onClick={(event) => {
                        deleteTriggerRef.current = event.currentTarget;
                        setDeleteEntry(entry);
                      }}
                    >
                      ลบ
                    </button>
                  </div>
                </div>
                <dl className="transcript-record-fields">
                  <div>
                    <dt>Video ID</dt>
                    <dd>{entry.videoId}</dd>
                  </div>
                  <div>
                    <dt>YouTube URL</dt>
                    <dd><a href={entry.youtubeUrl} rel="noreferrer" target="_blank">{entry.youtubeUrl}</a></dd>
                  </div>
                  <div className="transcript-record-transcription">
                    <dt>Transcription</dt>
                    <dd>{entry.transcription}</dd>
                  </div>
                  <div>
                    <dt>สร้างเมื่อ</dt>
                    <dd>{formatTimestamp(entry.createdAt)}</dd>
                  </div>
                </dl>
              </article>
            ))}
          </div>
        )}
      </section>

      {deleteEntry ? (
        <ConfirmDeleteDialog
          busy={deletingId === deleteEntry.id}
          confirmLabel="ยืนยันลบรายการ"
          description={`ลบรายการของช่อง ${deleteEntry.channelName} ใช่หรือไม่ การกระทำนี้ย้อนกลับไม่ได้`}
          onCancel={() => setDeleteEntry(undefined)}
          onConfirm={() => void handleDelete()}
          title="ยืนยันการลบรายการถอดเสียง"
        />
      ) : null}
      {deleteAllOpen ? (
        <ConfirmDeleteDialog
          busy={deletingAll}
          confirmationPhrase="ลบทั้งหมด"
          confirmationPrompt="พิมพ์ ลบทั้งหมด เพื่อยืนยันการล้าง queue"
          confirmLabel="ล้างรายการทั้งหมด"
          description={`รายการทั้งหมด ${entries.length.toLocaleString("th-TH")} รายการจะถูกลบออกจาก PostgreSQL และย้อนกลับไม่ได้`}
          onCancel={() => setDeleteAllOpen(false)}
          onConfirm={() => void handleDeleteAll()}
          title="ยืนยันการล้าง queue ทั้งหมด"
        />
      ) : null}
    </section>
  );
}
