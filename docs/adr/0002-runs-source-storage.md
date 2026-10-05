---
status: accepted
---

# เก็บ Original FIT ใน PostgreSQL เดียวกับ Runs revisions

เก็บ immutable `BYTEA` source พร้อม owner/SHA-256/size/provenance และ unique `(owner, hash)` ใน PostgreSQL ที่มี persistent storage อยู่แล้ว ใช้ transactions สำหรับ source, dedup, immutable 64 KiB revision chunks และ coherent manifest; ไม่เก็บ ZIP ถาวรหรือ FIT ใต้ static public root Backup/restore PostgreSQL รวม original bytes และ revision references ใน snapshot เดียว โดย restore proof เป็น release gate ไม่ใช่ผลที่อนุมานจาก unit tests

เลือกแทน private filesystem/object store เพื่อตัด distributed DB/blob commit, orphan repair และบริการเพิ่ม แลกกับ DB/WAL/backup ที่โตและการอ่าน binary จาก DB Source FIT จำกัด 20 MiB และ private decoder document รวม numerical projection ใช้ budget เดิม 512 MiB; streaming spans, bounded readers และ binary COPY ลดสำเนาโดยไม่ลด sample/RR resolution การตัดสินใจนี้ไม่ใช่ capacity claim และต้องผ่าน native Linux/100k/storage/restore measurements ของ final integration หาก budgets ไม่ผ่านให้ทบทวน object storage โดยคง owner/hash/immutable source contract

Ordinary reprocess เก็บ coherent revision และ pinned export ที่ยังไม่หมดอายุ; DELETE revoke dependent snapshots ก่อนรอ readers และตรวจ dependency ซ้ำภายใต้ owner lock แล้วลบ source/chunks/materialized evidence/receipts เพื่อไม่เก็บข้อมูลที่ถูกลบ ไม่มี deploy/startup reset (IMP-05, ARCH-02, REP-05, OPS-04–05). รายละเอียดใน [runtime contract](../runs-rebuild/runtime-contract.md) และ [architecture](../runs-rebuild/architecture.md)
