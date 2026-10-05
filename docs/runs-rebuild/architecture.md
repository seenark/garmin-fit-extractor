# Runs Rebuild — Proposed architecture

**Status: proposed; ยังไม่อนุมัติและยังไม่มี production implementation.** ใช้ Runs Rebuild Requirements 1.0/D01–D24 เป็นขอบเขต ไม่เปลี่ยน product decisions ที่ล็อกแล้ว เอกสารนี้เลือกแนวทางทางเทคนิคพร้อม trade-offs และ gates; ไม่อนุญาต deploy/reset โดยปริยาย

อ่านร่วมกับ [audit](repository-audit.md), [decoder assessment](decoder-assessment.md), [LT assessment](lt-method-assessment.md), [implementation plan](implementation-plan.md), [verification plan](verification-plan.md) และ [glossary](../../CONTEXT.md).

## 1. Architectural shape

คง React/TanStack Router + Axum/PostgreSQL และ application shell เดิม ใช้ native browser Canvas สำหรับ PNG และ PostgreSQL สำหรับ durable jobs ไม่เพิ่ม chatbot, Python HTTP service, Redis, object-store service หรือ queue framework เพียงเพื่อเริ่มงาน

```text
FIT / ZIP
  authenticated bounded import
  immutable Original FIT + per-item report
  PostgreSQL jobs
  killable decoder process
  decoded revision -> normalized revision -> analysis revision
  validated coherent publish manifest -> stable Run Activity

Owner history + cutoff + eligible native inputs -> LT method -> historical snapshots
Explicit Export Selection + pinned manifests + privacy -> one JSON snapshot -> Copy / Download
Pinned activity summary -> browser Canvas -> PNG
```

Decoder runtime ยังต้องผ่าน fidelity gate จริงตาม assessment การเก็บ source ไม่ใช่เหตุผลให้ประกาศ full decoding เสร็จก่อน gate ผ่าน ถ้าต้องใช้ Python/JS เป็น decoder subprocess ให้เปลี่ยนเฉพาะ seam นี้ ไม่ rewrite API/auth/database ทั้งหมด (DEC-08–10, ARCH-01)

## 2. Data layers และ storage decision

### Original FIT ใน PostgreSQL BYTEA

เสนอเก็บ bytes ต้นฉบับใน PostgreSQL ที่ persist อยู่แล้ว พร้อม owner, SHA-256, size, integrity result และ provenance. ใช้ unique `(owner_id, sha256)` และตรวจ size/hash เมื่อชนกัน ไม่ dedup ข้าม owner หรือเปิดเผยว่าคนอื่นมีไฟล์เดียวกัน

เหตุผล: FIT limit เริ่มต้น 20 MiB; transaction เดียวครอบคลุม immutable source/dedup/revisions/delete และ backup DB รวม source ได้ ไม่ต้องแก้ปัญหา distributed DB/blob commit หรือเพิ่ม volume/storage service อีกชุด Trade-off คือ WAL/DB/backup โตและ decode ต้องอ่าน binary จาก DB; **ยังไม่พิสูจน์ว่าเหมาะกับ production volume** ต้องวัดใน P2/P5 ถ้า budgets ไม่ผ่านให้เปิด ADR ใหม่สำหรับ private object storage โดยรักษา source hash/owner contract ไม่แก้ด้วยการทิ้ง FIT

ZIP ไม่เข้า permanent storage. Import อ่าน compressed/uncompressed bytes แบบ bounded; temporary source/decoded/export spools อยู่ private process directory ที่ไม่ใช่ public root ใช้ names ที่ระบบสร้าง ไม่ใช้ member path เป็น destination. Cleanup หลังงาน, TTL sweep หลัง crash และ process startup; temp bytes ไม่ใช่ source archive

### Proposed logical persistence

ชื่อ table เป็นข้อเสนอ ไม่จำเป็นต้องหนึ่ง table ต่อ entity แต่หน้าที่และ invariants ต้องครบ:

