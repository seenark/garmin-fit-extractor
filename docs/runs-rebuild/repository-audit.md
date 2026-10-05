# Runs Rebuild — Repository audit

สถานะ: **สำรวจแล้ว / แผนยังไม่อนุมัติ** ตรวจเมื่อ 2026-10-04 โดยอ้าง checkout จริง ไม่ใช่ production database เอกสาร Runs Rebuild Requirements 1.0 ที่ผู้ใช้แนบเป็น product contract; IDs ในชุดเอกสารนี้อ้างข้อกำหนดนั้น ไม่สืบทอดร่าง LT1-only เดิม

## 1. Snapshot และวิธีตรวจ

- Branch `main` ติดตาม `origin/main`; HEAD `d90051ce2cb398678a7c16b6b981724673e8028f` จาก `git rev-parse HEAD`.
- `git status --short --branch` ตอนเริ่มไม่มีรายการแก้ไข tracked/untracked ไม่ reset หรือเปลี่ยน branch ของผู้ใช้
- ค้นแล้วไม่พบ `AGENTS.md`, `CLAUDE.md`, `CONTEXT.md`, `CONTEXT-MAP.md` หรือ ADR directory เดิมในไฟล์ที่ไม่ถูก ignore จึงเพิ่ม glossary [CONTEXT.md](../../CONTEXT.md) และ proposed ADRs สำหรับรอบนี้
- Codegraph ไม่มี index จึงใช้ read-only scouts และอ่าน source/manifest/migrations โดยตรง ไม่สร้าง index
- ไม่อ่าน `.env`, credentials, `db-data`, production data หรือ FIT ส่วนตัว ไม่ต่อ production DB และไม่ apply migration/reset/deploy กับระบบเดิม
- Baseline ใช้ `git archive HEAD` ใน directory ชั่วคราว ซึ่งไม่มี `.env`, แล้ว `bun install --frozen-lockfile`. PostgreSQL 18 เป็น container ใหม่ชื่อสุ่ม มี DB/user/password ชั่วคราวและ port loopback ที่สุ่ม ไม่ใช้ Compose ของ checkout
- `legacy-data-migrator prepare-target` ทำ migrations **เฉพาะ DB ชั่วคราว** ก่อน baseline. Tests เดิมมี `TRUNCATE ... CASCADE` จึงห้ามนำ URL ของฐานข้อมูลที่ต้องเก็บมาใช้

## 2. Architecture และ versions ปัจจุบัน

| ส่วน | สิ่งที่ตรวจพบ |
|---|---|
| API | Rust edition 2024, Axum 0.8.9, SQLx 0.9.0, PostgreSQL, Google session/OAuth/FIT Coach และ admin transcript อยู่ใน `apps/api` |
| Decoder API | `fitparser` 0.11.0; ZIP resolved 2.4.2; raw wrapper ใน `apps/api/src/fit/raw.rs` |
| Web | React/TanStack Router; `@tanstack/charts` pin 0.16.0; Vite ใน baseline resolved 8.1.5; ไม่มี direct PNG/image-export dependency |
| CLI | `packages/cli`; `@garmin/fitsdk` range ^21.205.0 resolved 21.208.0; CLI ใช้ decoder/normalizer คนละ implementation กับ Rust |
| Package/toolchain | `packageManager` Bun 1.3.14; เครื่องตรวจ Bun 1.3.14, cargo/rustc 1.95.0, Python 3.12.13; Docker client/server 29.4.0 |
| Deploy | Compose PostgreSQL image `postgres:18`, persistent `./db-data:/var/lib/postgresql`; app เป็น unified image; Dockerfile build Rust 1.94 และ Bun 1.3.14; runtime non-root UID 10001 |

Manifest ranges ไม่ใช่ exact installed versions ทั้งหมด การ implement ต้อง pin lockfile/package/profile/options ที่ใช้จริง ผล baseline บน host Rust 1.95 ไม่ใช่หลักฐานว่า production Docker Rust 1.94 build ผ่าน

หลักฐาน: [README](../../README.md), [root scripts](../../package.json), [API dependencies](../../apps/api/Cargo.toml), [web dependencies](../../apps/web/package.json), [CLI dependencies](../../packages/cli/package.json), `Cargo.lock`, `bun.lock`, [Compose](../../compose.yaml), [Dockerfile](../../Dockerfile).

## 3. Flow ที่มีอยู่และช่องว่าง

### Import / decode / storage

