---
status: accepted
---

# Running DFA-a1 proxies แบบ experimental แยก LT1/LT2

เลือก DFA-a1 0.75 และ 0.50 จาก exercise RR จริงเป็น numerical candidates สำหรับ VT1/VT2 proxies แยกกัน ไม่เรียก blood-lactate LT ที่ validated หลักฐาน running มีจริง แต่ protocol, automatic selector, fatigue, individual agreement และ target mapping เป็นข้อจำกัด โดยเฉพาะ LT2 ที่มี agreement กว้าง ต้องคง exact target/experimental label ข้างค่า

ไม่เลือก drift5%, arbitrary fastest30min, HR-derived RR, fixed-ratio LT หรือ device setting เป็น independent physiological truth Deferred TT/SPWVD candidates ต้องได้ reproducible protocol และ licensed reference dataก่อน ไม่เพิ่ม algorithm เพื่อเติมตัวเลขบน card

ใช้ numerical engine `running-dfa-open-1.0.0` และ workload segmentation `workload-observed-1.0.0` แล้ว มี synthetic native FIT→recorded RR→positive LT1/LT2 พร้อม independent SciPy/nolds comparator ที่ freeze ก่อนผล engine รายละเอียดและคำสั่งอยู่ใน [reproduction record](../runs-rebuild/lt-engine-reproduction.md) Selector ใช้ contiguous alpha [0.5,1.0] และ boundary window ที่ติดกันโดยตรงไม่เกินหนึ่งด้านละหนึ่ง ต้องมี negative OLS slope, observed target bracket และ crossing ภายใน selected observed HR range; ไม่ extrapolate หรือเลือก competing sections เพื่อให้ได้เลข

API/UI estimate statuses ใช้ Estimated/Low confidence/Insufficient data ตาม contract; experimental, method-not-released, processing failure และ staleness เป็น metadata แยก ไม่ให้ผู้ใช้เลือกสูตรหรือกรอก maximal confirmationเพื่อปลดล็อกเลข (ARCH-05, LT-01–16, TIME-01–06)

Timer pause ใช้ union ของ source timer events และ sample timer state บน [start,end) แยก recorded timer total ออกจาก derived total และแจ้ง disagreement; stop ที่ปลาย session ไม่ทำให้ block ก่อนหน้ากลายเป็น pause ทุก RR window ต้องไม่ข้าม pause แม้ center จะอยู่ก่อน stop

History เลือก latest eligible independent activity แยกแต่ละ target ด้วย event cutoff และ lookback 7 วัน ไม่ pool calibration ข้าม sensor หาก source ไม่พิสูจน์ RR sensor identity ให้ใช้เฉพาะ reference activity ของ snapshot เมื่อไม่มี positive แต่มี actual evaluated failure ให้เก็บ windows, rejected candidates และเหตุผลจริง ไม่แทนด้วย empty registry trace การกลับลำดับ LT1/LT2 ต้องแจ้ง conflict โดยไม่ clamp Core เก็บ lastAvailable แยกจาก latest attempt

Numerical input projection `runs-numerical-input-1.0.0` รักษา samples/RR/laps/timer/sensors ทุกแถวโดยไม่ cap และไม่แก้ full archive `trace.inputHash` เป็น SHA-256 ของ Value ที่ engine ใช้จริง ส่วน `inputNormalizedHash` เป็น full normalized archive hash ที่ core ยืนยัน Core ส่ง condensed history ได้เฉพาะพร้อม server-only immutable projection receipt ที่ตรวจ revision/hash/version จาก trusted persistence แล้ว Receipt ไม่ใช่ FIT field หรือสิทธิ์ให้ public client รับรอง analysis เอง

Research G1–G5 ใน [method assessment](../runs-rebuild/lt-method-assessment.md) แยก software agreement ออกจาก physiological validity ยังไม่มี licensed paired human running RR + independent gas/lactate corpus จึง block validated LT, empirical person-level uncertainty และ automatic zone prescription ไม่ block original FIT/import/selected privacy-safe JSON และไม่อ้าง Kubios equivalence