| Logical record | Data และ invariant |
|---|---|
| SourceFit (`runs_sources`) | owner/hash/size/immutable BYTEA; import time แยก event time; no ZIP/base64 export |
| ImportBatch/ImportItem | owner, received inputs, member provenance ภายใน, imported/duplicate/unsupported/failed และ reason/warnings; item references source/activity เมื่อสำเร็จ |
| Run Activity (`runs_activities`) | stable UUID, owner, source+session boundary, subtype/event start/end, indexed list summary และ current coherent publish manifest |
| Revision (`runs_revisions`) | immutable stage kind decoded/normalized/analysis, schema/library/profile/options/method versions, config hash, exact input revision IDs, payload และ validation result |
| ThresholdEstimate | target/cutoff, method/config/input revisions, real trace/uncertainty/reasons, computedAt, current/replaced revisions, source lineage สำหรับเว็บ |
| ProcessingJob (`runs_jobs`) | stage/input/version key, desired generation, lease/attempts/progress/error IDs, owner/source/activity, cancellation state |
| ExportManifest | owner, explicit activity IDs, pinned coherent manifests, mode/privacy/schema/generatedAt/omissions, expiry; ไม่เพิ่มประวัติทั้งบัญชี |

Decoded ordered archive และ normalized document อยู่คนละ immutable payload. Summary projections/indexed columns ใช้ list/filter โดยไม่ fetch decoded JSON; detail/streams อ่าน normalized ของ **กิจกรรมที่เปิดเท่านั้น** ไม่โหลดทุก decoded archive. Full export อ่าน decoded เฉพาะ selection. หาก measured stream-query budget ไม่ผ่านจึงแยก samples เป็น indexed table/chunks ไม่เพิ่ม abstraction นี้ล่วงหน้า

Revision retention เริ่มต้นเก็บ current + revisions ที่ยังมี active estimate/export references; unreferenced superseded decoded/normalized revisions เก็บ 30 วันเป็น **proposed operational default**. Analysis/estimate trace/history เก็บจนลบ activity หรือถึง retention policy ที่ประกาศ. ถ้าลบ prerequisite แล้วผลเก่าไม่ reproducible ให้ระบุ `reproducibility=unavailable` และเหตุผล ไม่อ้างว่า hash อย่างเดียว reproduce ได้ (REP-05)

## 3. Import compatibility และ integrity

- รับ direct FIT/ZIP แบบ case-insensitive suffix และตรวจ header/content/CRC ไม่เชื่อชื่อไฟล์ ไม่ salvage corrupt เป็น success ในรุ่นแรก
- รักษาขอบเขต bytes เดิมเป็นค่าตั้งต้นที่ configurable: input ≤10, compressed/FIT ≤20 MiB, FIT members ≤50, extracted FIT รวม ≤100 MiB. เพิ่ม total member-count/compression/time/memory/queue limits โดยทดสอบ hard reads ไม่เชื่อ ZIP declared sizes
- ปฏิเสธ traversal/absolute paths/symlinks/nested archives ตาม policy ที่ประกาศ; unsupported non-FIT members รายงาน ไม่ execute/extract เข้า static. Limit breach ในหนึ่ง member ไม่เปลี่ยน valid siblings ที่นำเข้าแล้วเป็น failure ทั้ง batch; request-wide limit ต้องตรวจ staging ก่อน commit ตาม API contract
- Manufacturer จาก file/session/device metadata ที่เชื่อถือได้ต้องยืนยัน Garmin; sport/sub_sport ต้อง running ที่ profile/fixtures ยืนยัน ไม่ใช้ชื่อ workout. Foreign accessory sensor ไม่ทำให้ Garmin activity ถูกปฏิเสธ
- **รุ่นแรกเสนอ single-session FIT เท่านั้น**: มี running session เดียวและ boundaries/related records ตรวจได้. Multisession/multisport คืน `UNSUPPORTED_SESSION_LAYOUT` พร้อมเหตุผล ไม่รวมทั้งหมดเป็น run. นี่เป็น explicit IMP-04 policy ไม่ใช่การสมมติ 1 FIT = 1 session. Compatibility matrix เก็บ verified profile IDs/subtypes จาก decoder corpus ไม่เดาเลข enum
- Byte-identical source ของ owner เดียวกันคืน activity ID เดิม แม้ rename/ZIP/concurrent upload; unique constraints และ transactional insertion ตัด race
- Different-byte possible duplicates **ไม่ merge ด้วยวันที่**. เปรียบเทียบ activity identity ที่ verified ได้, exact start/end, lap/timer/distance/stream fingerprints ตาม deterministic versioned rules. เก็บ provenance ของทั้งคู่และ duplicate evidence. Observations ที่ตรงกฎอยู่ analytical observation group เดียว มี deterministic representative; weak evidence เป็น warning ไม่ claim duplicate. AT-04/15 ต้องไม่เพิ่ม LT confidence จากคู่ซ้ำ
- Unsupported/corrupt bytes ไม่เป็น permanent accepted source; import receipt เก็บ reason/counts/hash ตาม policy ไม่เก็บ sensitive raw payload ใน logs. Accepted FIT ต้นฉบับต้องคง bytes เดิม

