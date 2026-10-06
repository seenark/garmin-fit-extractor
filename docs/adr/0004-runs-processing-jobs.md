---
status: accepted
---

# ใช้ PostgreSQL jobs และ staged coherent publish

ใช้ durable job rows ใน PostgreSQL เดิมกับ `SKIP LOCKED`, global slots, expiring original-holder leases, deterministic input/version/config keys และ bounded retries ตัว decoder อยู่ killable subprocess ของ executable เดิมก่อน Tokio/config/DB เพื่อคุม untrusted input ไม่เพิ่ม Redis/Celery/network service Linux limits คง AS/FSIZE 512 MiB, CPU 60 seconds, one process, no core dump และ parent wall timeout 60 seconds; descriptor stdout จำกัด 64 KiB ไม่ใช้ full-archive Value bridge

Version-manifest scanner และ import/delete invalidation สร้างงานอัตโนมัติ ไม่รอ page load หรือ user กด reprocess; analysis-only change ใช้ compatible normalized revision เดิมได้ Publish coherent manifest และ completion ของ original-holder job ใน transaction/CAS เดียวที่ตรวจ generation/existence/worker holder เพื่อรองรับ worker loss History epochs แยกจาก processing และ pending history ของ retained manifest ต้องทำงานต่อเมื่อ reprocess ล้มเหลว ไม่แทนด้วย placeholder ready/null คง published estimates และ existing pinned exports ระหว่าง ordinary updates

Blocking CPU และ private artifacts ถือ shared admission จน computation จบจริงแม้ requesting future ถูก drop; stale cleanup เปลี่ยน holder ใหม่ไม่ได้ DELETE tombstones/revokes ก่อนรอ readers และตรวจ dependency ซ้ำภายใต้ owner lock เพื่อจับ concurrently committed exports GET lease แยกจาก 15-minute snapshot TTL: `min(remainingTTL, 30s + ceil(byteLength / 5 MiB))`; HEAD ไม่ acquire/renew lease Midstream revoke/expiry/missing chunk ต้องเป็น body error ไม่ใช่ clean EOF และ worker ห้าม resurrect source ข้อมูลที่ส่งก่อน deletion เรียกคืนไม่ได้ Final integration เป็นเจ้าของ native Linux/100k/browser/resource/deploy proof (ARCH-04, REP-01–06, EXP-08/10, OPS-05). ดู [runtime contract](../runs-rebuild/runtime-contract.md) และ [architecture](../runs-rebuild/architecture.md)
