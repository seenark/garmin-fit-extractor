# การประเมินวิธีประมาณ LT1 และ LT2 สำหรับ Runs

สถานะอัปเดต 2026-10-05: **experimental numerical implementation** มี scoped regression tests และ actual native synthetic FIT→RR→positive LT1/LT2 เทียบ independent SciPy/nolds แล้ว ยังไม่มี physiological validation, licensed paired human reference corpus หรือ Kubios equivalence รายละเอียดและคำสั่งอยู่ใน [reproduction record](lt-engine-reproduction.md) ข้อความที่ระบุ “รอบ research 2026-10-04” ด้านล่างเป็นบันทึกก่อน implementation ไม่ใช่สถานะ engine ปัจจุบัน

## 1. ข้อเสนอที่เลือก

ใช้ numerical research candidates สองตัวแยกกัน: `running-dfa-a1-075` สำหรับ **HRVT1 ที่เป็น VT1 proxy** และ `running-dfa-a1-050` สำหรับ **HRVT2 ที่เป็น VT2 proxy** ทั้งสองต้องใช้ beat-to-beat RR จริงระหว่างวิ่ง ไม่ใช่ RR ที่สร้างจาก sampled HR หรือ resting HRV วิธีนี้มี primary running validation จริง จึงไม่ถูกต้องที่จะสรุปว่าไม่มีวิธีใดทำได้จาก FIT โดยไม่ตรวจ RR และ protocol ก่อน [S1–S4]

ยังไม่เปิด validated automatic LT1/LT2 ใน release ทั่วไป เลือกสถานะ experimental แบบมีตัวเลขได้เฉพาะเมื่อ numerical, input provenance และ protocol gates ผ่าน ค่าเหล่านี้ต้องระบุ target เป็น ventilatory proxy ไม่ใช่ blood-lactate threshold ที่วัดโดยตรง **ยังไม่ถือว่าฟีเจอร์ LT estimation สมบูรณ์จน engine มี positive path ที่รันจาก exercise RR และผ่าน gates ด้านล่าง** การ import/export FIT ไม่ต้องรอ physiological release gate

แยกข้อเสนอแต่ละ target:

- LT1: เลือก DFA-a1 0.75 เป็น candidate แรก หลักฐาน treadmill running และ consumer chest belt รองรับ VT1 proxy แต่ individual error และ fatigue มีนัยสำคัญ [S1, S3] การเปลี่ยนชื่อเป็น lactate-defined LT1 ต้องผ่าน paired running lactate validation เพิ่ม ไม่ใช้ cycling evidence แทน
- LT2: เลือก DFA-a1 0.50 เป็น candidate วิจัยสำหรับ VT2 proxy ไม่เปิดเป็นคำแนะนำ training zone อัตโนมัติ หลักฐานตัวแรกมี limits of agreement กว้าง และ trial หลังพบ premature exhaustion ใน 6/21 คนเมื่อใช้ running speed ที่ได้จากวิธีนี้ [S2, S4]
- LT2 ทางเลือก: เก็บ `30-minute-time-trial-OBLA4-proxy` ไว้เป็น deferred candidate ไม่เลือกช่วงเร็วที่สุดย้อนหลังเป็น TT และไม่เสนอให้ทุกคนทำ all-out test [S6]
- ไม่รองรับการอนุมาน LT1 จาก LT2 ด้วย fixed ratio, HRmax percentage, Garmin zone, การใช้ drift 5% เป็นนิยาม threshold, หรือ running power ที่สมมติว่าเป็น metabolic truth เหตุผลของ drift มีหลักฐานทดลอง heat stress และ threshold targets แตกต่างกันจริง [S5, S7, S8]
- Device-reported threshold ยังคงเป็น recorded device value แยกจาก estimated value ไม่ใช้เป็น reference truth เพื่อรับรองวิธีที่อาจมาจาก algorithm เดียวกัน

## 2. Target ต้องไม่ปะปนกัน

`LT1`, `VT1`, `LT2`, `VT2`, `OBLA4`, `MLSS` และ performance proxy ไม่ใช่ label ที่สลับแทนกันโดยไม่มีเงื่อนไข งาน primary ใช้ reference ต่างกัน และได้ agreement ต่างกัน [S2, S5, S6, S8]

ข้อเสนอ schema ของผลแต่ละตัวต้องมี `productSlot` (`LT1` หรือ `LT2`), `targetDefinition`, `referenceDefinition`, `sport`, `protocolClass`, `methodVersion`, `configurationHash` และ `evidenceCutoff` เช่น slot LT1 สามารถแสดง “ประมาณขอบเขตแรก: VT1 proxy จาก exercise RR; ยังไม่ยืนยัน blood-lactate LT1” การมี slot ชื่อ LT1 ไม่อนุญาตให้ลบคำว่า proxy

กำหนด target สำหรับ development และ validation ล่วงหน้า:

1. Candidate แรกของ LT1 เทียบ HR ที่ VT1 จาก gas exchange แบบ V-slope ร่วมกับ ventilatory equivalents/end-tidal criteria โดยผู้ประเมินอิสระสองคน ไม่เลือก criterion ที่ให้ agreement สูงสุดหลังเห็นผล DFA
2. Candidate แรกของ LT2 เทียบ HR ที่ VT2/RCP จาก disproportionate VE/VCO2 rise, VE/VCO2 equivalent rise และ PETCO2 fall ไม่เทียบกับ OBLA4 หรือ MLSS แล้วเรียกรวมว่า target เดียวกัน [S4]
3. หากจะเปิด lactate claim ให้ทำ running stage protocol พร้อม lactate samples และระบุสูตร LT1 หรือ LT2 ที่เลือกอย่างชัดเจนก่อนวิเคราะห์ การกำหนด fixed 4 mmol/L เป็น OBLA4 ไม่ใช่การยืนยัน individual MLSS [S6, S8]

## 3. Primary evidence: LT1

### S1 — Rogers et al., 2021: treadmill DFA-a1 0.75 เทียบ VT1