## 4. Decoded / normalized / export schemas

Internal schema ใหม่ไม่ผูกกับ CLI `1.0.0`. เสนอ versioned JSON Schema สามชุด decoded/normalized/export พร้อม data dictionary และ golden validation; เปลี่ยน semantic/unit/type เป็น major version, additive optional fields เป็น minor, method/config version แยกจาก schema

### Decoded field contract

แต่ละ message มี sequence index, numeric global/local identity ที่อ่านได้ และ definition/source range reference. Field identity ใช้ native `(message identity, field number)` หรือ developer `(developer_data_index, field_definition_number, application identity when available)` ไม่ใช้ชื่อเป็น primary key

แต่ละ field เก็บ decoded value, native type/unit/validity, original-vs-expanded role, parent component/subfield และ raw encoded/scale/offset/enum code **เฉพาะที่ decoder เปิดให้**; unsupported metadata มี warning/source reference ไม่สร้าง metadata ที่ไม่เคยอ่าน. Unknown IDs/typed values คงภายใน. Ordered messages/field arrays ไม่เปลี่ยนเป็น object keyed ด้วยชื่อที่ชนกัน

Value semantics:

- absent: ไม่มี field; invalid: มี field แต่ FIT sentinel พร้อม validity; decode failure: warning/error ไม่เปลี่ยนเป็น valid null; valid zero คง zero
- ไม่ส่ง NaN/Infinity; invalid/null มีเหตุผล/type metadata; unsafe integers เป็น decimal strings พร้อม original type/encoding semantics
- Timestamp precision คงของ source; archive ไม่ smooth/downsample/interpolate. Compressed timestamps ต้อง recover ตาม format โดยยังรู้ sequence/source reference
- Decoder library/version/profile/options/warnings/source hash/startedAt/completedAt/decodedSchemaVersion อยู่ใน revision. Native vs expanded/merged HR options แยกชัด

### Normalized entity / units / lineage

แยก session, laps, samples, timer events, sensors และ historical zone contexts. ทุกค่าอ้าง decoded revision/message/field identity ได้. Metric `{value, unit, sourceReference, definition}` และ absent/invalid state ไม่ใช้ zero แทน missing

Canonical UTC + source local offset/timezone ถ้ามี; unknown เมื่อไม่มี ไม่อ่าน server timezone มาเดา. เวลา elapsed/timer/moving แยก พร้อม definition. Canonical seconds, meters, m/s, s/km, bpm, W, steps/min, altitude meters, Celsius. Pace display `mm:ss/km` เป็น presentation ไม่ทับตัวเลข

Precedence: enhanced speed/altitude ที่ valid และ definitions ยืนยันก่อน base field; ถ้าต้อง fallback ระบุ exact source. Native/developer power ไม่เฉลี่ยเอง; รักษาทั้ง sources แล้วเลือก designated stream ตาม verified profile/source identity policy. Cadence conversion ใช้ field definition + fixture ไม่ใช้ heuristic `<130`. Native device summary/laps แยก system-calculated counterparts; zero speed ให้ pace absent/not applicable ไม่ infinity. Time-weighted metrics ใช้ actual interval coverage, pause/gap boundaries และ explicit method transformation; ไม่ถือว่าทุก row ยาว 1 s

