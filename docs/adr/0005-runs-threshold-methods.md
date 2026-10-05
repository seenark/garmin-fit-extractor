---
status: proposed
---

# ทดลอง running DFA-a1 proxies แยก LT1/LT2 หลัง research gate

เลือก DFA-a1 0.75 และ 0.50 จาก exercise RR จริงเป็น numerical candidates สำหรับ VT1/VT2 proxies แยกกัน ไม่เรียก blood-lactate LT ที่ validated หลักฐาน running มีจริง แต่ protocol, automatic selector, fatigue, individual agreement และ target mapping เป็นข้อจำกัด โดยเฉพาะ LT2 ที่มี agreement กว้าง ต้องคง exact target/experimental label ข้างค่า

ไม่เลือก drift5%, arbitrary fastest30min, HR-derived RR, fixed-ratio LT หรือ device setting เป็น independent physiological truth Deferred TT/SPWVD candidates ต้องได้ reproducible protocol และ licensed reference dataก่อน ไม่เพิ่ม algorithm เพื่อเติมตัวเลขบน card

Engine ต้องมี positive FIT→exercise RR→numerical result และ negative/conflict/history fixtures ก่อนเรียก implemented; null-only engine ไม่ครบ Research G1–G5 ใน [method assessment](../runs-rebuild/lt-method-assessment.md) แยก software correctnessจาก physiological validity. ไม่มี licensed paired running RR/reference corpus ที่ยืนยันแล้วจึงยัง blockedสำหรับ validation/release claims แต่ไม่ block original FIT/import/selected privacy-safe JSON

API/UI estimate statuses ใช้ Estimated/Low confidence/Insufficient data ตาม contract; experimental, method-not-released, processing failure และ staleness เป็น metadata แยก ไม่ให้ผู้ใช้เลือกสูตรหรือกรอก maximal confirmationเพื่อปลดล็อกเลข (ARCH-05, LT-01–16, TIME-01–06)
