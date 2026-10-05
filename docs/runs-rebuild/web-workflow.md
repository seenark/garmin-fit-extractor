# Runs v2: browser workflow

## ขอบเขต

หน้า Runs ใช้ authenticated `/api/v2/runs` เท่านั้นสำหรับ import, list, detail, reprocess, delete, evidence และ selected exports URL รายละเอียดเดิม `/extractions/:id` ยังเปิด stable activity ID ได้ ไม่ใช้ endpoint ส่งออก extraction เดิมเป็นทางลัดข้าม privacy policy หน้า Shoes, Google authentication, logout และ admin ใช้เส้นทางเดิมโดยไม่เปลี่ยนความหมาย

## Import และประวัติ

รับ Original FIT และ ZIP โดยไม่สนตัวพิมพ์ของนามสกุล ครั้งละ 1–10 ไฟล์ ไฟล์ละไม่เกิน 20 MiB การตรวจใน browser ไม่แทนการตรวจเนื้อหาและ bounds ฝั่ง server ผลแต่ละไฟล์หรือ ZIP member แสดง `imported`, `duplicate`, `unsupported` หรือ `failed` พร้อมเหตุผลและ warnings แยกจากสถานะ queued/processing ของกิจกรรม การล้มเหลวบางรายการไม่ซ่อนรายการที่สำเร็จ และไม่สร้างกิจกรรมปลอมให้รายการที่ไม่รองรับ

ประวัติเรียงตาม start time ของกิจกรรม ไม่ใช่ upload time Checkbox เป็นการเลือกส่งออกอย่างชัดเจนและไม่เปลี่ยนหลักฐานที่ LT engine ใช้ การเลือกหน้านี้เพิ่มเฉพาะรายการบนหน้าปัจจุบัน การเปลี่ยนหน้าและการเรียงลำดับไม่เลือกเพิ่มหรือถอดรายการให้เอง ปุ่มล้างการเลือกเป็นวิธีล้าง selection ที่ชัดเจน เมื่อ selected ID ถูกลบหรือ server ปฏิเสธ export รายการที่เลือกยังอยู่เพื่อให้ผู้ใช้แก้ selection เอง

การนำทางรายละเอียดอ่าน search จาก current router location ไม่ใช่ loader match ที่อาจยังค้างจากหน้าก่อนระหว่างรอโหลด จึงรักษา `order` และ `offset` ของ URL ปัจจุบันไว้ในลิงก์และลิงก์กลับประวัติ Regression test หน่วง ascending list response และคลิกลิงก์เดิมขณะ pending เพื่อไม่ซ่อนปัญหาด้วยการรอให้ href เปลี่ยนก่อน

## Detail, charts และ LT

รายละเอียดอ่าน coherent revision และ normalized streams ไม่โหลด decoded archive เพื่อวาดกราฟ Pace, HR และ Power ใช้แกนเวลาเดียวกัน มี inspector จาก samples ความละเอียดต้นฉบับ การซูม คืนช่วงเวลา และแยก Garmin laps จาก detected segments พร้อม target-specific eligibility/reasons ข้อมูลที่ขาด zero speed, timer pauses และช่องว่างเกิน 10 วินาทีไม่ถูกวาดเป็นเส้นต่อเนื่อง การลดจุดเป็น display-only boundary-preserving min/max buckets มี original/display count และชื่อวิธีบนหน้าจอ

ผล LT ใน detail เป็น historical snapshot ณ evidence cutoff ของกิจกรรมนั้น ไม่แทนด้วยผลล่าสุดของบัญชี LT1 และ LT2 ใช้ status `estimated`, `low_confidence` หรือ `insufficient_data` แยกกัน engine availability, experimental/research gate, context limitations, freshness และ job failure เป็นคนละข้อมูล ค่าระบบระบุว่าเป็น VT proxy ไม่ใช่ validated blood-lactate measurement ค่า device-reported แสดงแยกพร้อม provenance

หน้า history แสดง latest attempt แยกจาก lastAvailable ของแต่ละ target ผลเก่าบอก activity ID, evidence cutoff, computed time และ stale อย่างชัดเจน ไม่ยกค่าจาก cutoff เก่าขึ้นเป็นค่าปัจจุบัน Trend เรียงตาม evidence cutoff ไม่ใช่ computed time ผู้ใช้เปิดดู method, parameters, actual trace, counts, input revision/hash, reasons และ limitations ได้ Suggestions เป็นตัวเลือกที่ปิดได้ ไม่ใช่ maximal prescription

Queued/processing detail ตรวจสถานะใหม่ทุก 3 วินาที ระหว่าง historical stage ยัง pending จะตรวจสถานะต่อแม้ normalized revision พร้อมแล้ว หาก update failed จะแยกข้อผิดพลาดจากผลก่อนหน้าที่ stale กิจกรรมเดิมที่ไม่มี Original FIT ระบุ `sourceUnavailable` และข้อจำกัดอย่างตรงไปตรงมา ไม่มีการสร้าง FIT, normalized streams หรือ trace ที่ไม่เคยมีขึ้นมา และไม่เปิด reprocess

