# แผนดำเนินงาน Runs rebuild

สถานะ: **ข้อเสนอเพื่อขออนุมัติ (PROPOSED)** เอกสารนี้เป็นผลงานการสำรวจและวางแผนรอบแรก ไม่ใช่การอนุมัติเปลี่ยนระบบ และไม่ใช่รายงานว่า feature ใหม่เสร็จแล้ว ข้อกำหนด D01–D24 เป็นข้อกำหนดผลิตภัณฑ์ที่ล็อกไว้ ส่วน schema, module ใหม่, งบทรัพยากร และวิธีดำเนินการด้านล่างยังต้องผ่าน gate ที่ระบุ

ไม่มี production feature, migration บนฐานข้อมูลเดิม, reset, deployment หรือการอ่าน FIT ส่วนตัวในรอบนี้ การเขียน runbook ไม่ใช่สิทธิ์ให้รัน runbook

## เอกสารประกอบ

ใช้เอกสารทั้งหกฉบับร่วมกัน ไม่ใช้แผนนี้แทนหลักฐานหรือเกณฑ์ทดสอบ:

- [Repository audit](repository-audit.md): โค้ดเดิม ขอบเขตที่ต้องรักษา และ baseline ที่สังเกตจริง
- [Decoder assessment](decoder-assessment.md): corpus เดียวกัน ผลต่าง decoder และ fidelity gate
- [LT method assessment](lt-method-assessment.md): หลักฐานแยก LT1/LT2 และข้อจำกัดที่ FIT ตรวจไม่ได้
- [Architecture](architecture.md): storage, revisions, jobs, temporal model และ interface ที่เสนอ
- [Implementation plan](implementation-plan.md): ลำดับงานและ dependency ในเอกสารนี้
- [Verification plan](verification-plan.md): acceptance cases และหลักฐานที่ต้องเก็บก่อนปล่อย

คำศัพท์อ้างอิง [CONTEXT.md](../../CONTEXT.md) และ ADR ต่อไปนี้ยังมีสถานะ **proposed** ไม่ใช่ accepted:

1. [Decoder fidelity](../adr/0001-runs-decoder-fidelity.md)
2. [Source storage](../adr/0002-runs-source-storage.md)
3. [Schema compatibility](../adr/0003-runs-schema-compatibility.md)
4. [Processing jobs](../adr/0004-runs-processing-jobs.md)
5. [Threshold methods](../adr/0005-runs-threshold-methods.md)
6. [Charts and share card](../adr/0006-runs-charts-share-card.md)

## สถานะและลำดับ milestone

- **Implemented — ระบบเดิม:** Axum/PostgreSQL, ZIP import, raw/normalized JSON, history/detail, chart บางส่วน, authenticated extraction exports, FIT Coach OAuth projection และ CLI มีอยู่แล้ว ไม่มีหลักฐานว่า source FIT, revision queue, selected multi-run export, privacy pipeline หรือ numerical LT ใหม่ถูกนำไปใช้
- **Tested — baseline เท่านั้น:** ใน checkout จาก `git archive` ของ HEAD `d90051ce2cb398678a7c16b6b981724673e8028f` พร้อม PostgreSQL 18 ชั่วคราวและ frozen Bun install, `bun check`, `bun test`, `bun build` exit 0; web 95, CLI 8 และ Rust 71 tests ผ่าน Existing `fit_decode` tests มี default CRC validation และ reject corrupt FIT แม้ wrapper ไม่มีฟังก์ชัน CRC แยก
- **Not tested / ไม่ผ่านครบ:** `bun test:e2e` exit 1, ผ่าน 3 และไม่ผ่าน 1; `apps/web/e2e/extractions.spec.ts:210` คาด `order=asc` แต่ detail URL เป็น `order=desc` ห้ามสรุป root cause จากข้อความนี้ และห้ามกล่าวว่า protected end-to-end workflow ผ่านแล้ว Numerical LT, release budgets และ feature ใหม่ยังไม่ได้ทดสอบ
- **Experimental:** decoder comparison และการประเมินวิธี LT ใน P1 เป็น feasibility/research ไม่ใช่ production readiness ผลที่ยืนยันได้ต้องอ่านจากสอง assessment โดยตรง
- **Blocked:** decoder-backed full schema ยังปล่อยไม่ได้จน fidelity gate ผ่าน; numerical LT ของแต่ละ target ยังปล่อยไม่ได้จน method/reference gate ผ่าน การไม่มี reference dataset ที่เข้าถึงและใช้ได้ตามสิทธิ์ไม่อนุญาตให้ตัด P4 หรือแทนด้วย engine ที่คืน `null` ทุกครั้ง
- **Out of scope:** runtime LLM/API key, non-running/non-Garmin activity, training plan, chart ใน PNG, เปลี่ยน Shoes/auth/admin ที่ไม่เกี่ยวข้อง, บริการ queue/object storage ใหม่โดยไม่มีเหตุจากผลวัด และ production reset/deploy ในรอบนี้ Multi-session/multisport import ไม่อยู่ใน initial supported import policy; ต้อง reject ด้วยเหตุผลชัดเจน ไม่รวมเป็นการวิ่งหนึ่งครั้ง

ลำดับคือ **P0 สำรวจ baseline → P1 feasibility และข้อเสนอ → approval gate → P2 core import/storage/privacy/selected export → P3 charts และ segment analysis → P4 numerical LT และ historical evidence → P5 share/mobile/protected release gates** P3 และ P4 ทำคู่ขนานได้หลัง normalized/revision contract ใน P2 นิ่ง และ P5 บางส่วนทำหลัง P2 ได้ แต่ release ต้องผ่าน dependency ของทุก feature ที่ประกาศปล่อย

**milestone แรกหลังอนุมัติคือ P2 core end-to-end:** นำ FIT/ZIP เข้า เก็บ FIT เดิมใน PostgreSQL BYTEA ประมวลผลแบบ durable และส่งออกเฉพาะกิจกรรมที่เลือกด้วย privacy-safe coach/full JSON ที่ Copy/Download ตรงกัน การปล่อย core ไม่ต้องรอ numerical LT แต่ต้องบอกตรง ๆ ว่า LT engine ยังไม่ released ไม่เรียกสถานะนั้นว่า physiological insufficient data

## P0 — งานสำรวจและ baseline ที่สังเกตแล้ว

### P0.1 สำรวจเส้นทางเดิมและ protected scope

- **paths:** `apps/api/src/{app,main,db,model,auth,admin}.rs`, `apps/api/src/routes/{extractions,activities,auth,oauth}.rs`, `apps/api/src/fit/{raw,normalize}.rs`, `apps/api/migrations/0001_extractions.sql`–`0005_legacy_imports.sql`; ฝั่งเว็บ `apps/web/src/routes/`, `components/`, `lib/`; CLI `packages/cli/src/`; legacy migrator `tools/legacy-data-migrator/`
- **พฤติกรรม/ข้อกำหนด:** ตรวจ flow import ถึง export, owner isolation, FIT Coach และ CLI compatibility (D03–D06, D07–D10, D21, D24) พบ ZIP-only import contract เดิม, ไม่เก็บ original FIT, ไม่มี checkbox selection และไม่มี durable revision queue การ insert extraction เดิมเขียน `activities` projection ใน transaction เดียวกัน
- **หลักฐาน/exit:** audit อ้าง path และแยก source inspection ออกจาก runtime observation แล้ว; ไม่ถือช่องว่างเหล่านี้ว่า implementation เสร็จ
- **blocker:** ไม่มี codegraph index จึงใช้ source audit; ไม่สร้าง index และไม่เข้าถึง `.env` หรือฐานข้อมูลที่ระบุในไฟล์นั้น

### P0.2 รัน baseline ในสภาพแวดล้อมทิ้งได้

- **paths:** `package.json`, `scripts/e2e.ts`, `apps/api/tests/`, `apps/web/e2e/extractions.spec.ts`, `packages/cli/test/`
- **พฤติกรรม/ข้อกำหนด:** เก็บ baseline เพื่อแยก regression จากงานใหม่ และตรวจเส้นทาง protected workflow (D24)
- **หลักฐาน/exit:** ผล check/test/build และ E2E ที่ระบุข้างต้นเป็นผลที่สังเกตจริง; P0 สำรวจเสร็จได้แม้ E2E มี failure แต่ release gate ยังไม่ผ่าน
- **blocker:** ต้องวิเคราะห์และแก้ root cause ของ URL order ในงาน implementation ก่อนอ้าง protected workflow ผ่าน ไม่เปลี่ยน expected assertion เพื่อซ่อน failure และไม่รันซ้ำเพียงเพื่อยืนยันรายงานเดิม

## P1 — feasibility และแผนที่เสนอ ยังไม่ใช่ production implementation

### P1.1 Decoder fidelity และ schema gate