Zone mappings ใช้ settings ณ activity จาก source ที่เข้าใจจริง; unknown mapping ไม่ตั้งชื่อเองหรือใช้ current zones แทน. Device-derived thresholds/training effects แสดงเป็น recorded/deviceReported ไม่เป็นระบบ LT

Extension mechanism: ordered additional decoded fields + normalized `extensions` ที่มี namespace/compound identity/type/unit/source/classification. Extension ไม่ได้ bypass privacy allowlist; unknowns เก็บภายในแต่ default export omit

## 5. Quality และ detected segments

Quality engine ทำ stream coverage/gaps/timer states/duplicate/out-of-order timestamps/artifact suspicion/source changes/pause ratio/usable duration/environment unknowns แยกจาก LT validity. ไม่แก้ original sample order; analysis timeline เป็น derived view พร้อม mapping และ transformations

Segment detectorใช้ speed/workload/HR ตามที่มี ไม่ require name/labels/workout metadata. แสดง steady-workload/repeated surges/recovery-like/progressive/pause/unknown โดยไม่สรุปเจตนาหรือ maximal test. Garmin laps คงเดิมและ separate entities. แต่ละ segment มี start/end/time basis/version/features/accept/reject reasons และ per-method eligibility

Carryover/HR lag/gaps/source changes ต้องตรวจแยก ไม่เชื่อว่า cooldown หลัง interval มี steady physiology ทันที. HR drift เป็น signal ที่กำลังศึกษา ไม่ใช้ HR variance ตัด drift ออกหมด. Cadence-lock เป็น suspicion เท่านั้น. No-power activities ยัง import/chart/export และใช้ speed/HR ตาม method eligibility ได้

Candidate selection/weights/config/duration/CV/repeats เป็น declared versioned configuration ตั้งก่อน evaluation ไม่เลือก lowest drift/force breakpoint. Annotated development/validation sets แยกกัน; precision/recall ไม่ใช่ physiological accuracy (SEG-01–10)

## 6. LT engine และสองแกนเวลา

รายละเอียดวิธี/ข้อจำกัด/release gate อยู่ใน [LT assessment](lt-method-assessment.md) และ ADR-0005. Method ต้องมี implementation จริงและ positive/negative fixtures ก่อนเรียก numerical estimator implemented; null-only registryไม่ใช่งานเสร็จ. Runtime ไม่มี LLM/API key/user workout labels/RPE/lab requirements

แต่ละ target LT1/LT2 มี eligibility, method, status, value/null, uncertainty type, reasons, evidence period/independent count, actual trace และ suggestions แยกกัน. No fixed ratio/zone percentage/default HRmax/drift5% shortcut. ถ้า method คืนได้แค่ pace ไม่สร้าง HR/power เติม. Conflicts ไม่ silently swap/clamp/average

Numerical candidates ที่เสนอคือ running DFA-a1 0.75/0.50 สำหรับ **VT1/VT2 proxies** จาก exercise RR จริง ไม่ใช่ blood-lactate LT ที่ validated. Candidate ที่ผ่าน numerical/runtime gates แต่ยังไม่ผ่าน independent reference validation ใช้ `low_confidence` พร้อม experimental/target/limitations; ต้องแก้ decoder packed-HR bounds/accumulation และพิสูจน์ RR alignment ก่อนใช้ inputs. ไม่มี licensed paired running-RR/reference corpus ที่ยืนยันแล้ว จึงยังไม่ผ่าน physiological release gate

UI/API result แยก:

1. estimate status `estimated|low_confidence|insufficient_data`;
2. engine release/validation `experimental|supported|unavailable`;
3. job processing failure;
4. freshness/staleness.

Engine ที่ยังไม่ปล่อยไม่โทษว่าผู้ใช้ข้อมูลไม่พอ; show feature unavailable/research gate metadata ไม่สร้าง insufficient-data medical conclusion จาก server failure

### Historical cutoff

