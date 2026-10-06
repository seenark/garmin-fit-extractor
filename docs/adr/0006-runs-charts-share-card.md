---
status: accepted
---

# ใช้ chart library เดิมและ browser Canvas สำหรับ numeric PNG

Runs v2 ใช้ `@tanstack/charts` 0.16.0 ที่มีอยู่เดิมสำหรับกราฟ Pace, HR และ Power ที่ใช้แกนเวลาเดียวกัน มีจุดตรวจข้อมูลจาก samples ความละเอียดต้นฉบับ การซูมและคืนช่วงเวลา พร้อมเส้น Garmin laps และ overlays ของ detected segments ที่แยกกันอย่างชัดเจน ข้อมูลที่ขาด timer pause และช่องว่างเกิน 10 วินาทีทำให้เส้นกราฟขาด ไม่เชื่อมเส้นหรือเติมศูนย์แทนข้อมูลที่ไม่ทราบ

การลดจุดทำเฉพาะการวาดด้วย boundary-preserving min/max buckets พร้อม original/display count และชื่อวิธีบนหน้าจอ จุดตรวจข้อมูลอ่าน samples ต้นฉบับ ไม่ใช้จุดที่ลดแล้วแทน numerical inputs หรือ Full JSON ขอบเขต lap, segment, timer event และการเปลี่ยนจากค่าที่ทราบเป็นค่าที่ขาดต้องยังอยู่ แม้ทำให้จำนวนจุดเกินเป้าหมายการวาด

Share Card ใช้ native browser Canvas สร้าง PNG จาก summary ของ coherent revision ที่ผู้ใช้ตรึงไว้ ไม่ถ่ายภาพหน้าจอและไม่เพิ่ม renderer service มี layouts ซ้ายล่างแบบเรียงแนวตั้ง กลางล่างแบบสองคอลัมน์ และขวาล่างแบบชิดขวา ขนาด 1080×1080 และ 1080×1920 รองรับ Light, Dark และ Transparent ที่ไม่วาดพื้นหลังลง alpha channel ตัวเลือกค่าที่แสดงไม่เปลี่ยนเมื่อเปลี่ยน layout วันที่และชื่อที่ผู้ใช้กรอกเองต้องเปิดแสดงอย่างชัดเจน ไม่มีชื่อไฟล์นำเข้า ข้อมูลบัญชี เส้นทาง หรือกราฟในภาพ

ภาพพื้นหลังเป็น JPEG, PNG หรือ WebP จากเครื่องผู้ใช้เท่านั้น จำกัด 10 MiB และ 24 ล้านพิกเซล ตรวจ magic bytes และขนาดจาก header ก่อนเรียก image decoder จากนั้นตรวจขนาดที่ถอดรหัสอีกครั้ง Native image decoding จัด orientation และ Canvas ใช้ crop position ที่ผู้ใช้เลือก PNG ใหม่มี raster pixels เท่านั้น ไม่คัดลอก EXIF และไม่ส่งภาพไป API เมื่อเปลี่ยนภาพหรือออกจากหน้าจะปิด ImageBitmap และยกเลิก object URLs ของการดาวน์โหลด

ตัวเลขในหน้า detail และ PNG ใช้ formatter เดียวกัน สีและ font ของ Canvas อ่านจาก design tokens เดิม ไม่เพิ่ม fonts หรือธีมของเว็บใหม่ ความสำเร็จที่แสดงหมายถึง browser ยอมรับข้อมูล clipboard หรือรับไฟล์เพื่อดาวน์โหลด ไม่อ้างว่าไฟล์ถูกบันทึกบนเครื่องแล้ว

การพิสูจน์ renderer, alpha pixels, orientation/crop และ downloads เป็นคนละขั้นกับการพิสูจน์ข้อมูลจาก backend จริง รายงานผลแยกทั้งสองอย่างไว้ใน [web workflow](../runs-rebuild/web-workflow.md) ส่วน release gates และ resource budgets ที่ยังไม่ได้วัดยังใช้ [verification plan](../runs-rebuild/verification-plan.md) ไม่ถือการผ่าน tests เฉพาะ frontend ว่าผ่าน protected workflow หรือ deployment budget แล้ว