- **paths:** `apps/api/src/fit/raw.rs`, `apps/api/tests/fit_decode.rs`, `packages/cli/src/fit.ts`, `decoder-assessment.md`, proposed ADR 0001
- **พฤติกรรม/ข้อกำหนด:** เปรียบเทียบ pinned Rust `fitparser`, Python `fitdecode`, official `garmin-fit-sdk` บน synthetic/public licensed corpus เดียวกัน ตรวจ compressed timestamp, message/field numeric identity, unknown/developer fields, arrays, RR, invalid values, CRC และทรัพยากร (D04–D06, D09)
- **หลักฐาน/exit:** assessment ระบุรุ่นจริง fixture/license, command/result, expected structure และเหตุผลแก้ความขัดแย้งตาม spec ไม่ใช้เสียงข้างมากของ decoder; final synthetic corpus มี 13 cases รวม 1,743 bytes การเปิด `KeepCompositeFields` รักษา composite fields ได้ แต่ยังพบ packed HR expansion bug ที่ต้องผ่าน patch/upstream gate ผลจิ๋วนี้ไม่ใช่ capacity benchmark
- **blocker:** decoder ยังไม่ถูกเลือกจน ADR ได้รับอนุมัติ ต้องแก้ metadata retention และ packed HR พร้อม negative cases ก่อนยืนยัน full archive; official Garmin SDK มี licensing gate สำหรับการใช้/แจกและ externally published diagnostics จึงไม่เผยแพร่ SDK benchmark performance หรือรันเพิ่มโดยไม่มี legal approval งาน P2 ที่พึ่ง full decoding ยังติด fidelity gate แม้ storage/import groundwork ทำต่อได้

### P1.2 ประเมิน LT1 และ LT2 แยกกัน

- **paths:** `lt-method-assessment.md`, proposed ADR 0005 และ numerical fixtures ที่เสนอใน P4
- **พฤติกรรม/ข้อกำหนด:** แยก target/reference definition, population, sport/protocol, inputs, FIT-unverifiable assumptions, reproducibility และ agreement/error ของแต่ละวิธี (D11–D18) กำหนด release choice แยก target ไม่ใช้ correlation เป็นหลักฐาน individual accuracy
- **หลักฐาน/exit:** assessment ระบุ experimental numerical candidates `running-dfa-a1-075` สำหรับ LT1-side VT1 proxy และ `running-dfa-a1-050` สำหรับ LT2-side VT2 proxy พร้อมข้อจำกัด ไม่เปลี่ยนชื่อ ventilatory proxy เป็น lactate threshold ที่ validated; OLS crossing arithmetic smoke ได้ 150.0/170.0 bpm แต่ไม่ได้รัน DFA/FIT numerical engine ระบุว่า decoupling เป็น supporting evidence ไม่ใช่นิยาม LT1 ที่ drift 5% และไม่สมมติว่า fastest 30 minutes คือ maximal test
- **blocker:** method ที่ไม่มีหลักฐาน/ข้อมูล reference เพียงพอคงสถานะ blocked งานประเมินเสร็จไม่เท่ากับ numerical estimator พร้อมใช้

### P1.3 เสนอ interface, migration และ release criteria

- **paths:** รายงานหกฉบับ, `CONTEXT.md`, proposed ADR 0001–0006, existing `docs/fit-coach-openapi.yaml`, `README.md`
- **พฤติกรรม/ข้อกำหนด:** จัด mapping D01–D24, source/revisions/jobs, selected exports/privacy, cutoff และ Runs-only recovery ให้สอดคล้องกัน
- **หลักฐาน/exit:** ทุก feature มี owner module, dependency, verification และ blocker ใน P2–P5; ทุกตัวเลข resource budget มีป้าย proposed/unmeasured
- **blocker:** ไม่มีสิทธิ์ทำ schema cutover, reset หรือ deployment จากการมีเอกสารเพียงอย่างเดียว

## Approval gate ก่อน P2

ผู้ใช้ต้องอนุมัติ implementation scope และ ADR ที่กระทบ core โดยชัดแจ้ง ได้แก่ PostgreSQL BYTEA, decoder fidelity ทางที่เลือก, new Runs schema/API, version/revision jobs, privacy และ compatibility transition การอนุมัติทำ implementation ไม่รวมการล้างข้อมูลหรือ deployment

ต้องตกลงก่อนเขียน feature: retention ของ source/revisions, initial import limits, multisession rejection, sourceUnavailable handling, v1 extraction export transition, backup/restore acceptance และลำดับ cutover สำหรับ clients เดิม Numerical LT มี gate แยก LT1/LT2 ใน P4; การอนุมัติ P2 ไม่อนุญาตสร้าง threshold formula เองเพื่อเติมช่องว่างงานวิจัย

paths ใหม่ด้านล่างเป็น **proposed paths** ให้เพิ่มเท่าที่ behavior ต้องใช้ ไม่บังคับสร้าง abstraction หนึ่งชั้นต่อหนึ่งตาราง และไม่แยกบริการใหม่จาก Axum binary/PostgreSQL เดิม

การอ้าง path แบบย่อในงานด้านล่างใช้ `runs/` แทน `apps/api/src/runs/` และ `components/`, `lib/`, `routes/` ของ frontend แทนโฟลเดอร์ใต้ `apps/web/src/` ตามลำดับ ชื่อ module ใหม่ทั้งหมดในพื้นที่นี้เป็นข้อเสนอ ไม่ใช่ไฟล์ที่มีอยู่แล้ว ส่วนไฟล์ที่ reuse ระบุ existing/ใช้ไว้ชัดเจน

## P2 — Core import, archive, jobs, privacy และ selected export

### P2.1 เพิ่ม source/revision schema และ stable identity

- **paths:** ใช้ `apps/api/src/{db,model}.rs`; เพิ่ม proposed `apps/api/src/runs/{mod,store,model}.rs` และ migration ใหม่ใน `apps/api/migrations/` โดยไม่แก้ migration ที่ใช้แล้ว
- **พฤติกรรม/ข้อกำหนด:** เก็บ immutable FIT bytes ใน `runs_sources` แบบ BYTEA พร้อม owner/hash/size/provenance/integrity; `UNIQUE(owner_id, sha256)` สำหรับ atomic dedup (D03–D06) ไม่เก็บ ZIP และไม่วาง FIT ใน public root สร้าง stable activity ID และ source/session identity; logical tables สำหรับ import items, activities/summary, staged revisions, jobs และ estimates รวมกันได้เมื่อมี invariants เดียวกัน แยก decoded/normalized/derived documents และ indexed summary projection
- **verification/exit:** import แล้วเทียบ hash/length/bytes กับต้นฉบับ, concurrent same-owner duplicate ได้ source/activity เดียว, ต่าง owner ไม่เปิดเผยการมีอยู่, revision เปลี่ยนไม่เปลี่ยน activity ID; list query ไม่อ่าน decoded payload; constraint ป้องกัน cross-owner revision/reference
- **blocker/dependency:** approval ของ storage/retention และ decoder stage contract; วัด PostgreSQL WAL/backup size ก่อนยืนยัน BYTEA เหมาะกับ deployment จริง

### P2.2 FIT/ZIP import ที่มีผลราย member และ resource limits

- **paths:** ใช้ `apps/api/src/routes/extractions.rs`, `apps/api/src/config.rs`, `apps/web/src/routes/_authenticated.upload.tsx`, `components/upload-dropzone.tsx`, `lib/upload-validation.ts`; เพิ่ม proposed `apps/api/src/routes/runs.rs`, `apps/api/src/runs/import.rs`
- **พฤติกรรม/ข้อกำหนด:** รับ FIT โดยตรงและ ZIP แบบ case-insensitive ตรวจ actual content/header/CRC ไม่เชื่อ suffix อย่างเดียว; เก็บ batch/member result ที่ success, duplicate, unsupported หรือ failed โดยไม่ rollback member ที่สำเร็จ (D02–D06) ตรวจ Garmin manufacturer และ running sport/subtype จาก profile ที่ยืนยันแล้ว ไม่ reject เพราะ accessory sensor ยี่ห้ออื่น เริ่มด้วยหนึ่ง running session ที่ผูก lap/sample ได้แน่ชัด; multisession/multisport reject ด้วย unsupported reason
- **ความปลอดภัย:** จำกัด request/member/expanded bytes, member count, compression ratio, walltime, memory และ concurrency ตาม config ที่อนุมัติ; reject traversal, absolute path, symlink, nested archive และ ZIP bomb ไม่เปิด decode ก่อนตรวจ bounds; ไม่ใช้ upload filename เป็น public identifier
- **verification/exit:** FIT/ZIP valid, extension case, corrupt FIT/ZIP, running subtypes, non-Garmin/non-running, multisession, mixed partial batch และ archive attacks ผ่าน fixture tests และจริงจาก upload UI; member success ไม่ถูกกลืนโดย failure; retry batch ไม่เพิ่ม observation ซ้ำ
- **blocker/dependency:** P2.1 และ P1 decoder/import-profile gate; limits เริ่มต้น source 20 MiB เป็นข้อเสนอ ไม่ใช่ผลวัดว่าเครื่องรองรับ

### P2.3 Full decoded archive และ normalized contract ที่ตรวจย้อนกลับได้

