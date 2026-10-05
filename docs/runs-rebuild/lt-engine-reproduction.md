# การทำซ้ำ numerical engine สำหรับ Runs

อัปเดต 2026-10-05: `running-dfa-open-1.0.0` เป็น experimental ventilatory proxy สำหรับวิ่ง ไม่ใช่ validated blood-lactate LT, Kubios-equivalent preprocessing หรือ automatic zone prescription หลักฐานทั้งหมดในเอกสารนี้เป็น **CC0 synthetic software fixtures** การเทียบ floating-point output ไม่ใช่ physiological validation ดู [method assessment](lt-method-assessment.md) และ [ADR 0005](../adr/0005-runs-threshold-methods.md)

## Engine และ frozen configuration

Public seams อยู่ใน `apps/api/src/runs/analysis.rs` และ `thresholds.rs`: `analyze`, `dfa_window`, `select_decline`, `estimate_history`, `configuration`, `export_projection` รายการพารามิเตอร์จริงพร้อม configuration SHA-256 อยู่ใน `trace.parameters` และ `method.configurationHash`

- Method `running-dfa-open-1.0.0`; workload segmentation `workload-observed-1.0.0`; numerical projection `runs-numerical-input-1.0.0`
- ใช้หนึ่งค่าต่อ recorded beat ไม่ resample; smoothness-priors λ=500 ด้วย `I + λ² D₂ᵀD₂` และ pentadiagonal Cholesky ที่ใช้ O(n) memory/work จากนั้น integrated centered residual profile
- Local linear detrending ที่ scales 4–16 beats, non-overlap, discard incomplete scale tail, pooled squared residuals แล้ว root, natural-log OLS สำหรับ alpha ไม่ใช้ randomized RANSAC
- 120-second centered windows, step 5 seconds, interval ends ใน `(start,end]`; ต้องมี full anchored boundary support, ≥32 beats, weighted observed sample HR coverage ≥95%, sample gap ≤5 s และ RR continuity error ≤0.005 s
- Native RR ต้องเป็น recorded source พร้อม HR message 132 timestamp field 253, fractional timestamp field 0, event counter field 9 และ absolute beat timestamp ที่สอดคล้อง session-relative time; ไม่ใช้ HRV message arrival เป็น anchor Test-only direct RR ต้องติด `softwareFixture: true` และ explicit synthetic beat bounds ไม่ใช่ production provenance
- Artifact policy เป็น open replacement: local median 11 beats, raw RR นอก 250–2000 ms หรือ deviation >20% เป็น annotated correction; linear time interpolation เฉพาะระหว่าง clean neighbors และไม่ข้าม source gap Raw RR ไม่ถูกแก้ ทุก window และ unique-beat union ของ regression portion ต้องมี corrected fraction ≤3% ไม่อ้างว่าเหมือน proprietary automatic correction
- Workload block 30 s; progressive speed slope ≥0.0003 m/s/s; steady speed CV ≤0.08; speed coverage ≥80%; relative surge/recovery step ±15%; repeated surges ต้องพบอย่างน้อยสองครั้ง ไม่มี HR stability filter ตัด drift ไม่มี power ก็ใช้ observed speed ได้
- Selector freeze ก่อนดู human holdout: contiguous chronological inner alpha `[0.5,1.0]` ≥3 windows และ directly adjacent boundary window ไม่เกินหนึ่งแต่ละด้าน Require negative slope, nonzero HR variance, observed .75/.50 bracket และ fitted crossing ภายใน selected observed HR range ไม่ extrapolate หลาย eligible sections ให้ `ambiguousCrossing`
- History เลือก latest eligible independent activity แยก target ภายใน 7 วันและไม่เกิน event cutoff ไม่มี pooled sensor calibration หรือ count overlapping windows เป็นหลายกิจกรรม Unknown RR sensor identity จำกัด evidence ไว้ที่ reference activity ของ snapshot ไม่มี numerical positive แต่มี actual evaluation ให้เก็บ failed windows/candidates/reasons จริง ไม่คืน placeholder registry trace
- Timer pause เป็น union ของ source timer events/sample state บน `[start,end)` Stop ที่ session-end ไม่มี pause ก่อนหน้า Recorded timer total ไม่ถูกแก้เมื่อไม่ตรง derived total; `timerTotalsDisagree` แจ้งความต่าง และ full RR window ที่ข้าม pause ใช้ไม่ได้

