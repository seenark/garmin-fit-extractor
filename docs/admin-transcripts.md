# Admin transcript queue

The Running Shoe admin surface is integrated into the Runner’s Garage web
application at `/admin/transcripts`. It is not a separate Vite admin service.

## Access

- Sign in with Google through the Garmin application.
- The account email must be present in `ADMIN_EMAILS`.
- `ADMIN_EMAILS` is an allowlist of comma-separated verified Google emails.
- Missing or empty `ADMIN_EMAILS` denies every admin request.
- The allowlisted account sees the `Admin queue` navigation entry after `/api/v1/auth/me` returns `isAdmin: true`.
- Server-side session and allowlist checks remain authoritative even if the browser has stale UI state.

Transcript content is private operational data. It is never returned by public shoe routes, placed in the static catalog, stored in URLs/local storage/analytics, or printed in errors and logs.

## Add a queue entry

The form preserves the original Running Shoe admin workflow and accepts three operator-entered fields:

1. **ชื่อช่อง** — channel/reviewer hint.
2. **YouTube URL** — supported `watch`, `youtu.be`, `shorts`, and `embed` URLs.
3. **Transcription** — text supplied by the operator; line breaks are preserved and rendered as text.

The server derives `video_id` from the YouTube URL. Operators do not need to enter it separately.

- **เพิ่มรายการ** saves and clears the form.
- **บันทึกแล้วเพิ่มคลิปถัดไป** saves the entry and keeps the channel name for the next form.
- Blank fields, non-YouTube URLs, unsupported YouTube paths, short/invalid video IDs, and duplicate video IDs are rejected.

After a successful mutation, the UI reads the queue back from the API before announcing success.

## Review, edit, and delete

- Entries are sorted by `updated_at DESC, id DESC`.
- **แก้ไข** loads the original channel name, URL, and transcription and updates the same PostgreSQL row.
- `created_at` is preserved; `updated_at` always advances, including edits in the same millisecond.
- **ลบ** opens a confirmation dialog and deletes one entry.
- **ล้างรายการทั้งหมด** is disabled for an empty queue and requires typing `ลบทั้งหมด` in the browser. The client then sends the exact API confirmation token `DELETE_ALL`.
- The UI reads the queue back after deletion and verifies that the target state is gone.

## API contract

All paths require a valid Garmin session and an allowlisted email. Mutating requests additionally require an exact `Origin` equal to `GARMIN_FIT_APP_ORIGIN`.

| Method | Path | Purpose |
| --- | --- | --- |
| `GET` | `/api/admin/transcript-entries` | List all entries, newest updated first |
| `GET` | `/api/admin/transcript-entries/:id` | Read one entry |
| `POST` | `/api/admin/transcript-entries` | Validate and create an entry; returns `201` |
| `PUT` | `/api/admin/transcript-entries/:id` | Validate and update an entry |
| `DELETE` | `/api/admin/transcript-entries/:id` | Delete one entry; returns `204` |
| `DELETE` | `/api/admin/transcript-entries` | Delete all entries with `{ "confirmation": "DELETE_ALL" }` |

The collection delete response is `{ "deleted": number }`. Invalid collection confirmation returns `400` with code `TRANSCRIPT_CONFIRMATION_REQUIRED`. Duplicate `video_id` values return `409` with code `TRANSCRIPT_DUPLICATE_VIDEO`; database details are not exposed.

## Storage and migration

Queue data is stored in PostgreSQL in `transcript_entries`. The legacy SQLite row is imported with its explicit ID, including the historical row ID `56`, by the idempotent legacy migrator. The static running-shoe catalog remains checked-in frontend data and is not moved into PostgreSQL.

Do not place PostgreSQL dumps, raw transcription text, credentials, session values, OAuth values, or production snapshots in Git.