- **paths:** ใช้ `apps/api/src/fit/{raw,normalize}.rs`, `apps/api/src/model.rs`; เพิ่ม proposed `apps/api/src/runs/model.rs`, `apps/api/tests/runs_normalization.rs` และ licensed/synthetic fixtures ใน `apps/api/tests/fixtures/`
- **พฤติกรรม/ข้อกำหนด:** เก็บทุก field ที่ decoder รองรับ ไม่ whitelist ตาม UI; เก็บ record order, numeric message/field/developer identity, field type/units, absent/invalid/zero distinction, raw/expanded values เท่าที่ decoder ให้ได้และ warnings ของช่องว่าง (D04–D06, D09) decimal string สำหรับ integer ที่ JSON/JS รับอย่างเที่ยงตรงไม่ได้; ห้าม NaN/Infinity และห้าม archive smoothing/downsampling/interpolation
- **normalization:** แยก activity/session/recorded lap/sample/timer/sensor/zone; UTC พร้อม offset เฉพาะเมื่อมี source; elapsed/timer/moving time ต่างกัน; canonical units และ provenance ของ speed/altitude/native/developer precedence; zero speed ไม่มี infinite pace; time-weighted means สำหรับ irregular samples; cadence units มี fixture ยืนยัน; recorded/calculated แยก และไม่ย้อนหลัง current zones เป็น historical zones
- **verification/exit:** corpus gate รักษา numeric identity/order/unknowns, dictionary ระบุความหมายและ units, pause/gap/zero/invalid/unsafe integer fixtures ให้ผลที่ตรวจคำนวณเองได้; detail API อ่าน normalized streams โดยไม่โหลด full archive
- **blocker/dependency:** P1 decoder fidelity ต้องผ่านก่อน full export release; รายการ decoder unsupported ต้องบอกตรง ๆ ไม่ใช้ชื่อ full เพื่ออ้างเก็บข้อมูลที่ decoder ไม่เคยส่งออก

### P2.4 Durable PostgreSQL queue และ coherent publish

- **paths:** ใช้ `apps/api/src/{main,app,config}.rs`; เพิ่ม proposed `apps/api/src/runs/{jobs,store}.rs`, decoder child mode ใน `main.rs`, `apps/api/tests/runs_jobs.rs`
- **พฤติกรรม/ข้อกำหนด:** ใช้ PostgreSQL queue ใน API binary เดิม ไม่เพิ่ม Redis/network worker service (D04–D06, D18, D24) claim ด้วย `FOR UPDATE SKIP LOCKED`, durable queued/running/succeeded/failed states, bounded attempts/backoff, leases/heartbeat, cancellation และ restart recovery; เสนอ 3 attempts และหนึ่ง decode child ต่อ API instance พร้อม PostgreSQL global slot/lease เพื่อไม่เพิ่มงานเกิน limit เมื่อมีหลาย replicas
- **killable decode:** แยก decode untrusted FIT เป็น subprocess mode ของ **Rust executable เดียวกัน** ใช้ stdin/source reference ที่ owner-scoped และ bounded output; parent บังคับ walltime และ platform-supported memory limit, kill และ reap เมื่อ timeout/cancel/crash ไม่หวังว่า async timeout จะหยุด CPU/parser ได้เอง หาก decoder gate ต้องใช้ทางอื่นให้แก้ ADR และขออนุมัติ ไม่สร้างบริการข้างเคียงเงียบ ๆ
- **desired versions:** startup scanner และ bounded ongoing sweep เทียบ desired decoder/profile/options, normalizer, segment/method/config version กับ published input manifests; job key ครอบ stage + source/input hashes + desired versions/config + cutoff/evidence digest analysis-only change ไม่ decode ใหม่ normalizer change reuse decoded เมื่อ contract เข้ากันได้ และ decoder change ทำ downstream ตาม dependency
- **CAS publish:** stage immutable revisions ก่อน; publish current manifest/summary/projection ใน transaction เดียวหลังตรวจ current generation, desired version, lease fencing token, owner/source ยังอยู่ และ dependency manifests ยังตรง CAS แพ้ต้อง discard/requeue ไม่ทับงานใหม่ ห้ามเห็น decoded ใหม่กับ normalized เก่าหรือ estimate ที่ใช้ inputs คนละชุด
- **recovery/failure:** expired lease reclaim ได้แต่ stale worker publish ไม่ได้; bounded retry หมดแล้วบอก error แบบ privacy-safe เก็บ last-good revision และแสดง stale/failed attempt แยกจากผลเดิม ห้าม last-good ที่อ้าง deleted evidence ฟื้นกลับมา; successful stage ใช้ซ้ำได้ด้วย idempotency key ไม่ทำ duplicate publish
- **verification/exit:** process kill ระหว่าง decode/stage/publish, lease expiry, two replicas, retry exhaustion, desired-version เปลี่ยนกลางงาน, CAS conflict และ delete กลางงานยังให้ current manifest coherent; restart เดินงานต่อเอง stable ID ไม่เปลี่ยน
- **blocker/dependency:** P2.1/P2.3; ต้อง prove memory enforcement บน OS/container ที่ deploy จริง ไม่อ้าง limit จากเครื่องทดลองว่า production enforce แล้ว

### P2.5 Shared export projection และ fail-closed privacy

- **paths:** เพิ่ม proposed `apps/api/src/runs/{export,privacy}.rs`, routes ใน `apps/api/src/routes/runs.rs`, `apps/api/tests/runs_exports.rs`; ใช้ frontend `lib/{api,api-types,raw-json}.ts`, `components/json-copy-actions.tsx`, `routes/_authenticated.history.tsx`, `components/history-table.tsx`, `routes/_authenticated.extractions.$id.tsx`
- **พฤติกรรม/ข้อกำหนด:** new API proposed `/api/v2/runs` แยก schema domain จาก CLI 1.0.0; POST ต้องส่ง explicit owner-scoped `activityIds`, `coach|full`, privacy options ค่าเริ่มต้น false (D01, D07–D10, D21) validate ทุก ID และ pin coherent current manifests ใน repeatable-read transaction; deleted/foreign/processing/unsupported selected item ทำ complete-or-error ไม่ส่ง subset ไม่ดึง latest หรือประวัติทั้งหมดแทน
- **selection:** checkbox หนึ่ง/หลายรายการ, selected count, clear และ select-current-page ชัดเจน; เก็บ selected IDs เมื่อข้ามหน้าโดยไม่เลือกหน้าใหม่อัตโนมัติ; ไม่ถือ filtering/order เป็น selection ไม่ลบ selected item เงียบ ๆ เมื่อข้อมูลเปลี่ยน ให้ผู้ใช้แก้ selection หลัง server error
- **JSON:** two-space pretty JSON, meaningful names, ไม่ใช้ columns/rows encoding Full ส่งเฉพาะ selected current decoded+normalized+derived ไม่มี original FIT bytes, DB/account/revision dump และไม่มี precision/row truncation Coach ลดข้อมูลอย่างเปิดเผยพร้อม method/count/resolution/rounding โดยรักษา recorded laps, repeats, surges และ gap boundaries; P3 เพิ่ม detected segment projection ภายหลังจาก analysis จริง
- **privacy:** profile/developer registry แยก classification สำหรับ location/device IDs ครอบ native, normalized, nested unknown/developer fields, semicircles, route/bounds และ serials; unclassified fields omit แบบ fail-closed แม้ opt-in โดย manifest มี category/path ที่ไม่บรรจุข้อมูลลับ/count/reason ไม่ใส่ค่าเดิมใน warning; known opt-in ไม่เปิด unclassified อัตโนมัติ ค่าเริ่มต้นไม่มี account/email/private paths/upload filename Archive ใน DB ไม่เปลี่ยน
- **LT projection:** ใช้ historical result ของ selected activity เท่านั้น; evidence เปิดเผยเฉพาะ parameters/necessary aggregate/count/date range/limits ไม่ส่ง unselected activity IDs, raw records, laps, samples หรือ nested history; trend ไม่อยู่ใน data exports
- **Copy/Download:** สร้าง snapshot ที่มี selected revision manifest, privacy/mode/schema และ fixed generatedAt ใช้ JSON text เดียวสำหรับสองช่องทาง; owner-scoped token TTL เสนอ 15 นาที ไม่ใช้ public cache แสดง estimated bytes ก่อน copy เมื่อ clipboard ล้มเหลวต้องแจ้งจริงและให้ download payload/selection เดิม ไม่มีการตัดข้อความตาม clipboard limit
- **verification/exit:** one/many/page-crossing selection, zero selection, foreign/deleted/processing ID, concurrent revision update, full large sample completeness, default/opt-in/unknown nested privacy และ copy failure; Copy/Download snapshot มี logical content และ bytes ตรงกัน; inspect export ว่าไม่มี unselected history หรือ hidden omission values
- **blocker/dependency:** P2.1–P2.4 และ sourceUnavailable rule ใน P2.6; downstream analyses ที่ยังไม่ released มี machine-readable availability ไม่มี fabricated result

### P2.6 Compatibility และ migration dual-read ที่มีวันสิ้นสุดตาม gate