`multipart files` ใน `routes/extractions.rs` รับ ZIP 1–10 ไฟล์ ไม่รับ direct FIT. แต่ละ ZIP ≤20 MiB, body ≤210 MiB, FIT members ≤50, member ≤20 MiB และ extracted FIT รวม ≤100 MiB. มี path checks และ hard byte checks ระหว่างอ่าน ไม่เขียน member เข้า public directory; per-member failure ไม่ทำให้ valid siblings ล้มทั้งหมด

`fit::raw::decode_raw` เรียก `fitparser::from_bytes`; normalizer สร้าง normalized JSON; `db::insert_success` insert extraction และ FIT Coach activity projection ใน transaction เดียวเมื่อมี activity date. **ไม่เก็บ FIT หรือ ZIP bytes, checksum, source revision หรือ decoder version**. การนำเข้าเดิมเป็นงาน synchronous ไม่มี durable jobs/leases/recovery/reprocessing

Tests ที่รันจริง `fit_decode.rs` ผ่าน CRC corruption rejection และ default CRC validation. แม้ wrapper ไม่เรียก CRC function แยก จึงไม่ควรอ้างว่าเส้นทางเดิมไม่มี integrity validation อย่างไรก็ตามยังต้องเพิ่ม header/content compatibility, resource budget, explicit symlink/nested policy, Garmin running-only และ multisession policy ตาม IMP-01–09

### Decoded / normalized fidelity

- `RawFitRecord` มี `kind` และ `fields`; `RawFitField` มี `name`, `value`, optional `units` (`model.rs:424–438`). Wrapper ทิ้ง numeric field/message identity, developer identity และ validity/type/source metadata ที่ decoder อาจเปิดให้
- เก็บ unsafe JSON integers เป็น decimal strings, arrays recurse, nonfinite/invalid เป็น null (`raw.rs:37–81`). จึงยังไม่แยก absent / FIT-invalid / decode failure แบบ DEC-05
- `ActivitySample` มี index, timestamp, elapsed seconds, HR และ power เท่านั้น (`model.rs:149–157`). ยังไม่มี normalized speed/altitude/cadence/source mapping ที่ครบ
- `normalize_cadence` ใช้ heuristic ต่ำกว่า 130 แล้วคูณสอง; `average_record_value` ใช้ค่าเฉลี่ยราย row และ normalizer ปัดค่า (`normalize.rs:256–294`). ต้องเปลี่ยนเป็น field-definition/time-weighted/provenance policies ไม่ขยาย heuristic เหล่านี้ไป schema ใหม่
- `routes/activities.rs` มี lap-half HR drift เป็น derived metric เดิม ไม่ใช่ LT estimator และไม่ควรใช้แทน time-series/workload analysis
- รายละเอียด decoder spike อยู่ใน [decoder-assessment.md](decoder-assessment.md); ไม่มี claim ว่า raw archive เดิมคือ encoded/full metadata 100%

### Web / exports / graphs

- `/upload` และ `upload-validation.ts` รับ ZIP เท่านั้น มี per-member outcomes
- `/history` โหลด 50 แถวต่อหน้า ใช้ URL `offset/order` และไม่มี checkbox/export selection
- `/extractions/$id` โหลด extraction เดี่ยว มี analysis/raw tabs และ Copy/Download normalized/raw; ใช้ pretty JSON เดิมได้ แต่ **ยังไม่ใช่ coach/full selection/privacy contracts ใหม่**
- `json-copy-actions.tsx` ใช้ Clipboard API และ success/failure states; download filename เดิมอิง uploaded filename ต้องไม่นำชื่อที่ไม่จำเป็นเข้า payload/PNG ใหม่
- `activity-charts.tsx` / `activity-chart-data.ts` ใช้ `@tanstack/charts`; มี lap pace, zones, record power และ metrics บางชนิด ยังไม่ใช่ native sample pace/HR/power linked timelines + explainable detected segments ทั้งชุด
- ไม่พบ LT engine, as-of snapshots, revision trend, selected-only batch exports, recursive privacy projection, prompts แยก หรือ PNG Share Card ในเส้นทางที่สำรวจ

Existing routes/components เป็นจุดเริ่ม ไม่ต้องรื้อ application shell, Shoes หรือ auth เพื่อเพิ่ม workflow ใหม่

## 4. Database dependency map และ protected areas

