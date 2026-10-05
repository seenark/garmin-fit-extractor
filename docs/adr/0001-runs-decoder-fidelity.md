---
status: proposed
---

# Runs decoder ต้องรักษา identity และ native representation

คง Axum API และใช้ Rust `fitparser` 0.11.0 เป็น baseline ของ decoder spike ไม่ rewrite backend เพื่อเปลี่ยนภาษา เสนอให้ขยาย archive seam ด้วย numeric/native/developer identity, definition/source references และ `KeepCompositeFields` โดยไม่ใช้ combined numeric-enum mode ที่ทำ developer metadata ล้ม การยอมรับ implementation ต้องแก้หรือหลีกเลี่ยง packed HR expansion ที่ corpus แสดงว่าผิด แล้วผ่าน native/expanded/order/invalid/raw fidelity goldens ก่อน cutover

Python `fitdecode` 0.11.0 เป็น MIT frame-level comparator และทางเลือกเมื่อ Rust archive gate ไม่ผ่าน; official Python/JS SDK ไม่เป็น decoder เดี่ยวโดยอัตโนมัติ เพราะ compressed timestamps/unknown defaults และ FIT Protocol License ต้องตรวจเพิ่มเติม การค้นพบ license §2(f) กั้น official SDK comparative benchmark/publication เพิ่ม ไม่ใช่ permission ที่ให้จากการมี package อยู่เดิม

Original FIT retention ทำให้ decode ใหม่ได้ แต่ไม่ทำให้ incomplete archive เป็นงานเสร็จ ค่า raw encoded/offset ที่อ่านไม่ได้ต้องมี warning ไม่แต่ง metadata หรืออ้าง 100% extraction ตัวเลือก runtime ขั้นสุดท้ายขึ้นกับ [decoder assessment](../runs-rebuild/decoder-assessment.md) และ DEC-08–10/ARCH-01; รูปแบบ subprocess ไม่เปิด network service ใหม่โดยไม่มีเหตุผล