- **paths:** ใช้ `apps/api/src/routes/{extractions,activities}.rs`, `apps/api/src/{db,model}.rs`, `packages/cli/src/{cli,fit,normalize,schema,analyze}.ts`, `tools/legacy-data-migrator/src/`, `docs/fit-coach-openapi.yaml`, `README.md`; เพิ่ม Runs migration/projection เฉพาะส่วนที่จำเป็น
- **พฤติกรรม/ข้อกำหนด:** รักษา CLI `schemaVersion: 1.0.0`, command, field contract, two-space JSON + newline และ absolute output path ไม่ย้าย CLI เข้าสู่ schema v2 โดยเงียบ ๆ (D04, D09, D24) รักษา `/api/v1/activities` OAuth contract ของ FIT Coach ผ่าน explicit projection ของ coherent current revision; ไม่ให้ latest endpoint กลายเป็น selected export และไม่เปลี่ยน device-recorded ค่าเดิมเป็น estimated LT การให้ new-source activity มี FIT Coach projection ต้องใช้ targeted FK migration ที่รักษา owner/stable IDs ไม่สร้าง extraction ที่อ้างว่าสำเร็จปลอม ๆ เพื่อให้ `activities.id → extractions.id` ผ่าน
- **legacy:** extraction เดิมไม่มี FIT จึง mark `sourceUnavailable` และ read-only ไม่สร้าง source bytes จาก raw JSON, ไม่ pretend reprocess และไม่จับคู่กิจกรรมด้วยวันที่ New legacy-safe export ต้องผ่าน v2 classifier/validator เดียวกัน และระบุ source completeness ที่แท้จริง หาก coach/full contract ทำไม่ได้ให้ explicit `LEGACY_EXPORT_UNSUPPORTED` ไม่มี fallback ไป old raw endpoint หรือแอบส่ง subset UI legacy inspection บอกข้อจำกัดและไม่เรียก stored legacy raw ว่า complete new archive การลบตาม owner ยังทำได้
- **dual-read cutover:** (1) additive schema และ catalog rows แบบ provenance/owner mapping; (2) new writes เข้าสู่ source/revision path เท่านั้น, dual-read new/legacy ด้วย explicit sourceKind ไม่ทำ two normalizers; (3) web เปลี่ยนใช้ v2 และ FIT Coach เปลี่ยน projection แบบ atomic publish พร้อม targeted FK change; (4) ตรวจ counts/owner/IDs/checksums และ clients เดิมครบ; (5) ปิด old extraction write/export surface หลังอนุมัติ retirement และลบ compatibility code ที่หมดหน้าที่ ไม่ปล่อย indefinite shims การย้ายหรือเลิกใช้ stable legacy URLs ต้องประกาศ contract ไม่แอบชี้ ID ไปคนละกิจกรรม
- **old export transition ที่ต้องอนุมัติ:** `/api/v1/extractions` normalized/raw endpoints ใช้ได้เฉพาะ clients เดิมของ existing sourceUnavailable rows ในช่วง migration ที่ประกาศและมีขอบเขตเวลาชัดเจนตาม inventory/retirement gate รักษา legacy schema/semantics และ owner authentication; ต้องเตือนว่า old raw behavior **ไม่ได้รับ new privacy guarantee** ไม่เปิดใช้กับ new sources หรือ new UI และไม่ rewrite/redact payload เดิมเงียบ ๆ พร้อมเรียกว่า compatible ประกาศ deprecation/client inventory/approved retirement boundary ใน release notes แล้วปิด routes ด้วย explicit retirement response เมื่อ gate ผ่าน หาก clients ย้ายไม่ทันต้องขออนุมัติ transition ใหม่ ไม่ต่อเวลาโดยเงียบ ๆ และไม่ประกาศ P2.6 exit ผ่าน New UI รวม legacy-safe exports ใช้ v2 privacy pipeline เท่านั้น
- **verification/exit:** CLI contract และ OAuth scopes/envelopes/list/latest/detail เดิมผ่าน consumer-visible regression; targeted FK migration รักษา owner/stable IDs และไม่เพิ่ม fake succeeded extraction; new/legacy rows มี sourceUnavailable/ID mapping ถูกต้อง; v2 legacy classifier รับเฉพาะที่ผ่าน contract และ unsupported ให้ `LEGACY_EXPORT_UNSUPPORTED`; old endpoint ไม่รับ new sources/new UI; client migration inventory และ retirement approval มีหลักฐานก่อนเอา old shim ออก
- **blocker/dependency:** approval ของ compatibility/deprecation policy และข้อมูล clients ที่ใช้งานจริง; ไม่ค้น secrets หรือเรียก production เพื่อเติม inventory โดยไม่ได้รับอนุญาต

### P2.7 Delete, export races และ source backup

- **paths:** `apps/api/src/runs/{store,jobs,export}.rs`, existing `apps/api/src/db.rs`, new Runs migrations; recovery design ใน `architecture.md`/`verification-plan.md`
- **พฤติกรรม/ข้อกำหนด:** owner-scoped one/all Runs delete ทำ transaction ลบ source/decoded/normalized/derived/revisions/export manifests และ cancel jobs/เพิ่ม generation/tombstone เพื่อไม่ให้ worker resurrect (D04–D06, D18, D21, D24) dependent estimates รวม materialized evidence/caches ต้อง invalidate/erase เมื่อ evidence ถูกลบ ไม่แสดง old valid trace ที่ยังมีข้อมูลของกิจกรรมลบแล้ว
- **export race:** ก่อนสร้าง snapshotและก่อน retrieval ตรวจ owner/existence/generation อีกครั้ง; snapshot ที่เลือกกิจกรรมลบแล้ว revoke ไม่เสิร์ฟ pinned revision เก่า สำหรับการส่ง payload ใช้ per-owner DB coordination shared export lease/exclusive delete fence และตรวจ revoke ก่อนส่ง response/แต่ละ bounded chunk; deletion intent ยกเลิก stream และรอส่งงานที่กำลัง active จบ/abort ก่อนยืนยัน delete สำเร็จ bytes ที่ส่งถึงเครื่องผู้ใช้ก่อน delete ไม่สามารถเรียกคืนได้ และต้องบอกข้อจำกัดนี้ ห้าม stream ต่อหรือเริ่ม retrieval ใหม่หลัง delete confirmation ไม่มี public/CDN/service-worker cache ของ export และไม่มี durable plaintext dump ที่หลงเหลือ
- **backup:** BYTEA อยู่ใน PostgreSQL persistent storage; consistent DB backup ครอบ original+revisions+manifest ไม่ต้องดูแล FIT volume แยก ตรวจ source hash, foreign references/orphans และ delete retention policy ใน restore rehearsal
- **verification/exit:** delete ระหว่าง decoder, normalization, estimate, snapshot generation, clipboard retrieval และ stream; stale job/tokens ใช้ไม่ได้หลัง confirmation dependent LT invalidation ครบ; restore จาก backup ที่ทราบเวลาให้ original bytes/manifest ตรงกันใน disposable environment
- **blocker/dependency:** P2.4/P2.5 และต้องยืนยัน streaming coordination ไม่ทำ long-running transaction หรือ memory เกิน budget; backup encryption/access/retention และ deployment volume ยังต้องมีข้อมูลที่ได้รับอนุญาตก่อน production acceptance

**P2 exit:** demo จริงจาก FIT/ZIP ถึง owner-scoped detail, deletion และ selected coach/full Copy/Download ผ่าน privacy และ completeness, crash recovery และ compatibility gates ตาม verification plan ไม่ใช่เพียง compilation/tests ผ่าน เก็บ release evidence บน hardware ที่ระบุ; numerical LT absent ยังเป็น not released ไม่ใช่ค่าคำนวณเทียม

## P3 — Charts และ FIT-only segment analysis

### P3.1 Timeline ที่ไม่สร้างข้อมูลแทนสิ่งที่ขาด

- **paths:** existing `apps/web/src/components/activity-charts.tsx`, `lib/activity-chart-data.ts`, `routes/_authenticated.extractions.$id.tsx`, `apps/api/src/runs/model.rs`; reuse installed `@tanstack/charts` 0.16.0 ก่อนพิจารณาเปลี่ยน library
- **พฤติกรรม/ข้อกำหนด:** Pace/HR/Power/Laps เป็นข้อมูลหลัก (D19–D20) linked time axes, recorded lap markers, pause/gap breaks, point inspector, zoom/reset และ explanation ของ source/units; missing HR/power ไม่กลายเป็นศูนย์ pace ที่ speed zero ไม่ infinity/downstream zero substitution ทำ display-only downsampling ที่เปิดเผย method/original/display count โดย estimator/export/archive ไม่ใช้ชุดลดข้อมูล
- **verification/exit:** real rendered browser surface บน desktop/mobile, irregular/gap/zero/pause fixtures, inspector value เทียบ normalized source และ lap boundaries; spike ต้อง prove library gap/zoom/inspect ก่อน replacement; historical detail ไม่ดึง latest analysis
- **blocker/dependency:** P2 normalized streams และ revision pin; interaction/visual performance ยัง unmeasured จนรัน surface จริง

### P3.2 Recorded laps แยกจาก detected workload segments