Migrations `0001_extractions.sql` ถึง `0005_legacy_imports.sql` มี application tables 11 ตารางดังนี้ (`_sqlx_migrations` เป็น migration metadata ของ SQLx ไม่ใช่ Runs data):

| ตาราง | Ownership และ dependency |
|---|---|
| `extractions` | Runs root; `user_id -> users.id ON DELETE CASCADE` |
| `activities` | FIT Coach projection 1:1; `id -> extractions.id ON DELETE CASCADE`, `owner_id -> users.id ON DELETE CASCADE` |
| `users` | **Protected** Google identity; ห้ามลบเพื่อ reset Runs |
| `sessions` | **Protected** `user_id -> users.id ON DELETE CASCADE` |
| `oauth_states` | **Protected** login state; ไม่มี user FK |
| `oauth_login_requests` | **Protected** FIT Coach login state; ไม่มี user FK |
| `oauth_authorization_codes` | **Protected** `user_id -> users.id ON DELETE CASCADE` |
| `oauth_access_tokens` | **Protected** `user_id -> users.id ON DELETE CASCADE` |
| `oauth_refresh_tokens` | **Protected** `user_id -> users.id ON DELETE CASCADE` |
| `transcript_entries` | **Protected** unrelated admin; ไม่มี Runs FK |
| `legacy_imports` | **Protected** migration ledger source/hash/time/table manifest; ไม่มี FIT/activity FK |

Deleting `users` ทำให้ Runs, sessions และ OAuth records ถูก cascade ไปด้วย จึงไม่ใช่ Runs-only operation. Deleting extraction กระทบเฉพาะ activity projection ใน map เดิม ไม่ต้องใช้ broad `TRUNCATE CASCADE`. Shoes เป็น checked-in static catalog ไม่ใช่ database tables

### Authorization ที่ต้องรักษา

- `auth.rs` derive owner จาก opaque cookie `garmin_fit_session` ที่ hash SHA-256 และ session ใน DB ไม่เชื่อ owner ID จาก client
- FIT Coach bearer ตรวจ configured client, expiry และ `activities:read`; queries ผูก stored owner
- `admin.rs` ต้อง session + normalized verified-email allowlist; mutations ตรวจ exact Origin. Admin clear ต้อง `{ "confirmation": "DELETE_ALL" }`
- Client route gating ไม่แทน API authorization. Source/job/evidence/export ใหม่ต้องใช้ server-derived owner เหมือนเดิม

### Affected paths

Backend: `apps/api/src/fit/{raw,normalize}.rs`, `model.rs`, `db.rs`, `routes/{extractions,activities}.rs`, router/startup/config, additive migrations และ tests. New Runs-only processing/export/analysis modules ควรแยกหน้าที่จาก auth/admin; ไม่ทำ generic framework

Frontend: `_authenticated.upload.tsx`, `_authenticated.history.tsx`, `_authenticated.extractions.$id.tsx`, `history-table.tsx`, `upload-dropzone.tsx`, `json-copy-actions.tsx`, `analysis-summary.tsx`, `activity-charts.tsx`, `lib/{api,upload-validation,activity-chart-data,raw-json}.ts`; Runs tests/E2E และ Share Card module ใหม่

Docs/operations: README/PRODUCT ต้องเปลี่ยนเฉพาะเมื่อ behavior ใหม่ implement; compose/config ต้องให้ persistent FIT จริงตาม storage ADR; backup/reset/runbook, public schema/OpenAPI adapters, fixtures/license manifest

### Protected paths / contracts

- `apps/web/src/routes/shoes.*`, `apps/web/src/data/catalog.ts`, `apps/web/src/domain/*` reviewer/size-comparison/catalog และ public assets
- `apps/api/src/auth.rs`, `routes/auth.rs`, OAuth/token tables/behavior และ `admin.rs`/transcript schema; web auth shell และ admin transcript route/API
- `tools/legacy-data-migrator` และ migration ledger; ไม่มีเหตุผลให้เขียน migrator ใหม่เพื่อ reset Runs
- CLI `garmin-coach analyze`, schemaVersion `1.0.0`, pretty JSON + trailing newline + absolute output path/error behavior
- FIT Coach `/api/v1/activities/latest`, `/api/v1/activities`, `/api/v1/activities/{id}` และ OAuth contract ใน [OpenAPI](../fit-coach-openapi.yaml). แยก compatibility projection จาก Runs export ใหม่ ไม่ใช้ `latest N` แทน explicit selection

## 5. Baseline ผลที่รันจริง

