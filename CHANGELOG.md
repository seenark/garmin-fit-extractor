# Changelog

## Unreleased — Runs cutover integration

This records integration-branch changes, not a production release. Decoder,
analysis, and web changes are merged. Core persistence/API changes remain on
their unmerged implementation branch. Final combined runtime, browser,
recovery, and full-suite evidence is outstanding. No deployment, Runs reset,
or legacy migration apply is claimed.

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

### Core implementation pending merge

The Core branch implements immutable original FIT storage, stable owner-scoped
IDs/deduplication, coherent revision manifests, durable bounded jobs, reprocess
and version scanning, event-time evidence invalidation, permanent deletion,
and authenticated pinned Coach/Full exports. Export defaults omit location and
device identifiers independently; unknown fields remain omitted even with both
opt-ins. Snapshots expire after 15 minutes and deletion revokes them.

The branch also retires obsolete extraction APIs without an unsafe download
fallback, preserves source-less legacy summaries without inventing FIT, and
keeps FIT Coach's v1 OAuth activity projection separate. This paragraph is an
implementation inventory, not evidence of merged or exercised behavior.

### Remaining release limits

- Paired licensed human running RR and independent gas-exchange references,
  participant-level holdout validation, and empirical uncertainty are missing
  for validated VT claims. Lactate claims additionally require paired running
  lactate references. Synthetic numerical agreement does not close these gates.
- Quantified workload-detector accuracy needs independent human annotations.
- Actual production hardware/storage budgets, physical mobile/browser support,
  provider login, backup policy, and deployment authorization remain separate
  from disposable integration verification.