- **paths:** proposed `apps/api/src/runs/analysis.rs`, staged analysis revisions ใน `runs/store.rs`, `apps/api/tests/runs_segments.rs`; existing `components/analysis-summary.tsx`, charts/detail และ coach export ใน `runs/export.rs`
- **พฤติกรรม/ข้อกำหนด:** ตรวจ steady/interval/repeat/surge/recovery patterns จาก FIT โดยไม่ต้อง workout label และไม่กล่าวอ้าง intent หรือ maximal effort (D13, D19–D20) recorded Garmin laps ไม่ถูกแทนด้วย detected segments; segment มี bounds, inputs, policy version, eligible/rejected reason และ coverage แยกตาม target method; strides ไม่ทำให้ทั้ง run ถูก reject
- **analysis policy:** กำหนด candidate selection policy ก่อนดูผล ป้องกันเลือก window เพื่อให้ threshold สวย; พิจารณา HR lag/carryover, pauses, gaps, irregular sampling และ independence; ไม่ใช้ HR-stability filter ที่ลบ physiological drift; power เป็น optional measured signal ไม่ใช่ universal ground truth; quality ของข้อมูลแยกจาก physiological accuracy
- **verification/exit:** synthetic steady/strides/interval/gaps/lag/drift fixtures แสดง segment ที่ยังใช้ได้และ reject ที่มีเหตุผล; overlapping windows ในกิจกรรมเดียวไม่เพิ่ม independent activity count; graph overlays กับ coach JSON bounds ตรงกัน และผู้ใช้ inspect accepted/rejected reasons ได้
- **blocker/dependency:** P2.3/P2.4 และ target eligibility จาก P4 method specification ไม่ใส่ physiological cutoff โดยไม่มีหลักฐาน

### P3.3 Duplicate evidence grouping และ temporal invalidation seam

- **paths:** proposed `runs/{analysis,store,jobs}.rs`, `runs_activities` observation-group metadata; detail/list components
- **พฤติกรรม/ข้อกำหนด:** FIT ต่าง bytes คงกิจกรรมแยก พร้อม deterministic possible-duplicate evidence จาก verified device activity identity ถ้ามี, exact timestamps/duration/distance/laps/streams ไม่ merge ด้วยวันที่อย่างเดียว (D06, D12, D15, D18) observation group ให้ P4 เลือก representative ตาม policy ที่ประกาศและไม่ double-weight; ไม่เผย owner อื่นหรือ private identifiers ใน UI/export
- **verification/exit:** same-day distinct runs ไม่ถูกรวม, re-exported same run ไม่เพิ่ม evidence count, tie-break ทำซ้ำได้; import กิจกรรมเก่าหรือเปลี่ยน duplicate grouping enqueue เฉพาะ affected downstream cutoffs รวม cutoff ที่ยังไม่เคยอ้างกิจกรรมใหม่นี้ด้วย
- **blocker/dependency:** P2 identity และ P3.2; identity field ที่ decoder ไม่ยืนยันใช้เป็นคำตัดสินเด็ดขาดไม่ได้

## P4 — Numerical LT แยก target, history, trace และ suggestions

P4 อยู่ใน scope แม้ว่างานวิจัยยัง blocked การปล่อย LT1 ไม่ได้แปลว่า LT2 ผ่าน หรือกลับกัน Numeric release ต้องมี calculation จริงและ positive/negative fixtures ของ target นั้น Engine ที่มีแต่ status/null และ trace mock ไม่ใช่ completion

### P4.1 Freeze method specification และ reference gate แยก LT1/LT2

- **paths:** `lt-method-assessment.md`, proposed ADR 0005; proposed `apps/api/src/runs/thresholds.rs` method specification และ licensed numerical fixtures ใน `apps/api/tests/fixtures/`
- **พฤติกรรม/ข้อกำหนด:** freeze experimental candidates `running-dfa-a1-075`/`running-dfa-a1-050` แยก target ตาม assessment ก่อนดูผล holdout (D11–D15) ระบุ exact proxy/reference, formula/units, population/protocol applicability และ FIT inputs; initial policy เสนอ newest eligible independent running activity ที่ end ไม่เกิน cutoff ใน lookback ไม่เกิน 7 วัน โดยใช้ sensor/recording path/protocol class ที่เทียบกันได้ ไม่ pool HR corrections/thresholds ข้าม sensor หรือ protocol และไม่ให้ overlapping windows เพิ่ม independent count ค่า 7 วันเป็น operational proposal ไม่ใช่ physiology guarantee ต้องมี recency-sensitivity validation ก่อน validated release
- **numeric contract/ข้อห้าม:** pipeline ตาม assessment คือ real exercise RR/time alignment, annotated artifact/corrected copy, smoothness-priors detrending λ=500, centered integrated RR, local linear detrend, pooled RMS F(n) ที่ n=4…16, OLS log F(n) ต่อ log n, 120-s windows ทุก 5 s, window HR และ frozen decline selector ก่อน OLS alpha–HR crossing ต้อง pin window closure, scale overlap/end-tail, artifact policy และ comparator version ไม่มี RR จาก sampled HR/resting HRV, fixed LT ratio, HRmax/zone mapping, drift 5% standalone หรือ fastest-30-min proof
- **verification/exit:** pin `nolds 0.6.3` research comparator config และ licensed clean/artifact paired reference ที่ assessment ระบุ ไม่ใช้ comparator เป็น runtime service; positive/negative numerical tests ต้องแยก software agreement จาก physiological validation freeze deterministic selector/required facts/optional context ก่อนผล test และ report individual agreement/error ไม่ใช่ correlation อย่างเดียว
- **blocker/dependency:** licensed paired exercise-RR reference corpus, exact preprocessing/selector reproduction, sensor-to-FIT time alignment และ empirical validation plan ยังเป็น prerequisites ตาม assessment หาก chosen method มี REQUIRED fact ที่ FIT พิสูจน์ไม่ได้ให้ `requiredContextUnprovable` และ abstain; ความไม่ทราบ optional recovery/caffeine/meal/sleep เป็น `contextUnverified` และคง low confidence ไม่เรียก verified protocol หรือให้ label/confirmation ปลด gate

### P4.2 Implement numerical LT1 ที่ผ่าน gate

- **paths:** proposed `apps/api/src/runs/thresholds.rs`, new `apps/api/tests/runs_lt1.rs`, stage manifests ใน `runs/{jobs,store,model}.rs`
- **พฤติกรรม/ข้อกำหนด:** implement approved research version ของ `running-dfa-a1-075` ใน `thresholds.rs` หลัง P4.1 gate ใช้ actual RR/DFA pipeline และ original-resolution normalized inputs/accepted progression segments; fit alpha = slope×HR + intercept และคำนวณ HR crossing = (0.75−intercept)/slope เมื่อ negative slope, HR variance, observed bracket และ quality gates ผ่าน ไม่ extrapolate (D11–D15) ผลคือ LT1-side VT1 proxy แบบ experimental จน exact target validation ผ่าน ไม่แทน device-recorded threshold
- **verification/exit:** analytical OLS fixture HR `[130,140,150,160,170]`, alpha `[1,.875,.75,.625,.5]` ให้ 150 bpm ที่ 0.75; actual synthetic RR/FIT positive path ต้องได้ experimental numerical result จาก full engine พร้อม F(4)…F(16)/alpha comparator agreement ไม่ใช้ OLS fixture แทน end-to-end proof Negative HR-only, constant RR, short/gapped window, invalid provenance, no bracket, zero/positive slope, ambiguous decline และ artifact boundary ต้อง abstain ตามจริง Carryover ไม่ลบ early eligible segment
- **blocker/dependency:** P4.1 LT1 research/reproduction gate, P3 segments และ P2 revisions; ไม่แทน missing reference ด้วย drift calculator และไม่ใช้ null-only implementation ยืนยัน completion การ release แบบ experimental และ validated มีคนละ gate ตาม assessment

### P4.3 Implement numerical LT2 ที่ผ่าน gate โดยไม่พึ่ง LT1 ratio

- **paths:** proposed `apps/api/src/runs/thresholds.rs`, new `apps/api/tests/runs_lt2.rs`, stage manifests ใน `runs/{jobs,store,model}.rs`
- **พฤติกรรม/ข้อกำหนด:** implement approved research version ของ `running-dfa-a1-050` จาก actual RR/DFA pipeline โดยแยก LT2 target/version/result จาก LT1 (D11–D15) ใช้ HR crossing = (0.50−intercept)/slope เฉพาะ observed bracket และ approved eligibility เป็น LT2-side VT2 proxy จน exact validation ผ่าน ไม่คูณ LT1 ratio ไม่บังคับ workout label และไม่เดา maximal intent ถ้า protocol fact เป็น REQUIRED แต่ FIT พิสูจน์ไม่ได้ให้ abstain
- **verification/exit:** analytical OLS fixture เดียวกับ LT1 ให้ 170 bpm ที่ 0.50 พร้อม actual synthetic exercise-RR FIT positive path ผ่าน full pipeline; negative/non-bracket/non-declining/unverifiable-required-fact fixtures ให้เหตุผลถูก target; มี LT1 bracket อย่างเดียวไม่แกล้งมี LT2 และ LT2 insufficient ไม่ลบ LT1 valid Conflicting ordering แสดง conflict โดยรักษา raw candidates ไม่ clamp
- **blocker/dependency:** P4.1 LT2 research/reproduction gate, P3 segments และ P2 revisions; LT1 gate ไม่ใช้แทน LT2 gate และ method ที่ต้อง verified maximal TT ยังคง blocked/abstain ไม่ปลดด้วย user label

### P4.4 Historical owner evidence และ chronological invalidation