## Coach JSON และ Full JSON

ทั้งสองโหมดใช้ explicit selection เดียวกัน แต่สร้าง snapshot token ของแต่ละโหมด การเตรียม snapshot แสดงจำนวนกิจกรรม ขนาด bytes, fixed generatedAt และเวลาหมดอายุ ก่อนโหลดข้อมูลเพื่อ Copy ตัวเลือก location กับ device identifiers เป็นคนละ checkbox และปิดไว้เริ่มต้น การเปลี่ยน selection หรือ privacy ทำให้ต้องเตรียม snapshot ใหม่

Copy และ Download ใช้ token เดียวกันของโหมดนั้น และ GET token ทุกครั้งเพื่อให้ server ตรวจ owner/expiry/revocation แม้เคยเปิด preview แล้ว ไม่เปลี่ยน token หรือส่งออกเฉพาะ subset เงียบ ๆ Copy ใช้ server text ตาม bytes ของ pretty JSON พร้อม newline ไม่ parse แล้ว stringify ใหม่ Download ใช้ Blob จาก response ของ token เดียวกัน ดังนั้น pinned revisions และ generatedAt ไม่เปลี่ยนเพราะ activity reprocess

Clipboard denial แสดงข้อผิดพลาดจริงและยัง Download snapshot เดิมได้ Server errors ไม่ล้าง selection รายการ `privacyOmissions` แสดง category, count, pathPattern และ reason จาก snapshot จริง ไม่แสดงค่าที่ซ่อนไว้ Download ทำได้โดยไม่โหลด JSON ทั้งหมดเป็นข้อความ แต่หน้าจอจะบอกว่ายังไม่ได้โหลด omission preview จนกดตรวจรายการ

ChatGPT และ Claude prompt templates แก้แยกกัน Copy ใช้ข้อความปัจจุบันใน textarea ตามที่ผู้ใช้แก้ ไม่ฝัง prompt ใน JSON ไม่มี LLM network request หรือ API key หาก clipboard ใช้ไม่ได้ผู้ใช้เลือกข้อความใน textarea เพื่อคัดลอกเองได้

## PNG และภาพจากเครื่อง

ตัวเลข summary ใน detail และ PNG ใช้ rounding helper เดียวกัน PNG ตรึง coherent revision จนผู้ใช้กดใช้ revision ใหม่ มี square 1080×1080 และ portrait 1080×1920 พร้อม layouts ซ้ายล่าง กลางล่าง และขวาล่าง Light, Dark และ Transparent ทำงานได้โดยไม่ใส่ภาพ Transparent ไม่วาดพื้นหลัง checkerboard ลง PNG โดยมี alpha จริง ค่าเริ่มต้นเป็น distance, Timer time และ pace ส่วน HR/Power เลือกเพิ่มได้ ค่าไม่ทราบแสดง `—` ไม่ใช่ศูนย์ วันที่และชื่อที่กรอกเองต้องเปิดแสดง และไม่ใช้ upload filename

JPEG/PNG/WebP จากเครื่องจำกัด 10 MiB และ 24 ล้านพิกเซล ตรวจ format/dimensions ก่อน image allocation ใช้ native orientation และ crop position ของผู้ใช้ ไม่มี photo upload, EXIF copying, chart, route หรือข้อมูลบัญชีใน PNG เปลี่ยน preset แล้วรักษา metric choices และปิด image resources เมื่อไม่ใช้

## Verification boundaries

Focused tests ครอบคลุม upload bounds, snapshot/revocation/privacy lifecycle, historical/target display, gap-preserving chart data และ PNG header/rounding seams Browser consumer tests ใช้ explicit HTTP fixtures เพื่อตรวจ pending-navigation race, selection persistence, exact snapshot bytes, clipboard errors และกราฟจริง Native Canvas smoke ตรวจ PNG pixels, alpha, dimensions, orientation/crop, layouts และการดาวน์โหลดจริง

ผลดังกล่าวไม่ใช่หลักฐานว่า backend จริงหรือ protected compatibility ผ่านแล้ว Actual FIT/ZIP import, 3,600/100,000 sample fixtures, desktop/mobile full workflow, Shoes/auth/admin compatibility และ release budgets ตรวจหลังรวม backend dependencies ใน disposable integration environment เท่านั้น ไม่มี production reset หรือ deployment ใน workflow นี้ ผลคำสั่ง screenshot paths และข้อจำกัดที่สังเกตจริงอยู่ในรายงาน worker/integration ไม่ใช้เอกสารนี้แทน runtime evidence