`asOfActivityEnd/evidenceCutoff` คือเวลาที่ fitness อ้างอิง; `computedAt` คือเวลา run engine. Query evidence owner เดียวกันและ **activity end ≤ cutoff** รวม activity นั้นได้เมื่อวิธีรองรับ. User-level tuning/calibration/source comparison และ parameters ที่เรียนจากข้อมูลต้องใช้ cutoff เดียวกัน ไม่ใช้ future evidence ทางอ้อม

Initial experimental policy ตาม method assessment เลือก latest eligible independent owner activity ภายใน lookback **7 วัน** โดยไม่ pool HR corrections หรือ threshold values ข้าม sensor/protocol/target definitions และไม่มี minimum session count. 7 วันเป็น proposed operational ceiling ไม่ใช่ guarantee ว่า fitness คงที่; หากเกินช่วงให้ latest attempt abstain ด้วย recency reason และแสดง older last-good เป็น stale แยกต่างหาก อายุ evidence ใน historical export วัดเทียบ cutoff ของกิจกรรมนั้น ไม่เทียบวันที่คำนวณ

FIT เก่าอัปโหลดวันนี้ invalidate cutoffs หลัง event end ที่ method lookback/comparability policy ครอบคลุม แล้วสร้าง retrospectively recalculated revisions. Export หลาย activities มี snapshot ของแต่ละ end ไม่ใช้ LT ล่าสุดร่วมกัน. No eligible result ณ cutoff คืน reason/null ไม่ fallback current. Latest card แสดง latest attempt และ last available numeric result พร้อม evidence date/stale limits แยกกัน; trend axis ใช้ fitness date ไม่ใช้ computedAt

UI evidence endpoint owner-scopedเปิด actual source/segment/calculation lineage ได้. Export projection **ไม่แนบ unselected activity IDs, samples, laps, names หรือ reports**; มี method/reference/target/parameters/necessary aggregate intermediate summary/count/date range/uncertainty/limits เท่านั้น. Web-only full lineage ระบุว่าดูได้ในเว็บ (EXP-07)

Suggestions เป็น rule/templates จาก method prerequisites/deficiency codes แยก LT1/LT2; purpose/prerequisites/steps/duration/intensity rationale/expected data/safety/limits, opt-in/dismiss ไม่เป็น task บังคับ. Missing sensor ให้แก้ recording ก่อน ไม่สั่งวิ่งเพิ่ม; ไม่ default maximal/all-out และไม่ตั้ง HR target จาก LT ที่ยังไม่มี

## 7. Durable jobs / invalidation / publish

ใช้ PostgreSQL queue: claim `FOR UPDATE SKIP LOCKED`, global stage slots/leases จำกัด concurrency ข้าม replicas, lease heartbeat/recovery. Proposed defaults: one decode worker slot, maximum three attempts สำหรับ transient failures, timeout/backoff configurable; invalid FIT/unsupported/method abstentionไม่ retry เป็น transient server error

Untrusted decode ใช้ subprocess mode ของ executable เดิม มี bounded input/output, timeout/kill และ Linux address-space/process resource limit ที่ต้อง smoke บน deployment image. ไม่อ้างว่า container memory limit อย่างเดียวคุม child peak memory ได้; ถ้า runtime enforce limit ไม่ได้ให้เป็น deployment gate. ไม่มี shell execution ของ filename/content

Idempotency key จาก owner/source hash or input revision IDs + stage/library/profile/options/schema/method/config versions. Activity identity ไม่ขึ้นกับ job ID. Desired version manifest ใน code/config + startup/periodic bounded scanner สร้างงานเมื่อ stored versions ไม่ตรง; import/delete/source revisions trigger dependency invalidation. เปลี่ยน estimator ใช้ normalized revision เดิมได้ ไม่ decode FIT ทั้งหมดโดยไม่จำเป็น

Publish revision ใหม่หลัง decode/normalize/quality/schema validation ครบ. Transaction compare-and-swap desired generation + current manifest + existence/tombstone. UI/export อ่าน coherent manifest ไม่ปน summary ใหม่กับ samples/analysis เก่า; LT ที่ต้องรอใหม่มี pending/stale metadata และ reference coherent base เดิม ไม่แอบถือว่าเป็น fresh. Failure คง last-good data และ `updateFailed/stale` reason ไม่ทิ้งข้อมูลดี

