---
status: proposed
---

# เก็บ Original FIT ใน PostgreSQL เดียวกับ Runs revisions

เสนอ immutable `BYTEA` source พร้อม owner/SHA-256/size/provenance และ unique `(owner, hash)` ใน PostgreSQL ที่มี persistent storage อยู่แล้ว ใช้ transactions สำหรับ source, dedup, staged revisions และ delete; ไม่เก็บ ZIP ถาวรหรือ FIT ใต้ static public root Backup/restore PostgreSQL รวม original bytes และ revision references ใน snapshot เดียว

เลือกแทน private filesystem/object store ในช่วงแรกเพื่อตัด distributed DB/blob commit, orphan repair และบริการเพิ่ม แลกกับ DB/WAL/backup ที่โตและการอ่าน binary จาก DB การเลือกนี้ต้องผ่าน target memory/import/storage/restore budgets ไม่ใช่ claim scale จาก fixture 20 MiB limit ต่อไฟล์เป็น starting constraint ไม่ใช่ capacity estimate ถ้า budgets ไม่ผ่านให้ทบทวน object storage โดยคง owner/hash/immutable source contract

Revision retention และ temp/export cleanup ต้องไม่เก็บสำเนาของ activity ที่ผู้ใช้ลบผ่าน materialized evidence ไม่มี deploy/startup reset (IMP-05, ARCH-02, REP-05, OPS-04–05). รายละเอียดใน [architecture](../runs-rebuild/architecture.md)