| Command / scenario | ผล | เวลาที่สังเกต |
|---|---|---:|
| `bun install --frozen-lockfile` ใน checkout แยก | exit 0, 294 packages | 5.62 s |
| `legacy-data-migrator prepare-target` ต่อ disposable DB | exit 0 | 25.163 s รวม compile |
| `bun run check` | exit 0 | 24.079 s |
| `bun run test` | exit 0; web 95, CLI 8, Rust 71 ผ่าน, ไม่มี failed | 39.219 s |
| `bun run build` | exit 0 | 2.433 s, warm artifacts |
| `bun run test:e2e` | **exit 1; 3 passed, 1 failed** | 41.452 s รวม startup/build; Playwright 21.6 s |
| `bun run packages/cli/src/cli.ts analyze ...activity.fit --output ...json` | exit 0; JSON parse ได้, schemaVersion 1.0.0, 1 lap | 0.12 s |

E2E failure ที่สังเกต: `apps/web/e2e/extractions.spec.ts:210` คาด detail URL `order=asc` แต่ได้รับ `order=desc`. ไม่ rerun เพื่อกลบผล ไม่แก้ production code และยังไม่พิสูจน์ root cause. Regression นี้เป็น prerequisite ใน milestone แรก ไม่ยอมให้ implementation ใหม่เปลี่ยน expectation เพียงเพื่อให้ผ่าน

E2E ที่ผ่านคือ responsive charts, missing chart data empty states และ shoe-pair no-bridge behavior. API/Vite launch และ health readiness สำเร็จ ก่อน test ที่เหลือล้ม ไม่อ้างว่า Google OAuth จริงหรือ full Runs workflow ผ่าน

Logs/results อยู่ใน directory ชั่วคราว `runs-rebuild-audit-6mctahor` และ sibling `*-baseline-{0..4}.log`, `*-baseline-results.json`; ไม่ commit logs/screenshots/exports ที่อาจมีข้อมูล test session. ตัวเลขด้านบนเป็น baseline บน workstation ไม่ใช่ production performance budgets

## 6. Data capability / unknowns

- Fixture ที่มีสิทธิ์ใช้: `apps/api/tests/fixtures/activity.fit`, 771 bytes, MIT attribution จาก fitparser; SHA-256 `90bf03369eb0bf70275d3c2d3f9496d2b6fea54edd6ed03be271eb02b4e9ab93` ยืนยันใน decoder spike. Synthetic/public corpus และ coverage อยู่ใน decoder report
- **ไม่ได้ audit ประวัติจริง**: จำนวนผู้ใช้/files/sessions, Garmin subtype mix, RR/HR/power coverage, sensor mix, durations, pauses และคุณภาพ segment ของผู้ใช้ยังไม่ทราบ
- ไม่มี annotated segmentation validation set หรือ time-matched physiological reference dataset ของ deployment. ไม่อ้างความแม่นยำจาก fixture หรือซอฟต์แวร์ที่คำนวณซ้ำได้
- ไม่ได้ build/recreate production image, benchmark target hardware, backup/restore real stack หรือ smoke Google OAuth จริง. ไม่มี production credentials และไม่ได้ขอเข้าถึงโดยอัตโนมัติ

## 7. สถานะส่งมอบรอบนี้

- **implemented:** เอกสาร audit/assessment/design/plan และ glossary/proposed ADRs เท่านั้น; ไม่มี rebuild production features
- **tested:** baseline commands/CLI runtime ข้างต้นและ decoder spike ที่รายงานแยก
- **not tested:** AT-01–AT-36 ของระบบใหม่, actual OAuth provider, deployment/container restore, production dataset และ target benchmarks
- **experimental:** numerical LT candidates/eligibility/validation policy ตาม [LT assessment](lt-method-assessment.md)
- **blocked:** release ของ numerical method ที่ยังไม่มี evidence/validated inputs/reference corpus และ decoder gaps ที่ระบุใน assessment; ไม่ block core import/export ทุกส่วน
- **out of scope:** built-in chatbot/LLM, sync/scraping, non-running/non-Garmin, social/public sharing, direct Instagram publishing, AI images และ full training plans

ถัดไป: ตรวจ [architecture](architecture.md), [implementation plan](implementation-plan.md) และ [verification plan](verification-plan.md) เพื่ออนุมัติ milestone แรก ยังไม่ถือว่าอนุมัติ rewrite/reset จากการส่งเอกสารนี้