[Full text: A New Detection Method Defining the Aerobic Threshold…](https://pmc.ncbi.nlm.nih.gov/articles/PMC7845545/), DOI [10.3389/fphys.2020.596567](https://doi.org/10.3389/fphys.2020.596567), PMID 33519504

- ผู้ชาย recreational runners 17 คน อายุ 19–52 ปี; วิเคราะห์ 15 คนหลังตัด excessive atrial ectopy สองคน ไม่ใช่ mixed-sex representative Garmin cohort
- Bruce treadmill protocol: เริ่ม 2.7 km/h, grade 10%; เพิ่ม speed 1.3 km/h และ grade 2% ทุก 3 นาทีจน volitional exhaustion มีพัดลม ไม่ใช่ flat outdoor progressive run
- Reference เป็น VT1 gas exchange; ตรวจหลาย criterion แล้วใช้ excess CO2 เพราะคุณภาพ plot และมีผู้ตรวจสองคน ไม่ได้เจาะ lactate
- RR มาจาก 3-lead Biopac ECG 1000 Hz; Kubios **3.3.2**, smoothness-priors detrending λ=500, automatic artifact correction, DFA scales 4–16 beats; 120-second centered window ทุก 5 วินาที ทำ linear regression เฉพาะ rapid decline จาก alpha ใกล้ 1.0 ถึงประมาณ 0.5; หา HR ที่ alpha=0.75
- Artifact ใน regression portion 0–3%; visual inspection ตรวจ ectopy/noise ไม่ใช่ตรวจแค่ค่า HR ต่อวินาที
- Published HR Bland–Altman difference **VT1 minus HRVT**: bias −1.9 bpm, SD ประมาณ 5 bpm, LOA **−12 ถึง +8 bpm**; HR correlation r=0.97 ไม่ได้หมายความว่าทุกคนคลาดเคลื่อนน้อย Table 2 มีตัวอย่าง 108 เทียบ 122 bpm
- การเลือก linear section ยังมีผู้ประเมิน ไม่ใช่ deterministic automated selector ที่พร้อมนำไป production ข้อมูล raw ระบุ available from authors ไม่ใช่ corpus RR ที่ดาวน์โหลดได้จากบทความ

**ข้อสรุป:** มี real running candidate สำหรับ first ventilatory boundary; ไม่ยืนยัน lactate LT1 หรือความแม่นยำของ automatic selector ของเรา

### S3 — Van Hooren et al., online 2023 / issue 2025: consumer chest belt และ fatigue

[Primary abstract](https://pubmed.ncbi.nlm.nih.gov/37916488/), DOI [10.1080/02640414.2023.2277034](https://doi.org/10.1080/02640414.2023.2277034); [author institution record](https://cris.maastrichtuniversity.nl/en/publications/correlation-properties-of-heart-rate-variability-to-assess-the-fi/)

- 14 คน ทดสอบ treadmill สอง incremental ramps ใช้ consumer chest belt; reference เป็น VT1 gas exchange ไม่ใช่ lactate
- First ramp: HR bias **−0.9 bpm**, 95% LOA **−12.2 ถึง +10.5 bpm**, R²=0.56; VO2 bias −0.5 mL/kg/min, LOA −6.8 ถึง +5.8
- Second ramp: HR bias **−12.3 bpm**, LOA **−30.4 ถึง +5.9 bpm**; VO2 bias −7.3, LOA −18.1 ถึง +3.5 ความสัมพันธ์เปลี่ยนภายใต้ acute fatigue
- บทความระบุว่าการแบ่ง threshold ต้องใช้ non-fatigued state จึงห้ามถือ alpha crossing ตอนท้าย long run หรือหลัง intervals เป็น fresh LT1 โดยอัตโนมัติ
- Publisher full HTML และ PDF ตอบ HTTP 403 ในการอ่านครั้งนี้ Institution ระบุ CC BY แต่ไม่มี accessible full-text file ที่อ่านได้ จึงใช้เฉพาะข้อมูล abstract ไม่เติม sensor model, sex count หรือ algorithm settings จากการคาดเดา

**ข้อสรุป:** เพิ่มความเป็นไปได้ของ chest-belt running input แต่ FIT ตรวจความสด การนอน ยา คาเฟอีน และความพร้อมไม่ได้ ต้องมี `contextUnverified` และไม่สร้าง confidence สูงจาก RR quality เพียงอย่างเดียว

### S5 — supplied source: Rogers, Berk, Gronwald, 2022

[Supplied PubMed](https://pubmed.ncbi.nlm.nih.gov/35202064/), [full text](https://pmc.ncbi.nlm.nih.gov/articles/PMC8875480/), DOI [10.3390/sports10020025](https://doi.org/10.3390/sports10020025)

- Elite triathletes 9 คน (7 ชาย, 2 หญิง), **cycling**; Cyclus2 จน exhaustion ชายเริ่ม 90 W เพิ่ม 30 W/3 min; หญิงเริ่ม 75 W เพิ่ม 25 W/3 min
- Reference เป็น **LT1 log–log lactate versus power/HR** ไม่ใช่ VT1 และไม่ใช่ fixed 2 mmol/L เก็บ blood lactate ก่อนจบ stage 30–40 วินาที
- Polar H10 7 คน, Pioneer 1, Garmin 1; Garmin Edge 530 บันทึก RR; Kubios **3.4.3**, λ=500, automatic correction, scales 4–16, window 120 s/step 5 s, artifacts <5%; HRVT ที่ alpha=0.75
- Reported mean difference −1.7 bpm; LOA **−16.7 ถึง +13.3 bpm**; cycling power bias −5.3 W, LOA −25.6 ถึง +15.1 W; HR ICC 0.69 (95% CI 0.14–0.92), CCC 0.66 ความแม่นยำรายคนไม่ใช่ “ใกล้กันแน่นอน”
- Abstract mean HR difference (153.7−155.8=−2.1) ไม่ตรง bias ที่รายงาน −1.7 พอดี เก็บ published numbers แยก ไม่แก้หรือนำ rounded means มาแทน paired analysis
- Authors ระบุ raw data available on request ไม่มี public RR corpus ในบทความ

**ข้อสรุป:** สนับสนุน lactate LT1 association ใน cycling และการบันทึก exercise RR ลง Garmin FIT แต่ไม่ใช่ running-lactate validation

### S8 — Sempere-Ruiz et al., 2024: independent cycling reliability/validity

[Full primary text](https://www.frontiersin.org/journals/physiology/articles/10.3389/fphys.2024.1329360/full), DOI [10.3389/fphys.2024.1329360](https://doi.org/10.3389/fphys.2024.1329360)

- Untrained healthy adults 16 คน (13 ชาย, 3 หญิง); session 2 วิเคราะห์ 15 คน Cycling 3-minute stages, individualized 15/20/25 W increments, two tests ห่าง 6–9 วัน ECG 1000 Hz; Kubios **4.0.1**, λ=500, automatic correction, artifacts ไม่เกิน 5%, scales/window เหมือนข้างต้น
- Reference แยก VT1, LA2.0, LA2.5, baseline+1.0; HRVT1 HR เทียบ VT1 มี **bias +28.3 bpm, SD 17.4 bpm, ICC 0.31** เทียบ LA2.5 bias +8.4, SD 11.6 ผล power ที่สัมพันธ์สูงใช้ยืนยัน HR running ไม่ได้
- HRVT1 test–retest typical error **8.83 bpm** และ ICC 0.52; HRVT2 typical error 4.08 bpm อย่าแปล power reproducibility เป็น HR reproducibility

**ข้อสรุป:** Target/population/protocol matters. ไม่ยอมรับ claim ว่า 0.75 คือ LT1/VT1 สากล

## 4. Primary evidence: LT2

### S2 — Rogers et al., 2021: treadmill DFA-a1 0.50 เทียบ VT2

[Full text](https://pmc.ncbi.nlm.nih.gov/articles/PMC8167649/), DOI [10.3390/jfmk6020038](https://doi.org/10.3390/jfmk6020038), PMID 33925974

- Recreational male runners: สมัคร 17 คน วิเคราะห์ 15 คน ตัด ectopy สองคน; Bruce protocol treadmill จน exhaustion, อุณหภูมิราว 24°C
- Reference เป็น VT2 จาก **Oxynet gas-exchange neural network** ไม่ใช่ sampled lactate, OBLA4 หรือ MLSS; reference error ต้องไม่ถือเป็นศูนย์
- Biopac ECG 1000 Hz; Kubios **3.4.3**, λ=500, automatic correction, scales 4–16 beats, centered 120 s windows/5 s; artifacts <5% บริเวณ threshold; linear rapid-drop regression แล้ว solve alpha=0.50
- HR means VT2 174±12 เทียบ HRVT2 171±16 bpm; r=0.78, SEE **10.5 bpm**; reported bias **−4 bpm**, LOA **−24 ถึง +16 bpm** Individual agreement กว้างแม้ค่าเฉลี่ยไม่ต่างอย่างมีนัยสำคัญ
- Table 1 มี 170 เทียบ 188 และ 166 เทียบ 144 bpm การคำนวณซ้ำจาก rounded pairs ในรายงานนี้ได้ mean (estimate−reference) −3.6667 bpm, MAE **8.2 bpm**, max absolute error **22 bpm**, range −22 ถึง +18 bpm ไม่ใช้ derived MAE นี้อ้างว่า validated ใน Garmin cohort
- Raw data available from authors; ไม่ใช่ publicly licensed exercise RR fixture
- วิธี crossing ไม่จำเป็นต้องไปถึง exhaustion ทางคณิตศาสตร์ แต่ validation protocol ทำจน exhaustion การหยุดเมื่อ crossing อย่างเดียวเป็น protocol change ที่ต้องตรวจ ไม่ประกาศว่าความตรงยังเท่าเดิม

**ข้อสรุป:** feasible VT2 proxy candidate จริง แต่ไม่ใช่หลักฐานพอสำหรับ precise LT2 zone automation

### S4 — Gronwald et al., 2024: prolonged running cross-over

[Full text](https://pmc.ncbi.nlm.nih.gov/articles/PMC11534628/), DOI [10.1002/ejsc.12175](https://doi.org/10.1002/ejsc.12175), PMID 39300759

- Trained endurance athletes 21 คน (9 หญิง, 12 ชาย), mean age 25.9 ปี; treadmill ramp เริ่ม 7 km/h หญิง/8 km/h ชาย เพิ่ม 1 km/h ต่อ min จน exhaustion หลัง warm-up 10 min; จากนั้น randomized 20-min bouts ที่ speed จาก DFA 0.75 และ 0.50 โดยห่างอย่างน้อย 72 h
- Polar H10 + V800; Kubios **4.1**, λ=500, automatic plus visual manual noise removal; scales 4–16, 120 s window, 5 s step สำหรับ ramp/10 s สำหรับ prolonged bout; >5% artifacts excluded
- VT1 ใช้ V-slope/other ventilatory criteria; VT2 ใช้ VE/VCO2 disproportionate rise และ PETCO2 fall; ผู้ประเมินสองคน ถ้าไม่ตรงใช้ mean ไม่ใช่ blood lactate threshold แม้มี lactate ก่อน/หลัง bouts
- Linear section regression หรือ multiphasic dose-response เมื่อ goodness-of-fit ดีกว่า เป็นอีก implementation variation ไม่ควรเอามาปน version เดียว
- Mean speeds DFA1 10.6 เทียบ VT1 10.8 km/h; DFA2 13.1 เทียบ VT2 13.2 km/h **ไม่พิสูจน์ individual interchangeability** ไม่มี threshold HR Bland–Altman limits ให้ใช้แทน S2
- LT2-like speed bout จบเพียง 15/21; 6 คนหมดแรงก่อน 20 min (11:47±03:13 min:s), lactate ปลาย bout 9.98±2.41 mmol/L ในกลุ่มนี้ speed DFA2 15.2 เทียบ VT2 13.6 km/h

**ข้อสรุป:** ต้องแยก numerical crossing จาก safe exercise prescription ห้าม suggest ให้คง pace จาก DFA2 20 นาทีเพื่อ “ยืนยัน” โดยระบบอัตโนมัติ

### S9 — Mateo-March et al., online 2022 / issue 2023: elite cycling LT1/LT2

[Primary abstract](https://pubmed.ncbi.nlm.nih.gov/35238695/), DOI [10.1080/17461391.2022.2047228](https://doi.org/10.1080/17461391.2022.2047228)

38 male elite cyclists ทำ graded exercise test พร้อม RR/lactate DFA0.75 ไม่ต่างจาก LT1 ใน means แต่ DFA0.50 **ต่างจาก LT2 ทั้ง power (p=0.04) และ HR (p=0.02)** แม้ correlation power r=0.93 และ HR r=0.71 ไม่ใช้ correlation เป็นใบอนุญาตยืนยัน LT2 Individual LOA และ exact lactate criterion ไม่อยู่ใน abstract ที่อ่านได้ จึงไม่เติมตัวเลขเหล่านี้จากแหล่ง secondary

S8 เพิ่มข้อขัดแย้ง: HRVT2 เทียบ VT2 bias **+7.5 bpm, SD 10.1**, เทียบ OBLA4 **+9.2 bpm, SD 10.4** แต่ power agreement ดีกว่า ห้ามแปลง cycling watts หรือ correlation เป็น running HR correction ratio

### S6 — supplied source: McGehee, Tanner, Houmard, 2005: 30-minute TT

[Primary abstract](https://pubmed.ncbi.nlm.nih.gov/16095403/), DOI [10.1519/15444.1](https://doi.org/10.1519/15444.1)

- Competitive distance runners/triathletes 27 คน; criterion คือ **blood lactate 4.0 mmol/L** ไม่ใช่ individual LT1 หรือ MLSS
- เปรียบเทียบ VDOT, 3200-m TT, 30-minute TT, Conconi; mean velocity ของ 30-minute TT ไม่ต่างจาก criterion, SEE **0.21 m/s**; HR SEE **8.0 bpm** VDOT velocity SEE 0.41 m/s
- SEE ไม่ใช่ individual 95% CI หรือ guarantee ว่าคลาดเคลื่อนไม่เกิน 8 bpm ไม่มี LOA ใน abstract
- DOI resolver ให้ bibliographic record แต่หา public primary full text ไม่ได้ในรอบนี้ ดังนั้นยัง **ไม่ pin** warm-up, HR averaging period เช่น last 20 minutes, surface, pacing instruction หรือ maximal confirmation จากบทความนี้ ไม่เอา secondary coaching advice มาเติมเป็น study protocol
- Reproduction gate ต้องได้ original methods/permissions แล้ว pin exact start/end, elapsed versus timer duration, HR averaging, distance/speed source, environment และ TT verification ก่อน implementation
- FIT-only speed/HR ไม่พิสูจน์ maximal effort, motivation, RPE, pacing intention, race/TT protocol หรือผู้ทำสามารถฝึกหนักได้ Workout label เป็น recorded claim ไม่ใช่หลักฐานว่า maximal test สำเร็จ `fastest30Minutes` ใช้สรุป workload ได้ แต่ห้ามส่งเป็น LT2 estimate

**ข้อสรุป:** deferred performance-to-OBLA4 candidate ที่มี primary running evidence ไม่รองรับ automatic detection จาก ordinary runs และไม่ใช้แทน DFA หรือ lactate truth

## 5. Candidate อื่นและวิธีที่ไม่เลือก

### S10 — Cottin et al., 2007: RR spectral running candidate

[Primary abstract](https://pubmed.ncbi.nlm.nih.gov/17024637/), DOI [10.1055/s-2006-924355](https://doi.org/10.1055/s-2006-924355)

Professional soccer players 12 คน ทำ incremental track running จน exhaustion; SPWVD time-frequency RR analysis ใช้ **HF spectral power × HF peak frequency** หา HFT1/HFT2 เทียบ ventilatory equivalents VT1/VT2; speeds 10.08 เทียบ 9.83 และ 12.58 เทียบ 12.55 km/h; R² 0.94/0.96 Abstract อ้าง Bland–Altman แต่ไม่ให้ individual limits ไม่สามารถเอา R² มาแทน limits ได้ HF peak frequency เพียงตัวเดียวให้ linear relation กับ speed จึงหา thresholds ไม่ได้

เลือก deferred ไม่ใช่ “ไม่มี RR running method อื่น” Gates ที่เหลือคือ full methods, HF upper-band definition/fmax, resampling, SPWVD kernel/window/breakpoint selector, artifact handling, numerical implementation license และ raw paired running corpus ไม่มีเหตุให้เพิ่ม second numerical implementation ก่อน candidate DFA ผ่าน reproduction

### S7 — supplied source: Wingo, Stone, Ng, 2020: drift ไม่ใช่ threshold estimator

[Primary abstract](https://pubmed.ncbi.nlm.nih.gov/32102057/), DOI [10.1249/MSS.0000000000002324](https://doi.org/10.1249/MSS.0000000000002324)

Active men 7 คน วิ่ง/ปั่น 15 หรือ 45 นาทีที่ initial 60% VO2max ใน 35°C แล้ว GXT; HR เพิ่ม **19% ใน running และ 17% ใน cycling** จากนาที 15 ถึง 45 และ stroke volume ลด 20%/15% ผู้วิจัยเชื่อม drift กับลด VO2max และเพิ่ม relative metabolic intensity ไม่ได้ validate “drift≤5%=LT1” หรือใช้ drift crossing หา LT2

ข้อเสนอ: export decoupling/drift เป็น descriptive workload/context พร้อม formula, time windows, pause/gap/grade/temperature availability ไม่คืน threshold จาก drift ตัวเดียว ไม่คัด segment เพราะ HR ไม่นิ่งจนลบ physiological drift จริง

## 6. Inputs, quality และสิ่งที่ FIT พิสูจน์ไม่ได้

ข้อกำหนดต่อไปนี้เป็น **proposed system contract** ไม่ใช่สิ่งที่ implementation นี้ผ่านแล้ว:

1. RR ต้องเป็น sensor-derived interbeat intervals ระหว่าง running session มี provenance เช่น native `hrv.time` array หรือ developer field ที่ยืนยัน semantics ได้ รักษา beat order, units, invalids และ original bytes ห้ามสร้าง RR=`60000/record.heartRate` การเฉลี่ย HR ทำให้ beat fluctuation สูญหาย ไม่ recover ด้วย interpolation
2. Native RR ที่พบยังต้องพิสูจน์ time anchoring กับ running/timer events; message arrival timestamp ไม่จำเป็นต้องเท่ากับทุก beat timestamp หาก reconstruction ambiguous ใช้ `rrTimeAlignmentUncertain` และ abstain การ pause แล้วนำ arrays ต่อกันห้ามเป็น continuous physiological window
3. Segment eligibility ไม่ตัดทั้ง activity เพราะมี strides; ใช้ eligible incremental portion ที่ continuous และมี alpha range คร่อม target จริง ไม่ใช้ windows ข้าม pauses, gaps, sensor disconnect, intervals/recovery หรือ high-intensity carryover ไม่มี extrapolation ออกนอก observed HR range
4. ต้องบันทึก denominator และ corrected/missing/invalid beat count **ต่อ 120-s window และต่อ regression segment** ไม่ใช้ artifact percent ทั้งกิจกรรมบดบังช่วงสัญญาณไม่ดี RR-only ไม่สามารถแยก ectopy จาก noise ได้ทุกกรณี ห้ามส่งผลเป็น arrhythmia diagnosis
5. S11 ทดสอบ correction/device bias จริง: induced missing beats 1%, 3%, 6% ทำให้ DFA bias เปลี่ยน; mean threshold shift เล็กไม่ได้แปลว่า raw alpha ไม่มี bias; Polar H7 เทียบ ECG มี mean HRVT bias ประมาณ −4 bpm [S11]
6. เสนอ ceiling **3% corrected beats** สำหรับ initial experimental running candidate อิงช่วง 0–3% ของ S1 และ ≤3% ใน running repeatability study S12 เป็น conservative research inclusion rule ไม่ใช่ universal cutoff ที่รับรอง physiology ถ้า >3% abstain ใน version แรก; threshold evidence บางงานใช้ <5% จึงต้องมี 3%, 5%, 6% sensitivity cases ไม่สับสนกับ drift 5%
7. การใช้ λ=500, 4–16 beats และ 120 s/5 s มาจาก research protocol ไม่ใช่ค่าความแม่นยำ 95% การผ่าน quality gateไม่ใช่ physiological confidence
8. FIT ตรวจได้เฉพาะ evidence ที่บันทึก: speed/time/grade estimates, sensor identity ที่มี, RR, HR, timer/laps/workout claims ไม่พิสูจน์ treadmill calibration, medication/health eligibility, caffeine/alcohol/meal timing, fan/heat acclimation, sleep, recovery, perceived exertion หรือ maximal effort RPE ถ้าบันทึกเป็น additional context ไม่ใช่ตัวตรวจ maximality สากล
9. FIT เดี่ยวไม่มี blood-lactate samples หรือ calibrated gas exchange reference ตาม studies; recorded lactate threshold HR ของ Garmin ไม่ใช้แทน paired lab reference

[S11 full artifact/device experiment](https://pmc.ncbi.nlm.nih.gov/articles/PMC7865269/), DOI [10.3390/s21030821](https://doi.org/10.3390/s21030821), Kubios **3.4.1**; 17 male volunteers, Bruce treadmill, ECG 1000 Hz + Polar H7; subset artifact-free HRVT n=10, paired devices <5% artifacts n=11 อย่านับเป็น independent new cohort เพิ่มจาก S1 เพราะใช้ testing cohort ที่เกี่ยวข้องกัน

[S12 full running repeatability study](https://pmc.ncbi.nlm.nih.gov/articles/PMC10582140/), DOI [10.1007/s10484-023-09599-x](https://doi.org/10.1007/s10484-023-09599-x): 10 คน (8 ชาย, 2 หญิง), Polar H10/FatMaxxer, Kubios **3.5**, λ=500, scales 4–16, ≤3% artifact, standardized low-intensity 2-minute portion ก่อน/หลัง exhaustive ramp; DFA SEM 0.12 fresh และ 0.14 fatigued, ICC 0.85/0.55 **ไม่ใช่ reliability of LT HR threshold** ห้ามใช้ SWC 0.06/0.07 เป็น threshold-CI ของแต่ละ activity

## 7. Reproducible numerical path และ license gates

### Published path ที่ต้อง reproduce

แยก stages ให้ trace ตรวจได้: raw exercise RR/time alignment; artifact annotations/corrected copy; smoothness-priors detrending λ=500; integrated centered RR profile; local linear detrend; pooled RMS fluctuation F(n) ที่ n=4…16; ordinary least squares ของ log F(n) ต่อ log n; 120-s centered time windows ที่ step 5 s; window HR; fixed-before-results decline selector; regression alpha = slope×HR + intercept; crossing HR = (target−intercept)/slope

การคำนวณ pooled RMS ต้องรวม squared residuals ทุก subwindow ก่อน square root ไม่ใช่เฉลี่ย RMS ของแต่ละ subwindow [S13] แม้ชื่อ DFA เหมือนกัน overlap, scale list, residual denominator, end-tail treatment, smoothness-priors sampling convention และ artifact correction เปลี่ยนผลได้ ต้อง pin ทุกตัว ไม่อ้าง Kubios-equivalent จากการอ่าน formula อย่างเดียว

Selector ที่ freeze สำหรับ research version: contiguous chronological windows ที่ corrected-quality ผ่านและ alpha อยู่ใน [0.5,1.0] อย่างน้อย 3 windows; อนุญาต boundary window ที่ติดกันโดยตรงไม่เกินหนึ่งที่ต้นและหนึ่งที่ท้ายเพื่อให้เห็น observed crossing ต้องมี negative fitted slope, nonzero HR variance, observed target bracket, fitted crossing ภายใน selected observed HR range และ no pause/gap หากมีหลาย eligible competing decline sections ให้ `ambiguousCrossing` ไม่เลือกอันที่ใกล้ device threshold นี่เป็น **open automatic adaptation** เพราะ papers ใช้ visual section selection; ต้อง validate selector ต่างหาก ห้ามเรียก exact Kubios reproduction ไม่เพิ่ม R² cutoff หรือ relax noExtrapolation เพื่อให้ fixture ผ่าน

เมื่อเลือก algorithm แล้วให้ pin window closure/time anchoring และ upper/lower crossing equality โดย fixtures ไม่เพิ่ม post-hoc R² cutoff หรือ pick “best” combination บน holdout ไม่มีค่า R² สากลในหลักฐานที่อนุญาตให้รับหรือปฏิเสธทุกคนอย่างอัตโนมัติ

### Software และ licensing

- Kubios versions ที่ studies ใช้: 3.3.2, 3.4.1, 3.4.3, 3.5, 4.0.1, 4.1 ไม่ใช่เวอร์ชันเดียวกัน เก็บ per-study configuration ไม่สลับเงียบ
- [Official Kubios HRV Scientific EULA](https://www.kubios.com/hrv-scientific-license/) effective 2023-10-18 จำกัด academic/personal licenses, redistribution และ third-party processing/service bureau ไม่มีสิทธิ์ bundle/headless commercial service จากการซื้อ personal license โดยอัตโนมัติ ต้องตรวจสิทธิ์ reference use ก่อนรัน; ครั้งนี้ไม่ได้ซื้อหรือรัน Kubios
- เลือก [nolds **0.6.3**](https://pypi.org/project/nolds/0.6.3/) เป็น **numerical comparator** ไม่ใช่ whole physiological estimator [versioned source](https://raw.githubusercontent.com/CSchoel/nolds/0.6.3/nolds/measures.py), [MIT license](https://raw.githubusercontent.com/CSchoel/nolds/master/LICENSE.txt) ให้ใช้/แก้/แจกได้โดยรักษา notice
- Comparator config เสนอ `nvals=range(4,17), order=1, fit_trend='poly', fit_exp='poly', overlap=False, debug_data=True` ไม่ใช้ default scale range หรือ randomized RANSAC [S13] `overlap=False` เป็น explicit research choice ที่ยังต้องเทียบ Kubios ไม่อ้างว่า published papers ระบุ overlap เช่นนี้
- nolds ไม่ทำ Kubios automatic artifact correction หรือ smoothness-priors pipeline ให้เอง การใช้งาน comparator บน clean RR ไม่ยืนยัน corrected noisy pipeline ข้อมูล proprietary preprocessing ที่ไม่ reproduce ได้ต้องทำ open replacement ตาม published algorithm พร้อม paired reference tests และเปลี่ยน methodVersion ไม่ reverse-engineer software ที่ license ห้าม
- [S14 Peng et al. original DFA paper](https://pubmed.ncbi.nlm.nih.gov/11538314/), DOI [10.1063/1.166141](https://doi.org/10.1063/1.166141), เป็นต้นทาง mathematical method ไม่ใช่ running threshold validation
- [S13 nolds author documentation](https://nolds.readthedocs.io/en/latest/nolds.html#detrended-fluctuation-analysis) อธิบาย algorithm/defaults; method artifact manifest ต้อง pin runtime, dependency versions, source/content hashes, floating-point policy และ config ก่อนรัน ไม่ใช้ mutable `latest` เป็น release pin

## 8. Numerical fixtures ไม่ใช่ physiological validation

รายการต่อไปนี้เริ่มเป็น fixture specification ในรอบ research 2026-10-04 ปัจจุบันสร้าง software fixtures และ engine แล้วตามข้อ 13 ส่วน licensed human comparator/physiological fixtures ยังขาดและไม่ถูกแทนด้วย synthetic data

**Positive software fixtures**

1. Known OLS crossing: HR `[130,140,150,160,170]`, alpha `[1,.875,.75,.625,.5]`; slope −0.0125, intercept 2.625; alpha .75 ต้องได้ **150 bpm**, alpha .50 ต้องได้ **170 bpm** ใช้ตรวจ crossing arithmetic เท่านั้น ไม่อ้างว่า alpha array นี้เกิดจาก human RR
2. Deterministic synthetic RR vector ที่เก็บ numeric values/seed/generator version/content hash; compare full F(4)…F(16) และ alpha กับ nolds 0.6.3 config เดียวกัน รวม end-tail, constant shift และ unit scaling cases วิธี DFA ต้องให้ผลคงเดิมเมื่อเปลี่ยน RR ms เป็น s หลังตรวจ unit boundary toleranceเสนอ **absolute 1e−9** สำหรับ alpha และ analytical OLS HR ใน double precision เป็น software agreement tolerance ไม่ใช่ 1e−9 physiological accuracy ถ้า platform numerical error ไม่ผ่าน ต้องอธิบายก่อนแก้ tolerance
3. End-to-end synthetic FIT ที่มี RR จริงในความหมายของ encoded beat intervals ไม่ใช่ recorded HR-only; independently specified beat ordering/timing/gap และ matching numerical golden ให้มีทั้ง 0.75 และ 0.50 bracket จาก actual numerical pipeline **ต้องรันถึง positive experimental result** ก่อนเรียก engine implemented Synthetic ไม่มี human validity
4. Licensed real exercise-RR segment ที่มี clean/annotated-artifact output จาก authorized reference pipeline ตรวจ algorithm agreement; golden physiological target ต้องอยู่คนละ field กับ golden numerical alpha/crossing

**Negative/abstention fixtures**

- HR-only FIT, resting HRV ถูกแนบใน running session, unknown RR units/provenance, invalid/zero RR, missing time anchor, duplicate/reordered beats, truncated RR array และ pause/disconnect ตรงกลาง window
- Constant RR (F(n)=0; alpha undefined), shorter than required complete 120-s window, alpha ไม่คร่อม .75 หรือ .50, zero/positive slope, no distinct HR, competing decline portions, window ไม่มี finite numerical output และ threshold ต้อง extrapolate
- Corrected count รอบ 3% inclusion boundary; 5% และ 6% เป็น sensitivity/reject cases ไม่ใช่ “fix ให้ output ได้”; injected missed/double beats ต้องมี known count ไม่ลบโดยไม่บันทึก
- Activity มี strides แต่ early eligible ramp ยังต้องใช้ได้; late surge/recovery หลัง intervals ต้องไม่ถูกเลือกเป็น fresh threshold; opposite LT1/LT2 estimates ต้องได้ conflict ไม่ clamp หรือ derive missing partner ด้วย ratio
- Later-uploaded old activity, source deletion, processing failure และ reprocess version ให้ตรวจ cutoff/revision และ last-good separation ไม่ใช้ null ของ latest attempt ลบ evidence ของ old valid result

**รอบ research 2026-10-04:** throwaway Python **3.12.13** คำนวณ OLS fixture (150.0/170.0 bpm) และ paired errors จาก S2 Table 1 (n=15, MAE 8.2, max 22 bpm) รอบนั้นยังไม่ได้รัน DFA/FIT engine ข้อ 13 ระบุหลักฐาน implementation รอบปัจจุบันแยกจาก physiological validation

## 9. Dataset availability และสิ่งที่ยังขาด

S1/S2/S5 raw data statements ระบุขอจากผู้เขียน ไม่ใช่สิทธิ์ redistribute หรือ automatically approved physiological reference corpus การยืนยันว่ามี public article ไม่เท่ากับมี public RR data

S12 มี [OSF project SRVHC](https://osf.io/srvhc/), [public file-list API](https://api.osf.io/v2/nodes/srvhc/files/osfstorage/), [node metadata](https://api.osf.io/v2/nodes/srvhc/?format=json) ตรวจพบไฟล์ **Dataset.xlsx**, version 1, 22,999 bytes, modified 2022-11-14; reported SHA-256 `a5f7b04c3174d421ec1cd1c8ec6dcd2055b9d3504fcfa5e690031c8a5e9c6205`; children count 0 [license metadata](https://api.osf.io/v2/licenses/563c1cf88c5e4a3877f9e965/?format=json) เป็น **No license**

Download ตอบ HTTP 429; render endpoint ตอบ HTTP 400 จึงยังไม่ตรวจ workbook cells และไม่อ้างว่ามี raw RR/full ramp/threshold annotationsอยู่ภายใน File-list ของ public project มีไฟล์เดียว; **corpus ที่ตรงโจทย์และมี reuse license ยังไม่ยืนยัน** ห้ามประกาศว่าไม่มี reference dataset ใดในโลก แต่ห้ามนำ workbook นี้มาประกาศว่าปิด gateแล้ว ต้องได้ licensed paired exercise RR + independent gas-exchange VT1/VT2 หรือเลือก permitted research collection ตาม protocol ที่ preregister

Public paper table ใช้ตรวจ reported error arithmetic ได้ แต่ไม่มี RR จึงไม่ใช้ reproduce whole algorithm Synthetic fixtures แก้ numerical gate ได้ แต่แก้ physiology gate ไม่ได้

### ความเป็นอิสระและ conflict disclosures

S2 ระบุ David Giles ทำงานที่ Lattice Training และไม่มี external funding; S5 ประกาศไม่มี commercial/financial conflict; S4 ประกาศไม่มี conflict และไม่มี funding นอก institutional salary support; S8 ระบุ public research grants และไม่มี commercial/financial conflict ตาม full primary texts ที่ลิงก์ไว้ นี่เป็น disclosure ไม่ใช่การตัดสินว่าผลงานถูกหรือผิด S1/S2/S11 มีผู้เขียนและ related cohort ซ้ำกัน; S3/S4/S12 มีผู้พัฒนาสมมติฐาน DFA ร่วมเขียน จึงควรได้ independent testing/analysis team ที่ไม่เลือก reference หรือ selector เพื่อให้ตรง DFA ก่อน validated release สำหรับแหล่งที่อ่านเฉพาะ abstract ไม่อ้างว่าไม่มี conflict เพราะไม่ได้ตรวจ disclosure ทั้งฉบับ

## 10. Predeclared validation และ release gates

### G1 — input semantics และ reproduction

ก่อน runtime candidate เปิดรับ FIT: decoder ต้องผ่าน beat units/order/timestamp/gap fixtures และ immutable provenance; real exercise RR corpus อย่างน้อยหนึ่งไฟล์ต้อง decode ถึง analysis path โดยไม่มี HR-derived substitution ต้องมี authorized reference preprocessing หรือ documented independently validated open replacement Pin all numerical/config/license details ตามข้อ 7 และผ่าน positive/negative fixtures ข้อ 8

เกณฑ์ numerical agreementใช้ 1e−9 เฉพาะ same mathematical config ที่ระบุ ไม่ใช้เทียบ arbitrary Kubios version ต่าง preprocessing การเทียบ reference pipeline ต้องรายงาน F(n), alpha, selected windows, corrected counts และ crossing differences ไม่ compare final HR อย่างเดียว

### G2 — locked target/protocol และ selection

เลือก VT1/VT2 definitions ตามข้อ 2; paired testing ใช้ running ไม่ใช้ cycling มา calibrate HR; reference raters blind ต่อ DFA result และ device threshold; ประเมิน disagreement แยกจาก consensus ทำ reference repeat/retest subset ให้รู้ reference uncertainty

Candidate policy และ artifact/protocol selectorต้อง frozen ก่อนดู held-out reference ห้ามเปลี่ยน 0.75/0.50, warm-up exclusion, artifact ceiling, decline selector หรือเลือกคนที่ตรงผลเพื่อ improve accuracy หากแก้ policy ให้สร้าง new version และ new untouched validation cohort

### G3 — split, independence และ no leakage

- Split ตาม **user** ก่อน; development/calibration users ไม่ปรากฏใน independent-user holdout ไม่ random split overlapping 120-s windows
- สำหรับ longitudinal test ให้ freeze model/config ณ cutoff แล้วเดินกิจกรรมของแต่ละ holdout user ตาม activity-end chronology ใช้เฉพาะกิจกรรมก่อน/เท่ากับ `evidenceCutoff` upload time และ computation time ใช้แยก ไม่มี future calibration หรือ future RR ใน historical estimate
- Centered 120-s window มีข้อมูล t−60 ถึง t+60 ต้องมีทั้งหมดก่อน cutoff ถ้า activityจบก่อน t+60 ห้ามคำนวณ window โดยเติมข้อมูล/ดึง next activity การวิเคราะห์ภายในกิจกรรมแบบ retrospective ยอมใช้ข้อมูลหลัง window-center เมื่อข้อมูลทั้ง windowอยู่ก่อน activity-end cutoffเท่านั้น
- Analysis unit คือ independent user/independent activity ไม่ใช่ windows งาน S1/S2/S11 ที่ใช้ related cohortไม่นับเป็น independent replication สามชุด Separate-day repeated activities ใช้ clustered/repeated-measures analysis ไม่เพิ่ม n จาก overlaps
- Deduplicate original/hash/possible duplicates ก่อน weighting; selected export sessions ไม่เปลี่ยน owner-history research evidence; no foreign owner/no future/no deleted evidence ใช้ input manifest และ source/version hashes reproduce

### G4 — quantitative research screening ไม่ใช่ validated release

Predeclare errorเป็น **estimate−reference** และกลับ sign ของ published comparisons เมื่อจำเป็น Metrics ต่อ LT1/LT2 และต่อ target: bias, MAE, median absolute error, RMSE, Bland–Altman LOA พร้อม clustered uncertainty, largest errors, proportional bias, repeatability, eligible coverage, abstention counts/reasons และทุก failed subject ห้ามรายงาน correlation alone หรือ p>0.05 เป็น equivalence

เสนอ **research replication screen** ก่อนขยาย study:

- LT1 VT1 HR: |bias| ≤ **1.9 bpm** และ LOA ทั้งสองอยู่ภายใน **±12.2 bpm** ตัวเลขอิง S1 mean-bias magnitude และ S3 fresh first-ramp largest LOA magnitude จึงเป็น no-worse-than-published benchmark ไม่ใช่ medically acceptable error
- LT2 VT2 HR: |bias| ≤ **4 bpm** และ LOA ภายใน **±24 bpm** อิง S2 เป็น screening benchmark เท่านั้น ไม่อนุญาต claim precise zones เพราะ 24 bpm เป็น error กว้าง และ S4 แสดง overload ในบางคน
- ต้องรายงาน CI ของ bias/LOA และ participant-level uncertainty ไม่ใช้ point estimates ที่ sample เล็กประกาศผ่าน หาก confidence boundsเกิน benchmark ให้ inconclusive/failed ไม่ผ่าน
- ไม่กำหนด 20/30/100 คนแบบไร้ rationale ก่อนเห็น reference variability วาง sample size ด้วย prespecified CI precision จาก development independent-activity error distribution ไม่ใช้ holdout มาคำนวณซ้ำแล้วหยุดเมื่อผ่าน ตัวเลข sample size และ eligible coverage ยัง **ไม่ pin** เพราะ corpus/variance และ target-population denominatorยังไม่ทราบ ถือ gate blocked ไม่ใช่ complete

### G5 — gate ก่อนใช้คำว่า validated

ผ่าน G4 ไม่พอสำหรับ “validated LT1/LT2” เสนอ intended use แรกเป็น **descriptive ventilatory-proxy estimate ไม่ใช่ auto zone prescription** ต้องมี independent Garmin-running RR cohort, population/sensor/protocol coverage, individual-error analysis และ false confident-output audit ที่เหมาะกับ intended use

เสนอ criterion ที่ไม่สร้าง error toleranceตามใจ: estimator paired MAE ต้องไม่แย่กว่า **independent reference repeat/retest MAE** และ LOA width ต้องไม่กว้างกว่า **paired reference-repeat LOA width** ใน protocol/populationเดียวกัน ให้ upper confidence bound ของ excess MAE และ excess LOA width ไม่เกินศูนย์ ไม่ใช้ “ไม่พบ significant difference” เป็น equivalence Reference precisionต้องวัด/lock จาก development protocolก่อน holdout หาก reference ambiguous ให้ adjudicate หรือ report reference-indeterminate ไม่ลบเพื่อ improve score เกณฑ์นี้ไม่พิสูจน์ clinical/training safety; automatic training prescriptionยังต้อง outcome/safety justification แยก

หากจะใช้ lactate LT1/LT2 label ต้องผ่าน **running lactate** G2/G3/G5 ใน exact definition ที่เลือก ไม่เอา VT agreementมาเปลี่ยนชื่อ หากไม่มี reference-repeat/raw licensed dataset ขณะ release ให้คง `experimental` หรือ `researchBlocked` พร้อมเหตุผลจริง ไม่สร้าง CI/validated confidence หรือทุก activity null placeholderให้ดูเหมือน engineเสร็จ

## 11. Runtime status, uncertainty, conflicts และ suggestions

API/UI ใช้ locked status **`estimated | low_confidence | insufficient_data`** แยก LT1 และ LT2 ตาม D14 เท่านั้น ชื่อต่อไปนี้เป็น **engine metadata/reason** ไม่ใช่ status ใหม่:

- `experimentalEstimate`: มี numerical candidateจาก exercise RR ที่ผ่าน G1/G2 runtime eligibility แต่ยังไม่ผ่าน G5; value, unit, exact proxy target, method/config/revision, evidence date range/count, quality summary, known physiological limitation ต้องไปกับ export
- `validatedEstimate`: ใช้ได้เมื่อ exact target/sport/protocol/sensor scopeผ่าน G5 เท่านั้น ไม่อัปเกรดจาก number of windows/low artifact
- `abstained`: methodมี numerical engineแต่ activity/evidenceไม่พอ; `noExerciseRR`, `rrTimeAlignmentUncertain`, `insufficientContinuousWindow`, `excessArtifact`, `noTargetBracket`, `nonDecliningRelation`, `ambiguousCrossing`, `unsupportedProtocol`, `fatigueCarryover`, `requiredContextUnprovable` ระบุแยก ไม่รวมทุกเหตุเป็น “not enough data”
- `researchBlocked`/`methodNotReleased`: reference/license/implementation gateยังไม่ผ่าน ไม่อ้างว่า personไม่มี LT หรือ fitter engine abstainedจากการวิเคราะห์ที่ไม่เคยรัน
- `conflictingEvidence`: estimatesสำหรับ same targetและ eligible contextsไม่สอดคล้องกัน; เก็บทุก method/target revision ไม่เฉลี่ย lactateกับventilatory/protocolต่างกัน และไม่ clamp LT1 ต่ำกว่า LT2 หาก estimatesกลับลำดับให้ conflictทั้ง pairโดยรักษา raw numerical results
- `stale`/`sourceUnavailable`: last valid resultแยกจาก latest attempt/failure/deleted dependency; historical exportใช้ as-of revision ไม่ fallback latest

Mapping ที่เสนอ: `validatedEstimate` ที่ยัง valid และมี evidence ภายใน scope เป็น `estimated`; positive `experimentalEstimate` เป็น `low_confidence` พร้อม numerical value และ proxy label; `abstained` หลัง method ตรวจ inputs จริงและไม่มี usable value เป็น `insufficient_data` พร้อมเหตุผล `researchBlocked`/`methodNotReleased` เป็น method-availability metadata โดยไม่มี physiological result ที่อ้างว่า engine วิเคราะห์แล้ว ไม่เปลี่ยนเป็น user-data insufficiency. `conflictingEvidence` ที่ยังมี usable candidates ให้ `low_confidence` โดยไม่สร้างค่า consensus หรือ automatic zone; ถ้า conflict ทำให้ method eligibility ไม่ผ่านให้ `insufficient_data`. `stale` เป็น freshness metadata ของผลเก่า ไม่ใช่ status ที่สี่; `sourceUnavailable` เป็น provenance metadata และทำให้ method abstain เฉพาะเมื่อ prerequisites ที่จำเป็นตรวจไม่ได้ ผล recorded Garmin threshold ไม่เปลี่ยน mapping นี้

Job queued/running/failed และ processing error แยกจาก physiological status ไม่แปลง failureเป็น `insufficient_data` ที่เสแสร้งว่าได้วิเคราะห์แล้ว UI แสดง latest attempt กับ older last-good resultคนละส่วนพร้อม revision/evidence dates และ stale badge

**Unknown context policy ที่เลือกสำหรับ numerical candidate:** `running-dfa-a1-075/050` แบบ experimental ใช้ observed continuous progressive-running segmentและreal exercise RR เป็น required facts ไม่ require proof of maximal effort, fresh/recovered state หรือ exact laboratory preparation เนื่องจากไม่อ้าง exact study reproduction ความไม่ทราบ recovery/caffeine/meal/sleepเป็น `contextUnverified` metadataและคง `low_confidence` เท่านั้น ไม่กล่าวว่าความสดผ่าน หาก FIT แสดง interval carryover/fatiguing protocolที่ไม่เข้า eligible classให้ abstain หาก chosen versionกำหนด protocol factใดเป็น REQUIRED แต่ FITพิสูจน์ไม่ได้ เช่น sensor RR provenance/time continuity หรือ verified maximal TT ให้ `requiredContextUnprovable` และ abstain ห้ามใช้ user confirmation/workout labelปลด gateและห้าม downgrade REQUIRED factเป็น optionalเพียงเพื่อสร้าง output

### Recency และ comparability ที่เลือก

Initial experimental versionไม่ pool HR correction หรือ threshold valuesข้าม sensor models/recording paths/protocol classes ไม่ใช้ cycling calibrationแก้ running HR และไม่รวมหลาย targetdefinitions Select **latest eligible independent running activityที่จบไม่เกิน cutoff** จาก owner history โดยไม่อิง export checkboxes; tie-breakด้วย stable activity ID ไม่จำนวน overlapping windows ไม่มี minimum session count: หนึ่งกิจกรรมที่ครบ eligibilityอาจให้ low-confidence numerical candidateได้ จำนวนกิจกรรมมากไม่ยกระดับ confidenceเอง

ใช้ lookback **ไม่เกิน 7 วัน** จาก `evidenceCutoff` ถึง source activity-end สำหรับ initial experimental current value เป็น **operational conservative ceiling ที่เสนอ ไม่ใช่หลักฐานว่า physiologyคงที่ 7 วัน** S12 ใช้ repeat sessionsห่างราวหนึ่งสัปดาห์และพบความแปรปรวน; S8 ใช้ 6–9 วันและพบ systematic HR differences; S3 แสดง fatigueเปลี่ยน agreementได้ภายใน sessionเดียว จึงไม่รับประกันความคงที่แม้ยังอยู่ใน 7 วัน และไม่เอา ceilingนี้ไปลด artifact/target gates

ถ้าไม่มี comparable eligible activityใน lookback ให้ latest attemptเป็น `insufficient_data`/`noRecentComparableEvidence`; latest cardอาจแสดง older last-good separatelyโดย `stale=true`, evidence ageและmethod limits แต่ไม่เสนอเป็น current threshold Historical per-activity estimateคำนวณอายุจาก cutoffของ activityนั้น ไม่จาก computedAtหรือวันนี้ การเปลี่ยน sensor/protocolต้องใช้ matching evidenceใหม่ ไม่ reuse old HR correctionแม้กิจกรรมเก่ายังอยู่ใน 7 วัน Policyนี้ versionedและต้องทดลอง recency sensitivityก่อน validated release

Uncertaintyต้องแยก **signal quality**, **numerical sensitivity** และ **physiological external validity** Artifactต่ำไม่ใช่ “95% confidence” OLS regression CIจาก overlapping windowsไม่ใช่ person-level threshold CI ไม่มี validated empirical interval ให้ `uncertaintyInterval=null` พร้อม `intervalUnavailable` ไม่ใช้ published LOAเป็น personal CI ไม่แสดงทศนิยมละเอียดเกิน input/validated error และไม่ promiseทั้ง LT1/LT2จาก activityเดียว

Suggestionsเฉพาะเหตุและไม่รับประกัน output:

- `noExerciseRR`: ตรวจว่าเครื่องบันทึกและ chest strapรองรับ/เปิด exercise beat-to-beat RR แล้ว import runใหม่ที่บันทึกจริง HR-only runเดิมไม่สามารถ recover RRได้
- `excessArtifact`: ตรวจ belt contact/fit, sensor disconnect และ recording support; ข้อมูลใหม่อาจยังไม่ผ่าน ไม่บอกให้เร่งจน thresholdปรากฏ
- `unsupportedProtocol`/`noTargetBracket`: แจ้งว่า ordinary runนี้ไม่มี eligible progressionเพียงพอ ให้เก็บตามแผนวิ่งเดิม ไม่ defaultเพิ่ม effort/all-out หากต้องการ thresholdที่ยืนยันได้เสนอ qualified lab assessmentพร้อม exact target definitionและ safety screening ไม่บังคับผู้ใช้ตัดสิน physiological model
- `fatigueCarryover`/`contextUnverified`: ไม่เอา late-session crossingเป็น fresh threshold; ใช้ future appropriately supervised standardized assessmentเมื่อเหมาะสม ไม่สั่งให้ทดสอบทันที
- `researchBlocked`: บอก gateและ scopeที่ขาดตรงๆ core FIT import/full exportยังใช้ได้ ผล recorded Garmin thresholdยังแสดงอย่างแยกประเภท

### SUG milestone: short method-specific collection templates

Milestoneถัดไปต้องทำ **short optional templatesแยก LT1/LT2** และตรวจ gatesก่อนเผยแพร่ ไม่ใช้คำแนะนำ labแทนฟีเจอร์นี้ Templateไม่ใช่ training planและไม่รับประกัน output:

- `running-dfa-a1-075`: “ถ้ามีช่วงวิ่งเพิ่มความหนักอย่างต่อเนื่องตามแผนเดิม ให้บันทึก exercise RR ด้วย sensorที่รองรับ รักษาช่วงต่อเนื่องโดยไม่หยุดหรือสลับ recovery; ระบบอาจใช้ช่วงที่ alphaคร่อม 0.75 หาก signalและmethod gatesผ่าน” Protocol templateที่จะใช้ reproduce studyต้อง pin warm-up/ramp/treadmill contextจาก S1/S4ก่อน ส่วน ordinary-run templateเป็น exploratory observed-progression class ไม่อ้าง protocolเดียวกัน
- `running-dfa-a1-050`: “ใช้ exercise RR จากช่วง progressionที่มีอยู่แล้วและเข้า method scope; ไม่ต้องเพิ่ม effortหรือวิ่งต่อเพื่อให้ alphaลงถึง 0.50 ถ้าข้อมูลไม่คร่อม targetระบบจะไม่ประมาณ LT2” Template submaximal stop-after-crossingยัง experimental protocol changeตาม S2 ต้องมี reference validationก่อนเปิดเป็น established assessment ห้าม default exhaustion/all-out
- `30-minute-time-trial-OBLA4-proxy`: ยังไม่เผยแพร่ collection templateจนได้ original full methodsและFIT-only eligibility proof หาก maximalityเป็น REQUIREDและFITพิสูจน์ไม่ได้ methodนี้ต้อง abstain ไม่ถามให้ผู้ใช้ยืนยันเพื่อ unlock

Template publication gate: approved method version, exact required/optional facts, sensor/protocol eligibility proof, numerical positive/negative path, Thai wordingที่ไม่ promiseผลและไม่ prescribe intensityที่เพิ่มขึ้นเพียงเพื่อสร้าง output Suggestionsเลือกจาก actual reason traceของ LT1/LT2แยกกัน

## 12. ผลรอบ research 2026-10-04 และ remaining release gates

**ทำแล้ว:** อ่าน supplied sourcesทั้งสามและ public full textsที่เข้าถึงได้; ตรวจ primary running/cycling validations, numerical comparator source/version/license, public dataset file/license metadata; คำนวณ primary paired-error arithmeticและ analytical OLS crossing smoke ตามข้อ 8; เขียนข้อเสนอ release contract

**ยังไม่ได้ทำในรอบ research:** DFA implementation, raw RR parsing/quality/correction comparator และ FIT positive path ได้ทำในรอบ implementation ตามข้อ 13 แล้ว ส่วน physiological testing, clinical/training safety validation, decoder deployment, baseline suite และ private FIT access ไม่ได้ทำในงาน numerical slice นี้

**Experimentalที่เลือก:** DFA-a1 0.75/0.50 running ventilatory proxies เป็นสอง real numerical candidates พร้อม implementation/reproduction plan ไม่ใช่ universal impossibilityและไม่ใช่ completed null engine

**Blocked สำหรับ human reference research:** licensed paired exercise-RR reference corpusและpermissions; authorized paired preprocessing/selector agreement; locked empirical sample-size/uncertainty plan G1–G4 Native supported source RR alignment และ independent software comparator ทำแล้ว แต่ไม่ใช้แทน human reference หรือรับรอง Kubios equivalence

**Blockedก่อน validated release:** independent running cohortและindividual/reference-repeat agreement G5; lactate-specific paired running evidenceหากใช้ lactate claim; intended-use error/safety evidenceหากจะ prescribe zones

**Deferred:** original full TT protocol/verificationและSPWVD method/license/corpus; ไม่ต้องให้ผู้ใช้เลือก physiological formulaเพื่อปลด blockersเหล่านี้

**Not supported:** 5% drift threshold estimator, fastest-segment TT proof, RRจากsampled HR/resting HRV, fixed LT1/LT2 ratios, silent target substitution, automatic all-out suggestions และ manufactured physiological confidence

## 13. หลักฐาน implementation 2026-10-05

Engine `running-dfa-open-1.0.0` ใช้ smoothness-priors λ=500 ผ่าน pentadiagonal Cholesky, scales 4–16, non-overlap, pooled squared residual RMS, ordinary least squares และ 120-second/5-second windows ไม่มี HR stability filter ตัด drift, RR=`60000/HR`, fixed LT ratio หรือ extrapolation Native eligibility ต้องมี HR message 132 timestamp/fractional timestamp/event-counter anchors และ absolute/relative beat timing ที่สอดคล้อง ไม่ใช้ arrival timestamp ของ HRV message 78

ข้อมูลทั้งหมดในหลักฐานนี้เป็น **CC0 synthetic software fixtures** ไม่ใช่ human running corpus Independent SciPy/nolds oracle freeze ก่อนประเมิน engine และเปรียบเทียบ F(4)…F(16), alpha ทุก window และ crossing ไม่เทียบเฉพาะ scalar card:

- Original quantized native FIT: actual LT1 **134.71544856754312 bpm**, LT2 `null` เพราะ `noExtrapolation` Actual LT2 fitted crossing **157.41934068501405** สูงกว่า selected observed HR maximum **156.75** จึงต้องคง rejected candidate นี้ Maximum absolute alpha/F disagreement เทียบ oracle คือ **4.9934056889355816e−11 / 4.73042938153867e−10** ไม่ปรับ fixture เดิมหรือ gate เพื่อให้เป็น positive
- Distinct positive quantized native FIT: actual LT1 **136.41887336482208 bpm**, LT2 **160.4210626128638 bpm** เทียบ independent expected **136.41887336477964 / 160.42106261369022** Maximum absolute alpha/F disagreement **2.4642399232277512e−11 / 5.050644347193156e−10** แต่ละ target มี eligible decline section เดียว ส่วน LT2 อีก section ถูก reject จริง
- ทั้งสอง FIT มี 481 recorded samples และ 1,063 encoded RR intervals ผ่าน public decoder แล้ว ไม่ใช่ manufactured target fields หรือ mocked cached scalar

Regression ครอบคลุม artifact correction 3% inclusion และ 4/5/6% rejection, gaps/pauses/full-window timing, constant HR/RR, positive slope, chronological adjacency, ambiguity, early valid progression ก่อน late strides, event cutoff/deletion/revision/hash, distinct sensor scope, selected-only export และ sparse long-duration source

History รักษา failed target trace จาก actual evaluation ถ้าไม่มี numerical positive แทนที่จะคืน empty registry รักษา older eligible LT2 แยกจาก newer LT1 และแจ้ง `conflictingTargetOrder` โดยไม่ clamp Core เก็บ actual lastAvailable แยกจาก latest attempt Numerical projection hash หมายถึง input ที่ใช้จริง ไม่ใช่ full normalized archive hash Receipt สำหรับ condensed history มาจาก core ที่ยืนยัน immutable manifest เท่านั้น หากยังมี samples engine hash input เองและไม่เชื่อ receipt

Timer stop ที่ท้าย session ไม่สร้าง pause ให้ block ก่อนหน้า Fixture stop ที่ 450 s แทน 480 s ให้ observed pause 30 s, recorded timer total 480 s และ derived total 450 s พร้อม `timerTotalsDisagree` Source records/summary ไม่ถูกแก้ และ window ที่ข้าม stop ถูก reject แม้ center อยู่ก่อน stop

Supplemental genuine native competing-decline source 2,126 RR/961 records มี LT1 `ambiguousCrossing` จาก2eligiblesectionsจริง ไม่pickbestsection; LT2ยังมีsectionเดียวและได้ **160.4210626128638 bpm** ไม่มีการบังคับnullทั้งสองtarget Maximum alpha/F disagreement **2.4642399232277512e−11 / 6.309370803592174e−10** ผ่านoriginal strict1e−9 พร้อมindependentoracleที่freezeก่อนengine ไม่เปลี่ยนmethodหรือfixturesเดิม

Actual Core-reported HR-only/same-event-end group bugแก้reference electionแล้ว: unknownsensor newestHR-onlyrowไม่ซ่อนverifiedtimedRRsource; chronologicalfrontierยังbindingprovenchanged-sensorที่พบก่อนeligibleRRแม้มีunknownHR-onlyrowนำหน้าและcache stale ส่วนolderknowncontextไม่override newerverifiedunknownRR ไม่ยกระดับobservationgroupเป็นsensoridentity Scoped **33 tests/check** ผ่านรวมthree-rowRED/GREENregression และunchangednativepair→publicwriter→localhistoryfull/condensedreceiptshapeเลือกactualLT1 **134.6372770392192 bpm**, independentcount1 FocusedStandards/Specclosureคนละแกน0unresolved หลักฐานlocalsmokeไม่อ้างPGmanifestหรือHTTPintegrationซึ่งCoreตรวจแยก รายละเอียดและartifacthashesอยู่ในreproductionrecord

ความสำเร็จข้างต้นปิด software-positive path เท่านั้น **G1 ส่วน licensed human reference/authorized preprocessing comparison, G2 blind paired protocol, G3 independent-user/longitudinal holdout, G4 empirical error/uncertainty/sample-size และ G5 physiological release ยังไม่ผ่าน** ไม่มีสิทธิ์อ้าง validated lactate LT, person-level empirical CI, automatic zones หรือ Kubios-equivalent preprocessing [reproduction record](lt-engine-reproduction.md) ระบุ pins, license, tolerances และคำสั่งที่รันได้