- **paths:** proposed `runs/{thresholds,store,jobs}.rs`, `runs_threshold_estimates`/input references, `runs/export.rs`; existing history/detail routes และ API list/summary projection
- **พฤติกรรม/ข้อกำหนด:** เลือก evidence อัตโนมัติจาก owner history **ไม่ขึ้นกับ export checkboxes** แต่ activity export ส่งเฉพาะ selected payload (D12, D17–D18) historical cutoff เป็น activity end; evidence end ต้อง `<= evidenceCutoff` และ user calibration/physiological settings ที่ใช้เป็น historical input ต้องทราบได้ไม่หลัง cutoff; แยก `asOfActivityEnd/evidenceCutoff` จาก `computedAt` ไม่มี future leakage หรือ latest fallback การประมวลผลย้อนหลังด้วย algorithm/config version ใหม่ทำได้เมื่อระบุ version และ computedAt จริง แต่ห้ามนำ future user calibration/data มาใช้ย้อนอดีต
- **late import:** เมื่อ import กิจกรรมเก่าที่สิ้นสุดเวลา t ให้พิจารณา affected cutoffs ตั้งแต่ t ตาม lookback/comparability ของ method รวม latest ไม่ใช้ upload time จัดลำดับ fitness chronology ตรวจทั้ง prior dependency references และ cutoffs ที่กิจกรรมใหม่นี้อาจเพิ่งเข้าเกณฑ์ จากนั้น invalidate version/input digests และ enqueue bounded jobs ไม่ decode ทุก owner โดยไม่มีเหตุ การ delete หรือเปลี่ยน duplicate group ใช้หลักเดียวกันและล้าง references ที่ห้ามเก็บ
- **latest/trend:** latest attempt และ prior valid result มี status/freshness คนละรายการ เมื่อ recompute fail แสดง last-good stale พร้อมเหตุ ไม่ใช้เป็น current valid เงียบ ๆ; trend จัดตาม evidence/fitness date แยก revision/computed date และมี provenance; trend เฉพาะเว็บ ไม่ส่งออกบัญชี history ทั้งหมด
- **verification/exit:** import A/B แล้วเพิ่มกิจกรรม C ที่เก่ากว่า ตรวจว่า historical cutoffs ที่ได้รับผลกระทบเปลี่ยนและ earlier cutoffs ไม่เปลี่ยน; future activity/calibration ไม่รั่ว; independent duplicates ไม่นับสองครั้ง; delete evidence แล้ว revoke trace/snapshot; export historical activity ไม่ได้รับ latest LT แทน; latest attempt ที่ fail ไม่เขียนทับ good revision
- **blocker/dependency:** P2 job/CAS/delete, P3 grouping, released target methods; history infrastructure ไม่เท่ากับ numerical LT completion

### P4.5 Real trace, status และ uncertainty ที่ไม่หลอกผู้ใช้

- **paths:** proposed `runs/{thresholds,model,export}.rs`; existing `components/analysis-summary.tsx`, detail/history routes; proposed frontend `components/threshold-status.tsx`
- **พฤติกรรม/ข้อกำหนด:** trace สร้างจาก actual numerical engine ระบุ target/method/version/formula, normalized input revision/hash, cutoff, candidate/accepted/rejected counts/reasons, parameters, actual intermediate calculations, evidence dates, conflicts และ limits (D11, D14–D15, D17) detail owner inspection กับ privacy-safe export เป็น projections คนละขอบเขต ไม่แนบกิจกรรมไม่ได้เลือกใน export
- **status:** ใช้ target status เพียง `estimated | low_confidence | insufficient_data` แยก LT1/LT2 ตาม assessment; `experimentalEstimate`, `researchBlocked`, `methodNotReleased`, `contextUnverified`, conflict และ freshness เป็น metadata/reason ไม่ใช่ status ที่สี่ Positive experimental proxy เป็น low_confidence พร้อมค่า/label/limits; target ที่ไม่มี usable value เป็น insufficient_data แต่ UI ต้องบอก engine unavailable/research gate ตรง ๆ ไม่อ้างว่าข้อมูลผู้ใช้ไม่พอจากการวิเคราะห์ที่ไม่เคยรัน Job failure แยกจาก physiological result และไม่เขียนทับ last-good Device-recorded metrics แยก group/label; quality score ไม่ใช่ physiological accuracy probability และไม่สร้าง personal CI จาก published LOA
- **verification/exit:** positive output trace ย้อนคำนวณ intermediate/result ได้จาก fixture; rejected segment trace ตรง engine decision ไม่ใช่ canned explanation; conflicting methods/recorded vs estimated ชัดทั้ง UI/export; trace privacy ไม่รั่ว nested history
- **blocker/dependency:** P4.2/P4.3 ของ target ที่แสดง และ export privacy; mock trace ไม่ผ่าน gate

### P4.6 Target-specific data-collection suggestions

- **paths:** proposed `runs/thresholds.rs` reason-to-suggestion policy; frontend `components/threshold-status.tsx`, coach export projection
- **พฤติกรรม/ข้อกำหนด:** optional short suggestion มาจาก actual target-specific missing input/rejected reason เช่น stream coverage หรือ protocol ที่ method ต้องใช้ตาม assessment (D16) ระบุข้อจำกัดและรองรับกรณีไม่มี suggestion ไม่เป็น training plan ไม่สัญญาว่าทำตามแล้วได้ threshold แม่น และไม่แนะนำ exhaustive/maximal effort โดยไม่มี safety/applicability policy
- **verification/exit:** LT1/LT2 reasons ที่ต่างกันให้ข้อความตรง target; conflicting หรือ unsupported method ไม่ให้คำแนะนำทั่วไปที่อ้างว่าแก้ข้อจำกัดได้; ไม่ละเมิด no-required-label policy; UI แสดง suggestions สั้นและแยกจาก calculated result
- **blocker/dependency:** frozen method eligibility/reasons ใน P4.1 และ engine trace; suggestion content ต้องตรวจโดยผู้มีความรู้ domain ก่อน release หากเป็น protocol ที่มีความเสี่ยง

**P4 exit:** numerical implementation ของทุก target ที่ประกาศ supported ผ่าน method/reference gate และมี actual engine evidence; target ที่ยัง blocked ต้องบอกตรง ๆ และไม่ถือว่าครบ D11–D18 ทั้งหมด ผู้ใช้ต้องอนุมัติ release scope ที่ต่างจากทั้งสอง target ก่อนปล่อยบางส่วน ห้ามแอบลด P4 เหลือ status-only engine

## P5 — PNG, mobile, prompts และ protected operational release

### P5.1 Numeric PNG ด้วย browser Canvas

- **paths:** proposed `apps/web/src/components/run-share-card.tsx`, `apps/web/src/lib/run-share-card.ts`, existing detail route/analysis summary; reuse browser Canvas และ shared numeric rounding policy ไม่เพิ่ม renderer service
- **พฤติกรรม/ข้อกำหนด:** สร้าง numeric minimal PNG ไม่มี charts/route/account มี presets อย่างน้อยสาม layout ที่จัดวางต่างกัน มี whitespace และ square/portrait พร้อม Light/Dark/real alpha Transparent (D22) ค่าเริ่มต้นเป็น distance, timer duration ที่ระบุชื่อชัดเจน และ pace; HR/power เป็น optional และค่าที่ไม่มีต้องไม่กลายเป็นศูนย์ เปลี่ยน preset แล้วรักษา metric choice; date/name ต้อง opt-in ไม่ใช้ upload filename แทนชื่อ
- **background:** รับ optional JPEG/PNG/WebP local raster จำกัด bytes/pixels และ reject SVG/HTML/unsupported image จัดการ orientation/crop, revoke object URLs และ cleanup ภาพที่ rasterize ออก Canvas ไม่ copy EXIF; no-photo ทุก mode ต้องทำงาน ข้อความไทย/อังกฤษที่ยาวและ extreme/missing values ต้องไม่ทับกัน
- **verification/exit:** เปิด browser preview และตรวจ PNG pixels จริงว่าตรง pinned revision/rounding ตรวจ alpha channel ว่าโปร่งใสจริง ไม่ใช่สีพื้นชื่อ Transparent ทดสอบทุก preset/size ทั้ง photo/no-photo, orientation/crop, oversized/malformed image, font loading, long Thai/English text และ download จริงบน mobile; ภาพไม่ถูกส่งไป API
- **blocker/dependency:** P2 normalized summary และ privacy; target pixel sizes/raster limits เป็นข้อเสนอที่ต้องวัด ไม่กล่าวอ้างผล visual ที่ไม่ได้เปิดดู

### P5.2 Prompt templates แยกจาก data

- **paths:** proposed `apps/web/src/components/coach-prompt-editor.tsx`, existing clipboard helper/`components/json-copy-actions.tsx`, detail/history export controls
- **พฤติกรรม/ข้อกำหนด:** ให้แก้และ copy short ChatGPT/Claude prompt templates แยกจาก coach/full data ไม่ embed prompt ใน JSON, ไม่ส่ง runtime LLM request และไม่มี API key/config ที่จำเป็น (D01, D23) prompt ไม่เพิ่ม selected activities หรือ history และไม่อ้างผล LT ที่ยังไม่ released
- **verification/exit:** copy prompt ได้ตามข้อความที่ผู้ใช้แก้; Copy data ไม่แนบ prompt; clipboard errors แสดงจริง; network observation ไม่มี LLM call และผู้ใช้ทำ manual handoff ได้จริง
- **blocker/dependency:** P2 selection/clipboard; external LLM integration อยู่นอก scope ไม่ใช่ blocker

