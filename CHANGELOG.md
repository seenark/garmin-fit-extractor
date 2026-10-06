# Changelog

## Unreleased — Runs cutover integration

บันทึกนี้เป็นงาน integration branch ไม่ใช่ production release Unchanged check/test/build
ล่าสุดผ่าน 643.58 s: Bun 109, CLI TAP 8 และ Rust 221 ใน 25 groups, 0 fail,
3 existing ignored ตามเดิม; unchanged e2e ผ่าน 90.36 s โดย failed tests เป็น `[]`
Current protected same-PG/API restart ผ่าน 19.02 s และ R5/R9 ผ่านทั้งเจ็ด gates ใน 337.05 s
รวม actual encrypted PostgreSQL restore/native re-decode Current isolated faults ผ่าน 104.71 s
Accepted browser 51/native Full 100k และ natural/worker/decode proofs คง pre-cancel identities
Current-source Full API runtime ผ่าน 141.81 s กับ fresh809,192,713-byte pin/frozen native100k
oracle/parent-memory observations ไม่ relabel pre-cancel browser artifact เป็น current binary
AT-34 exact-owned clone ผ่าน 27 cases/1235.26 s และ independent review correct0.97/findings[]
Independent current-Full review correct0.96/findings[] ยืนยัน actual scoped proof
สถานะและ final Git/workspace integration receipts ติดตามใน external evidence index;
software/source evidence ไม่แทน independently licensed physiological reference gates
เก็บ failed/inconclusive/timeout attempts เดิม ไม่อ้าง production หรือ whole-tree source hash

### Merged decoder and analysis changes

- Add a profile-verified Garmin-running decoder with CRC, manufacturer, and
  single-running-session checks and explicit unsupported reasons.
- Separate decoded archive and normalized schema 2.0.0. Retain ordered FIT
  identities, validity, supported native/developer values, provenance,
  full-resolution samples, recorded laps, timer events, and genuine recorded RR.
  Unknown metrics stay null; unsupported RR timing stays unanchored.
- Stream decoder JSON to a writer instead of returning a whole-archive runtime
  JSON value. Vendor MIT fitparser fixes for header CRC handling, packed
  components, accumulation, numeric metadata, and field preservation.
- Add observed-workload segmentation, quality/rejection reasons, immutable
  numerical-input projections, and independent experimental DFA-a1 VT1/VT2
  proxies under LT1/LT2 labels. Preserve device-reported thresholds separately.
- Record method versions, actual numerical traces, event-time historical
  cutoffs, seven-day recency, distinct observation groups, and target-specific
  abstention. Version the open selector adaptation; do not infer maximal intent
  or extrapolate beyond observed evidence.
- Add licensed synthetic Garmin/RR fixtures, independent numerical comparator
  artifacts, and capacity generators with manifests. These do not establish
  physiological validity or production performance.

### Merged web changes

- Move authenticated upload/history/detail consumers to Runs v2 while retaining
  existing web route paths. Accept direct FIT or ZIP and show per-item outcomes,
  processing/freshness, legacy source-unavailable state, and experimental labels.
- Preserve explicit selection across pagination, filtering, and ordering;
  separate Coach and Full Copy/Download actions with independent privacy opt-ins.
  Clipboard reads exact pinned server text. Download delegates the authenticated
  snapshot URL to the native browser rather than materializing the full file
  in JavaScript. Failure messages do not claim a completed save.
- Add linked Pace/HR/Power inspection and zoom, recorded-lap and detected-segment
  overlays, gap handling, and disclosed display-only sampling.
- Add numeric Canvas PNG cards with three layouts, square/portrait sizes,
  Light/Dark/Transparent themes, and optional local raster photos. Keep photos
  off the API and original EXIF out of the generated PNG.
- Provide editable ChatGPT/Claude prompts copied separately from activity JSON
  and PNG. No runtime LLM request or AI API key is added.
- Read native numeric timer event enums for pause boundaries. Keep duplicate
  timestamps individually inspectable by stable native sample index, including
  keyboard and pointer selection. Allow each dismissed LT target suggestion to
  reopen independently through an accessible control.

### Root integration changes

- Include vendored decoder dependencies and migrator source in Docker's locked
  Rust build stage; retain the existing single non-root API/web runtime image.
- Guard both root test modes with explicit disposable `TEST_DATABASE_URL`,
  exact server `PGDATA`, and PostgreSQL 18 checks before build/startup/migrations.
  Build a fresh native API and use Cargo's reported executable for decoder-child
  tests; do not fall back to an ambient application database or old binary.