Deletion: mark tombstone/cancel job generation, invalidate affected estimates, erase snapshots/materialized tracesที่มีข้อมูล source ถูกลบ, prune unreferenced sources/revisions/export manifests/caches/temp spools, recompute remaining owner evidence. Worker checks tombstone before publish และ DB foreign keys/CAS กัน resurrection. Previous reproducibility history ต้องไม่ซ่อนสำเนาข้อมูลที่ผู้ใช้สั่งลบ

## 8. Export module interface และ privacy

เสนอ `POST /api/v2/runs/exports` รับ explicit `activityIds`, `mode=coach|full`, `includeLocation=false`, `includeDeviceIdentifiers=false`. ไม่รับ owner ID; reject empty/duplicate/foreign/missing/unready IDs ตาม error contract ไม่ silently export remaining selection. Select-all หน้า UI เป็น **current page** ในรุ่นแรกพร้อมจำนวน ไม่รวม filtered/paginated items ที่ไม่เลือกเอง

Server validate all IDs, pin coherent revisions ใน transaction และสร้าง owner-scoped snapshot token/manifest. Proposed TTL 15 min. `generatedAt`, selection order, schema/mode/privacy/omissions และ bytes ของ snapshot คงเดิมระหว่าง Copy/Download; mode/privacy change สร้าง snapshot ใหม่โดยแจ้ง ไม่ fetch latest manifests คนละครั้ง. No account/history/revision dump/FIT bytes/base64/uploaded names/private paths/email

Full mode เก็บทุก selected decoded/normalized/derived field/sample ที่ export policyอนุญาต ไม่ downsample/smooth/roundเพิ่ม. Coach mode aggregate แยก boundaries ของ laps/segments/repeats/surges/pauses/gaps พร้อม source count, method/resolution/precision transformations และ quality warnings. Objects/meaningful names/units/ISO timestamps/indentation ไม่ columns/rows encoding

### Privacy projection

ใช้ compound numeric/profile/developer identities + structure classification ไม่อิงแค่ string key `latitude`. Categories location/deviceIdentifiers/unclassified ครอบคลุม coordinate semicircles/start/end/lap/bounding boxes/routes, serial/unit IDs, nested native/developer/expanded/metadata duplicates. Keep safe sensor type/units เพื่ออธิบายวิธี แต่ไม่ unique identity

Unknown opaque data ยังเก็บภายใน แต่ default omit หากพิสูจน์ safe ไม่ได้. รุ่นแรก **ไม่เปิด includeUnclassified**; location consent ไม่ยินยอม unknown/device โดยปริยาย. `privacyOmissions` แสดง category/path pattern/count/reason ไม่ใส่ hidden value. Absent source field ไม่ถูกนับว่า redacted. Manifest/privacy applies to both modes and transports; archive untouched. ไม่อ้าง anonymous 100% เพราะ dates/activity details ยังอยู่

### Export/delete race semantics

ก่อนเริ่ม response ต้องตรวจ owner/existence/revisions อีกครั้ง; materialization ที่ถูก invalidated คืน error ไม่สร้าง partial success. Serving download/Copied bytesถือ selected rows/manifest read lease กับ delete/publish linearization guard. Delete request cancel active exports ได้; deletion commit ต้องไม่แซง guarded transport โดยปล่อย complete JSON ของสิ่งที่ลบแล้ว. หาก delete/revoke ชนะก่อน transmission completes ให้ abort stream และ browser ไม่ save/claim complete; incomplete network dataไม่ใช่ completed export

ข้อมูลที่ส่งให้ browser สำเร็จ **ก่อน deletion commit** ไม่สามารถเรียกคืนได้ ไม่สัญญาลบ downloaded copies ของผู้ใช้. Test AT-33 ต้องกำหนด barrier phases/materialize/before-response/midstream และเวลาที่ operation linearize ชัดเจน. Read leasesมี timeout/cancel/drop cleanup ไม่เปิด DB transaction ค้างไร้ขอบเขต

