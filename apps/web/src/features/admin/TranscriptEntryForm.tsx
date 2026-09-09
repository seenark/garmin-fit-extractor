import { useEffect, useState, type FormEvent } from "react";

import type { TranscriptEntry, TranscriptEntryInput } from "../../api/transcriptEntries";

type TranscriptEntryFormProps = {
  entry?: TranscriptEntry;
  initialChannelName?: string;
  busy: boolean;
  error?: string;
  onCancel?: () => void;
  onSubmit: (input: TranscriptEntryInput, mode: "save" | "next") => void;
};

const emptyEntry = {
  channelName: "",
  youtubeUrl: "",
  transcription: "",
} satisfies TranscriptEntryInput;

export function TranscriptEntryForm({
  entry,
  initialChannelName = "",
  busy,
  error,
  onCancel,
  onSubmit,
}: TranscriptEntryFormProps) {
  const [fields, setFields] = useState<TranscriptEntryInput>(() =>
    entry
      ? {
          channelName: entry.channelName,
          youtubeUrl: entry.youtubeUrl,
          transcription: entry.transcription,
        }
      : { ...emptyEntry, channelName: initialChannelName },
  );
  const [validationError, setValidationError] = useState<string>();

  useEffect(() => {
    setFields(
      entry
        ? {
            channelName: entry.channelName,
            youtubeUrl: entry.youtubeUrl,
            transcription: entry.transcription,
          }
        : { ...emptyEntry, channelName: initialChannelName },
    );
    setValidationError(undefined);
  }, [entry, initialChannelName]);

  function updateField(field: keyof TranscriptEntryInput, value: string) {
    setFields((current) => ({ ...current, [field]: value }));
    setValidationError(undefined);
  }

  function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (
      !fields.channelName.trim() ||
      !fields.youtubeUrl.trim() ||
      !fields.transcription.trim()
    ) {
      setValidationError("กรอกข้อมูลให้ครบทั้งสามช่องก่อนบันทึก");
      return;
    }
    const submitter = (event.nativeEvent as SubmitEvent & { submitter?: HTMLButtonElement | null })
      .submitter;
    onSubmit(fields, submitter?.value === "next" ? "next" : "save");
  }

  return (
    <form className="transcript-form" onSubmit={handleSubmit}>
      <div className="transcript-form-heading">
        <div>
          <p className="shoe-eyebrow">{entry ? "EDIT ENTRY" : "NEW ENTRY"}</p>
          <h2>{entry ? "แก้ไขรายการถอดเสียง" : "เพิ่มรายการถอดเสียง"}</h2>
        </div>
        {entry && onCancel ? (
          <button className="quiet" type="button" onClick={onCancel} disabled={busy}>
            ยกเลิกแก้ไข
          </button>
        ) : null}
      </div>

      <div className="transcript-form-grid">
        <label className="field-label" htmlFor="transcript-channel-name">
          ชื่อช่อง
          <input
            id="transcript-channel-name"
            onChange={(event) => updateField("channelName", event.target.value)}
            required
            value={fields.channelName}
          />
        </label>
        <label className="field-label" htmlFor="transcript-youtube-url">
          YouTube URL
          <input
            id="transcript-youtube-url"
            inputMode="url"
            onChange={(event) => updateField("youtubeUrl", event.target.value)}
            required
            type="url"
            value={fields.youtubeUrl}
          />
        </label>
        <label className="field-label transcript-form-transcription" htmlFor="transcript-text">
          Transcription
          <textarea
            id="transcript-text"
            onChange={(event) => updateField("transcription", event.target.value)}
            required
            rows={10}
            value={fields.transcription}
          />
        </label>
      </div>

      {validationError || error ? (
        <p className="inline-error" role="alert">
          {validationError ?? error}
        </p>
      ) : null}
      <div className="transcript-form-actions">
        <button type="submit" disabled={busy} aria-busy={busy} value="save">
          {busy ? "กำลังบันทึก…" : entry ? "บันทึกการแก้ไข" : "เพิ่มรายการ"}
        </button>
        {!entry ? (
          <button className="secondary" type="submit" disabled={busy} aria-busy={busy} value="next">
            บันทึกแล้วเพิ่มคลิปถัดไป
          </button>
        ) : null}
        {entry && onCancel ? (
          <button className="secondary" type="button" onClick={onCancel} disabled={busy}>
            ยกเลิก
          </button>
        ) : null}
      </div>
    </form>
  );
}
