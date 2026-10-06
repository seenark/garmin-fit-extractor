---
status: accepted
---

# แยก Runs schema ใหม่จาก preserved CLI และ FIT Coach contracts

ใช้ decoded/normalized/export schema `2.0.0` และ `/api/v2/runs` แยกจาก CLI `schemaVersion: 1.0.0`. CLI command/output และ FIT Coach `/api/v1/activities*`/OAuth contracts คงเดิมผ่าน explicit current-revision projection ไม่ freeze internal model เป็น v1 และไม่ใช้ latest-history endpoint แทน explicit checkbox selection

Browser extraction/upload/download callers ย้ายไป privacy-safe Runs interface; known old browser routes คืน authenticated `410 RUNS_ENDPOINT_RETIRED` และ unauthenticated `401` โดยไม่มี aliases หรือ unredacted fallback Legacy rows ไม่มี FIT คง safe summary, `sourceUnavailable` และ fidelity warnings; normalized/analysis/history/revision เป็น null, reprocess คืน `409` และ export คืน `422` ไม่สร้าง source หรือ dates ปลอมและไม่ reset ข้อมูล

ปรับ existing `activities.id -> extractions.id` FK อย่างเจาะจง โดยรักษา owner FK และ auth/admin tables; new stable source IDs ไม่สร้าง fake extraction และ reprocess ไม่เปลี่ยน activity UUID ทางเลือก freeze schema เดิม/รื้อ OAuth ทำให้ source semantics หรือ protected contracts เสียโดยไม่จำเป็น Latest ใช้ independent numeric `lastAvailable.lt1/lt2` พร้อม provenance และ stale state ไม่มี `lastGood` alias (ARCH-03, REP-07, OPS-01/09). ดู [runtime contract](../runs-rebuild/runtime-contract.md) และ [architecture](../runs-rebuild/architecture.md)
