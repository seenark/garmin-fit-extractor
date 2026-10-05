---
status: proposed
---

# แยก Runs schema ใหม่จาก preserved CLI และ FIT Coach contracts

เสนอ versioned decoded/normalized/export schemas ใหม่และ Runs routes แยกจาก CLI `schemaVersion: 1.0.0`. CLI command/output และ FIT Coach `/api/v1/activities*`/OAuth contracts คงเดิมผ่าน explicit current-revision projection ไม่ freeze internal model เป็น v1 และไม่ใช้ latest-history endpoint แทน checkbox selection

Browser extraction/upload/download callers ย้ายไป new privacy-safe Runs interface ตาม announced migration; retire old Runs endpoints เมื่อ gate ผ่าน ไม่ทำ indefinite shims หรือ silent response-shape change Legacy rows ไม่มี FIT คง read-only `sourceUnavailable`/fidelity warnings หรือ reset เฉพาะเมื่อได้รับคำสั่งใหม่ Safe legacy exports ต้องผ่าน classifier/validator เดียวกัน ไม่ fallback ไป unredacted download

Existing `activities.id -> extractions.id` FK ต้องปรับอย่างเจาะจงก่อนรองรับ new stable activity source ไม่สร้าง fake extraction หรือ UUID ใหม่ระหว่าง reprocess; รักษา owner FK และ auth/admin tables ทางเลือก freeze schema เดิม/รื้อ OAuth ทำให้ source semantics หรือ protected contracts เสียโดยไม่จำเป็น (ARCH-03, REP-07, OPS-01/09). ดู [architecture](../runs-rebuild/architecture.md)