Unknown recovery, sleep, caffeine, meal timing, medication, treadmill calibration, health suitability และ RR sensor identity เป็น `contextUnverified` ไม่ใช้ user confirmation ปลดล็อกเลข Numerical positive ใช้ `low_confidence`, `engineStatus: experimental`, `researchBlocked: true`; empirical uncertainty interval เป็น `null` ไม่สร้าง person-level CI จาก overlapping-window regression

## Synthetic corpus และ independently frozen oracle

ไฟล์ใน `apps/api/tests/fixtures/` ใช้ prefix `runs-engine-`:

- `dfa-oracle.json`: 257-beat multisine `700 + 14 sin(0.173i) + 7 sin(1.37i) + 3 cos(2.51i)` ms Independent alpha **1.176678239780846** พร้อม F(4)…F(16)
- `progressive.json`: NumPy PCG64 seed **7429**, 480 s, 1,063 RR intervals และ 481 samples; `ρ(t)=0.97−1.57t/480`, `state=ρ×state+Normal(0,1)`, `RR=600−270t/480+3×state` ms; advance time ด้วย actual RR duration; HR `100+t/6`, speed `2.2+t/480`
- `progressive-positive.json`: distinct fixture ใช้ seed/ρ เดิมแต่ noise amplitude **6** ไม่แก้ amplitude-3 regression เพื่อให้ LT2 ผ่าน
- `progressive-oracle.json` และ `progressive-positive-oracle.json`: independent SciPy detrend/nolds DFA/NumPy OLS ของ raw RR และ FIT encoding quantization Native end-counter ticks เป็น `floor(t×1024+0.5)`, interval เป็น counter difference, recorded HR เป็น `floor(HR+0.5)` รวม interpolation/time-weighted window HR จริง ไม่ใส่ canned target ใน engine input
- `oracle.py`: standalone research-only comparator ไม่ import Rust และไม่เขียน golden จาก engine output

Native synthetic FIT ที่ decoder slice สร้างจาก corpus เดียวกันคือ `apps/api/tests/fixtures/runs/garmin_progressive_rr.fit` และ `progressive_rr_positive.fit` มี HR132 packed event timestamps และ absolute native timing ไม่ใช่ HR-derived RR

Independent numerical runtime ที่รัน: **Python 3.12.13**, **NumPy 2.2.6**, **SciPy 1.15.3**, **nolds 0.6.3** Comparator ตรวจ package versions และ SHA-256 ของ installed `nolds/measures.py`: `53a59a5c24d737539f584d9f86c09228852768308a0539b6cbd7d3ead3eae52a` ใช้ `scipy.linalg.solveh_banded` สร้าง SPD band system โดยอิสระ และ `nolds.dfa(nvals=4..16, overlap=False, order=1, fit_trend='poly', fit_exp='poly', debug_data=True)` Import เฉพาะ pinned measures module เพื่อไม่ดึง unrelated dataset resources จาก package initializer