### P5.3 Mobile และ state coverage ของทั้ง Runs flow

- **paths:** existing upload/history/detail routes, `components/{history-table,upload-dropzone,activity-charts,json-copy-actions}.tsx`, proposed share/threshold/prompt modules
- **พฤติกรรม/ข้อกำหนด:** upload/latest LT/checks/count/clear/select-current-page, processing/reprocessing/stale, sourceUnavailable, empty/loading/error/partial และ delete confirmation เข้าถึงได้ทั้งมือถือ/desktop (D07–D10, D14, D17, D19–D23) มี touch/keyboard targets, accessible labels/focus และข้อความไทย/อังกฤษที่อ่านได้; chart zoom/inspector, large export fallback และ no-photo sharing ต้องไม่ถูกซ่อนเพราะ viewport
- **verification/exit:** ใช้งานจริงตั้งแต่ upload ถึง selection, copy/download, chart, LT states และ PNG ใน supported browser/mobile matrix เก็บ screenshot และ interaction proof สำหรับ states สำคัญ ไม่ใช้ unit tests แทน surface proof
- **blocker/dependency:** feature modules ของ states ที่จะปล่อย; current E2E URL-order failure ต้องแก้ root cause และมี regression ก่อนอ้างว่า full workflow ผ่าน

### P5.4 Protected compatibility regression และ release budgets

- **paths:** `apps/api/tests/{http,db,model_contract,admin_transcript_entries}.rs`, CLI tests, `apps/web/e2e/extractions.spec.ts`, catalog/auth/admin tests, `scripts/e2e.ts`; `verification-plan.md`
- **พฤติกรรม/ข้อกำหนด:** รักษา Shoes list/detail/catalog/images, Google login/logout/session, authenticated/foreign-owner guards, FIT Coach OAuth/client contracts, admin transcript และ legacy migrator (D24) ใช้ disposable DB เท่านั้น เพราะ existing test helpers truncate protected tables; ไม่ใช้ SQL เหล่านั้นเป็น reset runbook
- **performance gate:** reference proposal คือ app 2 vCPU/2 GiB, PostgreSQL 18 แยก และ local SSD กำหนด list 50 rows p95 ≤300 ms ไม่รวม WAN; 1 h/1 Hz detail interactive ≤2 s หลัง fetch; inspector ≤100 ms; 20 MiB decode ≤60 s และ child RSS ≤512 MiB ภายใต้หนึ่ง worker; ten-source mixed batch ≤10 min; aggregate API RSS ≤1 GiB; full export 100,000 samples หรือ licensed large fixture ต้องครบโดยไม่มี cap เสนอ extra RSS ≤256 MiB, first byte ≤2 s และ throughput ≥5 MiB/s local; PNG 1080×1920 ≤2 s หลัง font/image load ตัวเลขทั้งหมด **proposed/unmeasured** ไม่ใช่ guarantees หรือ physiological thresholds
- **verification/exit:** ตรวจ scoped behavior regression ก่อน full-suite run สุดท้าย และใช้งาน actual workflow พร้อมเก็บผล ไม่ใช่เพียง exit status รายงาน benchmark ต้องระบุ hardware/fixture/bytes/peak RSS/WAL/DB+backup growth/time/completeness; privacy-safe logs ไม่บันทึก FIT/raw/coords/device IDs/filenames/token เมื่อ budget ไม่ผ่านให้แก้ bottleneck หรือขออนุมัติปรับ budget ไม่ truncate export เพื่อลดข้อมูลเงียบ ๆ
- **blocker/dependency:** P2–P5 feature gates และข้อมูล deployment hardware/observability ที่ได้รับอนุญาต; ผลบน baseline machine ไม่แทน production performance

### P5.5 Recovery, cutover และ Runs-only reset rehearsal

- **paths:** `apps/api/migrations/`, `apps/api/src/runs/{store,jobs,export}.rs`, deployment/backup documentation เดิม และ recovery sections ใน `architecture.md`/`verification-plan.md`; **ยังไม่มี reset executable ในรอบนี้**
- **พฤติกรรม/ข้อกำหนด:** ทำ restore/integrity/cutover rehearsal ใน disposable environment หลัง approval (D04–D06, D18, D21, D24) อนุมัติ reset แยกจาก implementation และ deploy; legacy sourceUnavailable เก็บไว้ตาม policy จนมี explicit decision ว่าจะ preserve หรือทำ Runs-only reset
- **verification/exit:** dry-run digest/target/scope เปลี่ยนเมื่อข้อมูลหรือ environment เปลี่ยน; stale confirmation ใช้ไม่ได้; protected before/after เท่ากัน; source hashes/owner/revisions หลัง restore ถูกต้อง; crash ระหว่าง apply ไม่ทำ partial reset; new writes หลัง cutover ไม่หายจาก rollback
- **blocker/dependency:** authorized target identity, backup access/retention, restore rehearsal และ operator approval; ตอนนี้ทำได้เพียง runbook design ด้านล่าง ไม่ติดต่อฐานข้อมูลจริง

## Runs-only reset runbook design — dry-run ก่อนเสมอ

ข้อเสนอนี้ไม่ใช่คำสั่ง executable และไม่อนุญาต reset ตอน startup/deploy ไม่มี `DROP DATABASE`, global `TRUNCATE CASCADE` หรือการลบ `users` เพราะ Runs tables และ auth-owned foreign keys บางส่วน cascade จาก users

1. **ระบุ target และ scope:** operator เลือก environment ที่อยู่ใน allowlist, PostgreSQL server/database identity/fingerprint, migration/schema version, backup policy และ scope แบบ owner ที่ระบุหรือ all Runs อย่างชัดเจน ห้ามอนุมานจาก `.env`, default connection หรือชื่อ host เพียงอย่างเดียว fingerprint ประกอบด้วย deployment identity และ DB identity ที่ตรวจได้ ไม่ใส่ secret ใน report
2. **ตรวจ catalog ก่อน dry-run:** allowlist เฉพาะ Runs คือ existing `extractions`, `activities` และ new `runs_*` ที่ตรวจ schema แล้ว รวม import items/jobs/revisions/estimates/export snapshots ที่เป็น Runs ตรวจ FK ทุกเส้นและ shared dependencies; `activities.id` FK ไป `extractions.id`, `activities.owner_id` ไป `users.id` และมี cascade ไม่ใช่ตารางชื่อ `activity_rows` หาก schema ไม่ตรงให้หยุด ไม่ใช้ prefix wildcard ลบตารางโดยไม่ตรวจรายชื่อ
3. **กำหนด protected invariants:** ห้ามลบ/แก้ `users`, `sessions`, `oauth_states`, `oauth_login_requests`, `oauth_authorization_codes`, `oauth_access_tokens`, `oauth_refresh_tokens`, `transcript_entries`, `legacy_imports` รวม Shoes checked-in catalog/static assets การ reset Runs ไม่ลบ account แม้เจ้าของไม่มี Runs เหลือ
4. **สร้าง dry-run report เท่านั้น:** แสดง environment/fingerprint/scope, Runs counts ต่อ table/source bytes, owner IDs ใน scope ภายใต้ access control, expected cascades, protected before counts, jobs/exports ที่ต้อง quiesce และ backup requirement สร้าง canonical digest ของ target+scope+schema+counts+data generation/IDs ที่เกี่ยวข้องเพื่อตรวจการเปลี่ยนแปลง ไม่ถือ count อย่างเดียวว่าพิสูจน์ว่าข้อมูลไม่เปลี่ยน Protected tables มีทั้ง total counts และ scope-related counts/digest ที่ไม่เผย token หรือ payload
5. **backup/restore ก่อน apply:** เก็บ consistent PostgreSQL snapshot ที่รวม BYTEA originals และ revisions พร้อม manifest/checksums; encrypt/จำกัด access และบันทึก backup identifier/cutoff ทำ restore rehearsal ใน disposable DB และตรวจ hash/owner/manifest/protected state ให้ผ่านก่อนอนุญาต apply ต้องกำหนด retention ของ backup ที่มี Runs ลบแล้ว และป้องกัน restore ที่ resurrect ข้อมูลโดยไม่ตั้งใจ
6. **quiesce แล้ว dry-run ใหม่:** ปิด new imports/reprocess/export สำหรับ scope, revoke active export tokens/หยุด streams, cancel และ reap decoder children; ใช้ lease/fence ป้องกัน stale worker publish Lock scope อย่างจำกัด ไม่ lock หรือ reset auth tables หลัง quiesce ให้สร้าง report/digest ใหม่เพื่อยืนยัน state จริง
7. **exact confirmation:** operator ต้องยืนยันข้อความที่ประกอบด้วย environment, full fingerprint, exact scope, backup identifier และ full dry-run digest ทั้งหมดตรงกัน ไม่รับ `yes` ทั่วไป หาก generation/schema/count/digest เปลี่ยนต้อง dry-run และ confirm ใหม่ การอนุมัติเอกสารนี้ไม่ใช่ confirmation
8. **apply หลังได้รับสิทธิ์เท่านั้น:** transaction ลบตาม allowlist และ FK order, tombstone/fence jobs, revoke exports และ invalidate estimates ที่เกี่ยวข้องโดยไม่ลบ user/shared records หาก fail ต้อง rollback ทั้ง transaction ไม่มี deploy hook และไม่ใช้ SQL จาก test truncate helpers
9. **ตรวจหลัง apply:** protected counts/digests เท่ากับก่อน; Runs เฉพาะ scope ถูกลบครบ; owner อื่นไม่เปลี่ยน; ไม่มี orphan/source/revision/evidence หรือ active job/token ที่จะ resurrect; health/login/FIT Coach/admin/Shoes ยังทำงาน Resume imports เมื่อ verification ผ่านเท่านั้น
10. **restore เมื่อจำเป็น:** quiesce ก่อน restore ใช้ backup ที่ยืนยัน fingerprint/cutoff และ rehearsal แล้ว รักษา protected writes ที่เกิดหลัง backup ด้วย restore/cutover plan ที่ได้รับอนุมัติ ไม่ restore whole DB ทับ auth changes เป็นทางลัด ตรวจ deletion tombstones กับ backup data เพื่อไม่ฟื้นกิจกรรมที่ผู้ใช้ลบหลัง backup และเก็บ audit record ที่ไม่มี private payload