No silent truncation: preview estimated byte size, clipboard success หลัง await write สำเร็จเท่านั้น; failure แจ้ง reason และ download snapshot/selection เดิม. Browser saves only complete response. `Cache-Control: private, no-store`, no CDN/private payload logging/static bundling

## 9. Legacy compatibility และ migration boundary

- CLI command/schema `1.0.0` และ output/error semantics **คงเดิม**; ไม่ freeze internal/new export schema เป็น v1. CLI ไม่ใช้ export checkbox/privacy web contract และไม่ต้องย้าย decoder เพียงเพราะ Runs เพิ่ม fields
- FIT Coach OAuth และ `/api/v1/activities*` **คง response contract** ผ่าน explicit projection ของ published normalized revision. ใช้ stable activity IDs; ไม่เพิ่ม latest-history behaviorใน JSON ใหม่. แยก v1 legacy derived drift ไม่เรียก LT
- Existing browser `/api/v1/extractions` เป็น Runs contract ที่จะเปลี่ยนตาม approved migration ไม่ใช่การรับรอง preserve old raw/privacy semanticsตลอดไป. เสนอเปลี่ยน web callers ไป v2 พร้อมประกาศ release note/route schema; retire old upload/download endpoints หลัง migration gate ไม่สร้าง indefinite shims/second normalizer. Old bookmarked detail routes redirect stable ID ไป new detail; announced API retirement errors ไม่ silent shape change
- Legacy rows ไม่มี original FIT: keep read-only/sourceUnavailable พร้อม fidelity warnings, event timeเดิม และ safe export projectionของ stored data หาก new schema validatorรองรับ. ถ้า export guaranteesทำไม่ได้คืน explicit `LEGACY_EXPORT_UNSUPPORTED` ไม่เรียก old unredacted endpointเป็น fallback. No automatic original reconstruction. Legacy raw browser view ไม่ใช่ unredacted Copy/Download loophole
- ระหว่าง new Runs cutover FIT Coach DB projectionต้องยัง lookup legacy `extractions/activities` และ new activity sourceตาม explicit resolver จน data migration เสร็จ; projectionไม่สร้าง new UUID ทุก retry. อย่าสร้าง fake successful extraction เพื่อให้ FK เก่ารับ source ใหม่. Additive FK/schema plan ต้องตัด dependency `activities.id -> extractions.id` อย่างเจาะจงเมื่อ approved โดยรักษา owner FK/OAuth contract ไม่ลบ auth tables
- Source-less Runs reset เป็น **ทางเลือกต้องคำสั่ง explicit แยก** ไม่ใช่ default migration. ไม่มี startup hook/deploy scriptล้างข้อมูล

## 10. Charts, PNG และ prompts

Charts reuse installed `@tanstack/charts` 0.16.0 หลัง capability spike เรื่อง missing gaps/zoom/inspect/mobile/linked times/overlayผ่าน. Separate Pace/HR/Power plots ใช้ timelineเดียวกัน; pause/gap breaks, Garmin lap markers และ accepted/rejected segment overlaysคนละ style. Inspect segmentแสดง features/eligibility reasons ไม่ใช่ quality percentage. Downsampling/smoothing display-only with metadata; numerical engineไม่รับ chart samples

PNG ใช้ browser Canvas `toBlob('image/png')`, shared display roundingจาก pinned revision, default distance/timer timeที่ labelชัด/average pace, optional HR/powerเมื่อมี. Proposed three layouts bottom-left/lower-center/bottom-right, square1080×1080/portrait1080×1920; numbers/presetsเป็น design proposals ไม่ Instagram spec. Light/Dark/Transparentมีจริงไม่ require photo. Transparent canvasไม่ paintพื้นหลัง/checkerboard; preview contrastบน bright/dark backgrounds และตรวจ alpha output