[nolds versioned source](https://raw.githubusercontent.com/CSchoel/nolds/0.6.3/nolds/measures.py) ใช้ [MIT license](https://raw.githubusercontent.com/CSchoel/nolds/master/LICENSE.txt); comparator import installed package ไม่ vendor proprietary code ใช้ synthetic data ที่ระบุ CC0 ใน fixture manifests ไม่ใช้ private FIT หรือ corpus ที่ไม่มี reuse license

Frozen software tolerances: absolute alpha **1e−9**, F(n) **1e−8**, fitted crossing HR **1e−8 bpm** ตัวเลขนี้เป็น numerical agreement tolerance ไม่ใช่ physiological accuracy หรือ uncertainty ห้าม relax selector/noExtrapolation เพื่อผ่าน tolerance

## คำสั่ง scoped reproduction

จาก repository root:

```sh
cargo test -p garmin-fit-extractor-api --test runs_engine
cargo check -p garmin-fit-extractor-api --test runs_engine
python3 apps/api/tests/fixtures/runs-engine-oracle.py
```

Python command ต้องใช้ runtime/dependencies ที่ pin ข้างบน Research dependencies ไม่ถูกเพิ่มใน application manifests Rust tests ใช้ public pure-engine seams และ actual generated RR ไม่ใช่ mocked target scalar Native FIT decoder smoke เป็นหลักฐานแยกด้านล่าง ไม่อ้างว่า `runs_engine` test เพียงชุดเดียวเปิด native FIT bytes

## ผลจาก actual native decoder smoke

รัน public `decode_run` บน synthetic FIT ทั้งสอง แล้วส่ง actual normalized output เข้า engine เปรียบเทียบทุก window F(n), alpha, candidate selection และ crossing กับ encoded oracle โดยไม่ inject expected output:

- Original amplitude 3: actual LT1 **134.71544856754312 bpm**; LT2 **null / noExtrapolation** Actual rejected .50 crossing **157.41934068501405** เกิน selected observed HR maximum **156.75** Maximum absolute alpha disagreement **4.9934056889355816e−11**, F disagreement **4.73042938153867e−10**
- Distinct amplitude 6: actual LT1 **136.41887336482208 bpm**, LT2 **160.4210626128638 bpm** Independent expected **136.41887336477964 / 160.42106261369022** Maximum absolute alpha disagreement **2.4642399232277512e−11**, F disagreement **5.050644347193156e−10**

Public history smoke รักษา actual rejected LT2 trace/candidate ของ original FIT และ numerical positive ของ distinct FIT Regression tests ครอบคลุม artifact fractions 3/4/5/6%, constant HR/RR, gaps/pauses, noExtrapolation, ambiguity, late strides, future/deleted/revision-mutated evidence, older independently eligible target, conflicting target order และ condensed verified projection receipt โดยไม่ clamp หรือเติม LT จาก ratio

Timer seam regression ที่ stop 450 s โดยไม่มี sample timer flags ให้ observed pause **30 s**, recorded timer **480 s**, derived timer **450 s** และแจ้ง `timerTotalsDisagree` Original fixture stop 480 s จึงมี pause **0 s** ไม่แอบเปลี่ยน recorded summary ให้ตรง derived result

## Immutable projection และ reproducibility boundaries

Numerical projection เก็บ numeric samples/RR/provenance/laps/timer/sensors ทุกแถว ไม่มี fixed row cap, downsample หรือ truncation Full normalized/decoded archive ยังรักษาข้อมูลอื่นทั้งหมด การตัด repeated references/extensions ที่ engine ไม่ใช้เป็น internal projection ไม่ใช่ full-data export policy

`trace.inputHash` เป็น SHA-256 จาก `serde_json::to_writer` ของ **actual numerical input Value** แบบ streaming ลง SHA writer ไม่สร้าง whole-input string `inputRevision` คือ source normalized revision; `inputNormalizedHash` คือ full normalized archive hash จาก core; `inputProjectionVersion` ระบุ projection format Hash ทั้งสองไม่ถูกเรียกแทนกัน Full-input evidence ถูก hash ใหม่จริง ส่วน condensed history ที่ไม่มี samples ต้องมี `evidenceVerification` จาก authenticated core เท่านั้น:

```json
{
  "kind": "immutableNumericalProjection",
  "revisionId": "the-verified-normalized-revision-id",
  "inputHash": "the-verified-64-lowercase-hex-projection-digest",
  "projectionVersion": "runs-numerical-input-1.0.0"
}
```

Receipt นี้เป็น internal contract ไม่ใช่ตัวอย่าง proof ที่ส่งจาก public client Core ต้องตรวจ immutable stored manifest, revision และ digest ก่อนสร้าง receipt Engine ยังตรวจ method/config, exact revision/hash, event cutoff, source availability และ recency หาก source มี samples ไม่เชื่อ receipt แทน actual digest Export ใช้ closed aggregate allowlist ไม่ส่ง source IDs/hashes/revisions, unselected windows หรือ last-good payload

## Release blockers ที่ยังคงอยู่

ยังไม่มี licensed paired human **running RR + independent gas-exchange VT1/VT2 หรือ lactate-defined targets** จึงยังไม่ปิด human reference agreement, authorized preprocessing/Kubios comparison, empirical person-level uncertainty/sample-size plan หรือ independent physiological release gate ต้อง preregister population/sensor/protocol/selector/reference-repeat agreement ก่อน human holdout ไม่ตั้ง sample size/CI/LOA ที่ไม่มีข้อมูลรองรับ ไม่อ้าง validated lactate LT, clinical/training safety หรือ auto zone prescriptionจาก synthetic agreement