- Disable Bun dotenv loading in test/build/check entrypoints and document the
  required outer `bun --no-env-file`. Keep Docker test storage uniquely owned.
- Align README and product guidance with selected privacy-filtered exports,
  immutable sources/revisions, experimental science limits, and protected
  CLI, FIT Coach OAuth, Shoes, Google sessions, and transcript administration.
- Add standalone operator-only `runs-reset` with read-only default planning,
  explicit target/scope fingerprints, restricted backup binding, and destructive
  apply confirmations. It is never invoked by startup, migration, or deployment.
  Actual exact-owned clone ผ่าน27 casesตามหลักฐานด้านบน; production applyยัง unauthorized
- Document native privacy/type fidelity with MIT fitparser 0.11.0's generated
  profile 21.202.0 callbacks as the sole wire/scaling/component/enum authority.
  Supplemental shape facts are only 212 `array: true` declarations from
  [first-party `garmin_fit_sdk/profile.py`](https://github.com/garmin/fit-python-sdk/blob/06bd8910bcf84d832d6cc98071a467f5a13e2339/garmin_fit_sdk/profile.py),
  tag `21.202.0`, header `Profile Version = 21.202.0Release`, commit
  `06bd8910bcf84d832d6cc98071a467f5a13e2339`, source SHA-256
  `43168b75f7db62c3f16dfd68fbf54722048eeedf6d4fbfa21fd50affaaff9eb4`.
  Boolean facts supply no fixed/dynamic element counts and no SDK parity claim.
  The in-code guard uses 73 flags for 18 callback-covered global messages, not
  blanket support for all 212 source declarations.
- Require native primitive/wire/shape/semantic checks before privacy export;
  unknown, undeclared, malformed, and developer-context-colliding fields fail
  closed even with opt-ins. Allowed arrays retain every valid source position,
  null/zero, raw value, and source reference without caps. Preserve actual
  scale/offset and Event subfield meaning instead of trusting decoded labels.
  Keep earlier native privacy/API proofs distinct from the accepted pre-cancel
  19/19 API, native Full, and completed four-consent UI privacy proofs. Preserve
  their executable identities; no current cancellation-ELF browser rerun is
  claimed. No restricted SDK runtime, processor,
  bridge, library use, benchmark, or comparative evaluation is claimed.
- Gate Full normalized and Coach fields with bounded compact proof from the same
  already-stored decoded revision, once per selected activity. Reject wrong-wire
  values hidden by older canonical numeric caches; remove optional unprovable
  values, matching references, and dependent derived permissions. Required
  unprovable values fail atomically with `422 RUNS_EXPORT_SOURCE_UNPROVABLE`.
  Require stored profile version `21.202.0` and exact chosen MIT profile/vendor
  source receipts before canonical authorization; missing/mismatched receipts
  fail with `422 RUNS_EXPORT_SOURCE_UNPROVABLE` before publication.
  Valid older data from that producer remains eligible despite different
  raw/normalizer/archive hashes, without forced reupload/reprocess; this is not
  blanket acceptance of older profiles.
- Stamp successful stored snapshots with internal policy
  `native-source-proof-v1`, without adding a public envelope field. Missing or
  old stamps fail GET/HEAD with `404 EXPORT_UNAVAILABLE`; transport checks abort
  incompatible admitted bodies. Never rewrite immutable snapshot bytes.
- Preserve storage-read failure categories: actual database/connection failures
  return `500 RUNS_STORAGE_FAILED`, stored-document integrity failures return
  `500 RUNS_DOCUMENT_INTEGRITY_FAILED`, and malformed/schema/input-budget failures
  remain semantic. An already-started stream fails rather than cleanly ending
  a shortened download. Earlier full API proof remains pre-grouping-fix;
  ต่อมา pre-cancel API proof ผ่าน19/19; current cancellation/runtime proofs แยก scopeด้านบน
- Preserve original typed visitor errors in the shared streaming scanner.
  A real PostgreSQL export-chunk INSERT failure inside a Coach callback remains
  HTTP 500 rather than becoming an invalid-document HTTP 422; no snapshot
  publishes, and a healthy later export recovers.
- Preserve explicit null native developer identity through Full export instead
  of mistaking a missing identity for developer data. Non-null developer
  identities and unknown developer fields still fail closed. Verify real-native
  values, raw values, source references and privacy under all four consents.
- Make export regression guards use explicit absolute `PGDATA` matched against
  the actual PostgreSQL 18 server and a loopback `_test`/`_rehearsal` URL, rather
  than requiring a disposable directory to live beneath `/tmp`. Keep the full
  normal test commands and privacy assertions; do not ignore failing tests.
- Match legacy import/verify to the native Runs activity schema. Keep same-owner
  legacy extraction checks and prove genuine native source/manifest integrity
  without manufacturing extractions or weakening protected-table fingerprints.
  The destructive regression fixture requires an explicit disposable `_test`
  or `_rehearsal` URL, matching actual PGDATA, PostgreSQL 18, and a fresh native
  decoder executable.
- Limit transport-reader death recovery to the same boot and PID namespace.
  Foreign scopes remain fenced without trusted supervisor death proof; no
  cross-host or replacement-container recovery is claimed.
- Correct observation identity in the shared stream producer: identical native
  records under 14-byte and 16-byte FIT headers differ only in summary
  `sourceReferences` byte offsets, but hashing the whole summary incorrectly
  separates their observation groups. The implemented fix excludes only those
  provenance references from the hash and restores the exact native references
  without cloning or rewriting immutable revisions.
- Bump only observation rule `exact-stream-v2` to `exact-stream-v3`, through
  the shared constant/hash domain, stored metadata and desired versions.
  Existing version scanning forces native processing; initial store and atomic
  publication update current fingerprints/groups and three-peer evidence;
  current history is rule-gated. Already-stored duplicate groups must not mix
  old and corrected identity even before release. Decoder/profile, normalizer,
  source authority, scientific method, schema and projection versions stay unchanged.
- Retain actual fail-before/pass-after native header and three-peer regressions,
  plus actual v2-to-v3 native HTTP reprocessing of two genuine immutable sources.
  Source IDs/bytes/summary references and old v2 metadata remain unchanged;
  current v3 fingerprints/groups agree. The same old session and old Full
  pinned bytes remain available after restart and native reprocessing.
  This proof predates the final physical-capacity and new-export/history fixes.
- แก้ expiry-only admission ด้วย kernel-backed physical completion guard ครบสาม stages,
  native CPU/thread lifetime และ inherited child FD; decode callers ทั้ง 13 จุดรับ actual
  parent admission Genuine before-test fail 3.72 s และ after-test pass 0.09 s
  (รวม build 28.12 s) มี source table และ migration `0008` รองรับงานนี้
  ไม่เปลี่ยน method, decoded/normalized schema, caps, TTL หรือ slot count
- Correct new-export/list/detail observation-rule precedence: an old manifest
  remains pending/stale even with failed history; new Full/Coach creation rejects
  it while old pinned GET/HEAD bytes stay unchanged. The genuine archived-native
  regression passes in 1.34 s, also preserving current-rule failed/ready
  precedence and source/profile/method receipts. Final scoped runtime proofs are recorded above.
- Add build-script migration-directory tracking for both API migrations and
  the legacy migrator's shared API migration directory, with matching Docker
  build copies. An earlier unchanged full run fails `MigrationFailed` because
  PostgreSQL has migration 8 but cached legacy SQLx metadata contains only 1–7.
  After both directory trackers and Docker copies, actual native legacy scoped
  proof passes in 3.72 s and the unchanged full normal run passes. Earlier
  missing `RUNS_DECODER_EXECUTABLE` is a scoped setup issue, not a product error.
  CLI errors/schema checks/assertions remain unchanged.
- Retain actual R8 Linux native faults/recovery and automatic global admission
  proof: all 11 gates pass with 79 queued jobs, real-time overlap, live leases
  and kernel hard caps on the pre-grouping-fix binary. Keep the older manual
  non-overlapping attempt explicitly inconclusive. Historical source fingerprint
  `74a0e5da68f0f6fb2107b426c6167daa6df46bae4f0bf5f747c441d0746e05e0`
  covers only its 204 enumerated files, not the current tree or Docker context.
- Retain the accepted pre-cancel debug 100k timeout with stale old revision
  and disabled new export preparation. Source-identical optimized native
  release reprocessing succeeds at generation 12/rule 3 without cap/TTL changes.
  Accepted pre-cancel Full Download has 809,192,701 bytes and SHA-256
  `427788e4550961fae93ecde80e7bf1163d589fbbc7952864fe0b2ed1ec97392c`;
  independent native verification checks all 100k samples, values, raw fields,
  source references, missingness and default privacy. Full Copy exposes a
  truthful native-download fallback, not an inferred underlying exception.
  Coach Copy succeeds and its native Download matches the same pinned bytes.
- Correct source-receipt boundaries: historical 204 includes 13 web tests and
  the vendor component test, but excludes API/legacy tests, docs, FIT, DB,
  generated/private environment files. Selected 207 และ E9d selected208 เป็นประวัติ;
  selected208 ล่าสุดมี SHA-256
  `7a9e7440254489ffae91b3ea059cdff080d456b30ad87694f34e6a3cf873c238`
  ไม่ใช่ whole-tree, Docker context, private environment, PostgreSQL หรือ generated identity
- จำกัด builder cancellation mapping ให้ `404 EXPORT_UNAVAILABLE` เฉพาะ watch failure
  ที่ independent affected-tombstone query ได้ `Ok(true)` ไม่ remap PG/serialization/storage
  errors แบบเหมารวม ไม่เพิ่ม positive-path query; `JoinError` คง 500
  Actual no-tombstone materialization callback failure ยังเป็น `500 RUNS_STORAGE_FAILED`
  และ corrupt GET ยังเป็น 500 R5/R9 current proof ครบเจ็ด gates รวม rollback/no pin,
  transport revocation, no rebirth, actual encrypted restore และ offline corrupt-copy refusal
  Transport guard ใช้ Arc/Weak กับ token `activeReads`/`readProcesses`; SQL shared-owner lock
  commit ก่อน chunk transport จึงไม่อ้างว่าถือ ShareLock ตลอด stream
- AT-34 exact-owned clone ผ่านครบ 27 cases: 24 retained negative cases และสาม actual
  confirmed applies ใน 1235.26 s Original source 200 rows, Shoes, schema, protected state,
  FK และ source bytes ไม่เปลี่ยน Clone owner-native 2/all-native 198/running-legacy หนึ่งคู่
  ถูกลบตาม scope ที่ยืนยัน โดยคง other-owner/legacy/cycling/NULL rows ตามแต่ละ stage
  Final source dump/encrypted backup ใหม่ verified แต่ไม่ physically restore dump ใหม่นี้
  Prior real clone restore/83 unique native re-decodes/121 declared hashes คง receipt เดิม
  Failed/timeout attempts เดิมยังคงอยู่ ไม่มี Drop/Truncate/source reset หรือ product-cap change
- Current Full pin `f2597886-e52e-40a2-a1fd-e467fd0394fd`/SHA-256
  `60e45793b11cf6d62a0940945676e1bc6d7344a3ca5eb0e858dd8651cb85f31c`
  ผ่าน streamed GET/no truncation/default privacy และ frozen native100k oracle
  API parent sampled VmRSS peak/kernel lifetime VmHWM 116,195,328 bytes; ไม่ใช่ isolated-run
  peak หรือ 512 MiB/60 s parent bound ไม่อ้าง UI/new decode/global rerun

### Merged Core implementation

Core implements immutable original FIT storage, stable owner-scoped
IDs/deduplication, coherent revision manifests, durable bounded jobs, reprocess
and version scanning, event-time evidence invalidation, permanent deletion,
and authenticated pinned Coach/Full exports. Export defaults omit location and
device identifiers independently; unknown fields remain omitted even with both
opt-ins. Snapshots expire after 15 minutes and deletion revokes them.

Core also retires obsolete extraction APIs without an unsafe download
fallback, preserves source-less legacy summaries without inventing FIT, and
keeps FIT Coach's v1 OAuth activity projection separate. This paragraph is a
merged implementation inventory, not evidence of exercised behavior.

- Keep decoder cancellation in the child-owning scope and await kill/reaping
  before releasing its workspace or capacity; drain non-abortable worker CPU
  work when job/global-slot authority is lost.
- Complete historical jobs in their publication transaction with final live
  lease, generation, current-manifest, evidence-manifest, and tombstone fences.
  Accepted pre-cancel natural/worker/decode global overlap receipts retain
  their original executable identities. Current Linux09f isolated fault/native
  recovery and Darwin074 protected/R5/R9 recovery proofs are complete. No
  current09f global overlap rerun is claimed.

### Remaining release limits

- Paired licensed human running RR and independent gas-exchange references,
  participant-level holdout validation, and empirical uncertainty are missing
  for validated VT claims. Lactate claims additionally require paired running
  lactate references. Synthetic numerical agreement does not close these gates.
- Quantified workload-detector accuracy needs independent human annotations.
- Actual production hardware/storage budgets, physical mobile/browser support,
  provider login, backup policy, and deployment authorization remain separate
  from disposable integration verification.
