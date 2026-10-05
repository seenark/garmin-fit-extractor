---
status: proposed
---

# ใช้ PostgreSQL jobs และ staged coherent publish

เสนอ durable job rows ใน PostgreSQL เดิม ใช้ `SKIP LOCKED`, global slots, expiring leases, deterministic input/version/config keys และ bounded retries ตัว decoder อยู่ killable subprocess ของ executable เดิมเพื่อคุม untrusted input resource limits ไม่เพิ่ม Redis/Celery/network service เพื่อ queue แรก

Version-manifest scanner + import/delete invalidation สร้างงานอัตโนมัติ ไม่รอ page load หรือ user กด reprocess; เปลี่ยน analysis ใช้ normalized revision เดิมได้ Publish ใหม่หลัง validation ด้วย transaction/CAS ที่ตรวจ desired generation/existence แล้วเปลี่ยน coherent manifest ค่าเก่าที่ยังดีคงดู/export ได้พร้อม stale/update-failed status

Delete cancellation/tombstones และ export read leases ต้องมี linearization/race tests ไม่มี worker resurrection หรือ complete JSON จาก source ที่ถูก revoke ก่อน guarded response เสร็จ ข้อมูลที่ส่งสำเร็จก่อน deletion ไม่สามารถเรียกคืนได้ Proposed concurrency/retry/TTL ต้องวัดและอนุมัติก่อน release (ARCH-04, REP-01–06, EXP-08/10, OPS-05). ดู [architecture](../runs-rebuild/architecture.md)