Photo handling local-in-browser raster-only JPEG/PNG/WebP, proposed ≤10 MiB/≤24 million pixels, decode orientation/crop/position, no SVG/HTML/scripts, verify dimensions before large allocation, remove blob URLs/imagesเมื่อเปลี่ยน/ปิด. Canvas renderไม่ copy EXIF/location metadataไป PNG. Preset changeคง selected metrics; no route/device/account/name/dateโดยปริยาย. Thai/English labels/short distance/over-marathon/hour durations/long valuesต้อง visual test

Prompt templatesเป็น editable textที่ Copyแยก ยึดเฉพาะ attached selection แยก recorded/estimated และ acknowledge missing evidence. ไม่ฝัง instructionsใน JSON และไม่เรียก AI runtime

## 11. Reset, backup, rollback และ operations

**ไม่มี reset command execution ในรอบนี้.** Runbookหลังอนุมัติ implementation:

1. Dry-run default; แสดง deployment environment/DB fingerprint/owner-or-all-Runs scope, exact allowlisted tables/storage, counts/bytes/jobs/exports และ protected table counts. ไม่แสดง emails/FIT/pathsจากผู้ใช้
2. Quiesce affected owner imports/jobs/exports, consistent backup PostgreSQL+original BYTEA+revisions; record schema/application versions/hash manifests. Restore rehearsalใน DB/containerใหม่ แล้ว checksum/redecode original; encrypted restricted backupและ deletion retention policyที่ประกาศ
3. Applyต้อง explicit environment+scope+dry-run digest confirmation. Recompute dry-runและ abortหาก counts/versionเปลี่ยน ไม่ `DROP DATABASE`, broad `TRUNCATE CASCADE`, delete users หรือ volume deletion
4. Transaction deletes only approved Runs rows in FK order; assert protected counts/content hashes unchanged. Cleanup private temp remnants; affected worker generation canceled. Container recreationไม่ reset
5. Restore consistency/orphan/source hash/reference checks; run Shoes/auth/admin/CLI/FIT Coach regression. After new writes, rollback code onlyเมื่อ schema/data compatible; มิฉะนั้น maintenance+roll-forwardหรือ separately approved restoreที่แจ้ง potential newer-write loss ไม่ย้อน DBเงียบๆ

Single PostgreSQL backup covers original+derived dataใน proposed layout. If later external blobsถูกเลือก ต้องเพิ่ม coordinated immutable blob snapshot/manifest before DB snapshotและ orphan checks ไม่อ้างว่า DB dumpพอ

Observabilityใช้ counts/latency/stage/error IDs/reason categories ไม่ raw samples/GPS/emails/tokens/filenames. Metricsแยก processing failuresจาก estimator abstention. API health/worker backpressure visible; queue growthไม่ lock list/export last-good.

## 12. Decision records และ exit gates

| ADR | Requirement / gate |
|---|---|
| [0001 decoder fidelity](../adr/0001-runs-decoder-fidelity.md) | ARCH-01, DEC-08–10; corpus/metadata/options gapsก่อน runtime choice |
| [0002 source storage](../adr/0002-runs-source-storage.md) | ARCH-02, IMP-05, OPS-04/05; persistent bytes/backup/WAL benchmarks |
| [0003 schema compatibility](../adr/0003-runs-schema-compatibility.md) | ARCH-03; CLI/FIT Coach preserved, extraction migration explicit |
| [0004 processing jobs](../adr/0004-runs-processing-jobs.md) | ARCH-04, REP-01–06; version scanner/leases/CAS/delete/export races |
| [0005 LT methods](../adr/0005-runs-threshold-methods.md) | ARCH-05, LT-13–16; real method/positive-negative/physiology gate |
| [0006 charts/share card](../adr/0006-runs-charts-share-card.md) | ARCH-06; mobile gaps/inspection/PNG alpha/budgets |

ทุก ADR เป็น proposed. Budgets/reference hardwareอยู่ใน verification planและ **ยังไม่ measured**; byte completeness/privacy/chronologyเป็น hard invariants ไม่ผ่อนเพื่อให้ timingผ่าน. First approved milestoneต้องแก้ observed history-order regressionและทำ source retention/safe import/revision foundation โดยไม่รอ numerical LTที่ยังไม่ผ่าน research gate
