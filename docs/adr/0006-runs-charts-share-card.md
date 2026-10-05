---
status: proposed
---

# ใช้ chart library เดิมและ browser Canvas สำหรับ numeric PNG

เสนอ reuse `@tanstack/charts` 0.16.0 หลัง spikeยืนยัน missing-gap rendering, linked inspection/zoom/mobile และ lap/segment overlays การ downsampleเพื่อวาดเท่านั้นไม่แตะ native archiveหรือ numerical inputs ไม่เพิ่ม chart library ก่อนพิสูจน์ capability gap

Share Cardใช้ native Canvas PNG ที่ owner browser จาก pinned revision; Light/Dark/real-alpha Transparent และ optional safe raster background ทำงานโดยไม่มี upload/AI/server image service เริ่มสาม layouts และ square/portraitเป็น proposed geometry ไม่ใช่ Instagram specification Shared roundingกับ activity preview, Thai/English long metrics, alpha/contrast/orientation/pixel limits ต้องผ่าน actual visual proof

เลือกแทน screenshot/dashboard exportเพื่อไม่ bake charts/checkerboard/EXIF หรือข้อมูลบัญชีลงภาพ และไม่เพิ่ม dependenciesสำหรับ Canvasที่ platformมีอยู่แล้ว Trade-offคือ font loading/browser raster/image allocationต้องมี measured budgets และ fallback errorที่ตรงจริง ไม่ fake success (ARCH-06, CHART-01–06, SHARE-01–09). ดู [verification plan](../runs-rebuild/verification-plan.md)