## Migration rollback และ data safety

ก่อนมี new writes สามารถยกเลิก read cutover และคง additive schema ไว้ได้ แต่ **หลังมี new writes ห้าม rollback ด้วยการ drop ตารางใหม่หรือ restore old backup ทับข้อมูล** เพราะ original FIT และ revision ใหม่ไม่มีใน legacy DB

แนวทางที่เสนอคือหยุด writes/workers/exports ที่ได้รับผลกระทบและคง BYTEA/revision ledger ทั้งหมดไว้ ย้อนเฉพาะ UI/API reader ที่ยังอ่าน schema ใหม่ได้ หรือใช้ **safe roll-forward** แก้ reader/job/publish จาก source ที่ยังอยู่ Rebuild derived stages ที่ fail ได้โดยไม่เสีย original และ stable IDs ถ้า old binary อ่าน new schema ไม่ได้ต้องใช้ maintenance จน compatible reader หรือ roll-forward พร้อม ไม่เขียนกลับ old raw schema แบบ lossy เพื่อให้ old binary รัน เมื่อ resume ต้อง sweep desired versions และ reconcile queued/running leases ไม่ restore deleted data จาก old revision/backup โดยข้าม tombstone

cutover exit ต้องมี client inventory/deprecation approval, owner/ID/count/hash reconciliation, backup/restore proof และ protected regression ถ้ายังมี consumer พึ่ง old extraction exports ให้รายงานว่า transition blocked ไม่เรียก indefinite shim ว่า migration เสร็จ

## Requirement coverage และ dependency

| ข้อกำหนด | งานหลัก | dependency/เงื่อนไขเสร็จ |
|---|---|---|
| D01 manual LLM handoff | P2.5, P5.2 | Copy/Download จริง ไม่มี runtime LLM |
| D02 Garmin running only | P2.2 | profile-backed compatibility และ unsupported reasons |
| D03 FIT/ZIP, retain FIT | P2.1–P2.2 | immutable BYTEA/atomic dedup/per-member results |
| D04 source/decoded/normalized/derived | P2.1, P2.3–P2.4 | fidelity gate และ coherent revision publish |
| D05 every decoder-supported field | P1.1, P2.3 | dictionary/identity/unknown fixtures |
| D06 automatic reprocess/stable IDs | P2.4, P3.3, P4.4 | desired version scan/CAS/chronological invalidation |
| D07 separate coach/full Copy | P2.5 | shared snapshot และ honest clipboard errors |
| D08 explicit one/many selection | P2.5 | owner-scoped complete-or-error ไม่มี auto-history |
| D09 readable JSON/names | P2.3, P2.5–P2.6 | v2 contract แยก CLI 1.0.0 |
| D10 both downloads/no truncation | P2.5, P5.4 | same payload/privacy และ large-data completeness |
| D11 recorded vs estimated LT | P4.1–P4.3, P4.5 | independent method gates ไม่ rename recorded |
| D12 automatic owner evidence | P3.3, P4.4 | checkbox-independent/duplicate grouping |
| D13 FIT-only/no workout label | P3.2, P4.1–P4.3 | per-segment eligibility ไม่เดา intent |
| D14 target-specific states | P4.2–P4.3, P4.5 | availability แยกจาก physiological insufficiency |
| D15 inspect method/formula/trace | P4.1, P4.5 | actual engine/intermediate calculations ไม่ mock |
| D16 optional short suggestions | P4.6 | target-specific reason ไม่ใช่ training plan |
| D17 latest LT/web trend | P4.4–P4.5 | fitness date ต่างจาก computedAt; trend ไม่ export |
| D18 historical no-future evidence | P2.4, P4.4 | cutoff/availability/late-import/delete cases |
| D19 Pace/HR/Power/Laps first | P2.3, P3.1 | gaps/units/inspector/zoom จริง |
| D20 laps vs segments/overlays/coach | P3.1–P3.2, P2.5 | bounds/provenance ตรงกันและ archive ไม่ลดข้อมูล |
| D21 default privacy/opt-in/omissions | P2.5–P2.7 | recursive fail-closed/old endpoint transition |
| D22 numeric PNG/presets/themes/photo | P5.1, P5.3 | raster/alpha/mobile/metrics consistency |
| D23 editable separate prompts | P5.2 | prompt ไม่อยู่ใน data JSON |
| D24 Runs-only/protected/no deploy reset | P0, P2.6–P2.7, P5.4–P5.5 | protected regression และ approved runbook เท่านั้น |

## ความเสี่ยงและ prerequisite ที่ยังเปิดอยู่

1. **Decoder fidelity:** ต้องใช้ผล assessment จริงเพื่อเลือก implementation/schema การมี CRC tests ไม่ได้แปลว่า unknown/developer identity ครบ หาก fidelity gate ไม่ผ่าน original storage ยังมีคุณค่า แต่ full archive feature ยัง blocked
2. **LT reference:** วิธี LT1/LT2 อาจมี population/protocol ต่างกัน FIT ไม่ยืนยัน maximal intent หรือ lactate/ventilatory reference การเข้าถึง dataset ตาม license และ individual error validation เป็น release prerequisite ไม่ใช่งานที่จะซ่อนหลัง null-only engine
3. **BYTEA economics:** immutable source ช่วย atomic delete/backup แต่เพิ่ม WAL, DB size และ restore time ต้องวัดจริงก่อนยืนยัน storage ADR สำหรับ deployment หาก budget ไม่ผ่านให้แก้ข้อเสนอโดยเปิดเผย ไม่เพิ่ม object store เงียบ ๆ
4. **Resource enforcement:** killable child และ global slots ต้องพิสูจน์บน deployment platform จริง child memory limit, parser allocations, ZIP expansion และ export backpressure ยังไม่ใช่สิ่งที่ baseline ยืนยัน
5. **Privacy classification:** developer/unknown nested fields อาจใส่ location/device identity ที่ชื่อไม่บอก ต้อง fail-closed และมี omission manifest ไม่ redact เฉพาะ top-level lat/lon
6. **Compatibility inventory:** ต้องรู้ clients ของ v1 extraction exports/FIT Coach/CLI จากข้อมูลที่ได้รับอนุญาตก่อน retirement ไม่เปลี่ยน old schema โดยไม่มี approval และไม่ขยาย transition โดยไม่มี exit gate
7. **Temporal recomputation:** late imports/duplicates/deletion เปลี่ยน candidate history ต้องตรวจ potential cutoffs ไม่ใช่เฉพาะ prior references และไม่ให้ last-good อ้าง deleted private evidence
8. **Export/delete race:** immutable pinned revision ไม่ได้ให้สิทธิ์อ่านหลัง delete ต้องมี revoke/fence/stream abort proof ระบบเรียกคืน copies ที่ผู้ใช้ได้รับก่อน delete ไม่ได้
9. **Production capabilities:** environment identity, DB volume/backup access, restore procedures และ actual hardware ยังต้องใช้ข้อมูลที่ได้รับอนุญาต ไม่มีการอ่าน secrets หรือติดต่อ production ในรอบนี้
10. **Baseline failure:** protected E2E ยังมี URL-order failure ต้องแก้ root cause และมี regression ใน implementation phase ก่อน release claim ไม่กล่าวว่า tests ทั้งหมดผ่าน

## เกณฑ์ส่งมอบ

รอบนี้ส่งมอบ planning/audit/feasibility evidence และ proposed ADRs เท่านั้น P2–P5 ทั้งหมดเป็นงานอนาคตที่ยังไม่ได้ implemented/tested การผ่าน gate ต้องมี actual runtime proof ตาม verification plan ไม่ใช้เอกสารที่ครบหรือ build สีเขียวเป็นหลักฐานว่า feature เสร็จ

ก่อนปิดแต่ละ milestone ให้อัปเดต contract docs/changelog, affected callsites และ consumer tests ตามพฤติกรรมจริง ลบ throwaway spikes และ migration compatibility code ที่หมดหน้าที่หลัง gate ผ่าน Permanent tests ทดสอบ data/security/behavior invariants ไม่ทดสอบ source wording หรือ mock forwarding Full project validation ทำหลังงานทั้งหมดรวมแล้วครั้งเดียวใน disposable environment และไม่ปนกับ production reset
