# Runs v2: browser workflow

## ขอบเขต

หน้า Runs ใช้ authenticated `/api/v2/runs` เท่านั้นสำหรับ import, list, detail, reprocess, delete, evidence และ selected exports URL รายละเอียดเดิม `/extractions/:id` ยังเปิด stable activity ID ได้ ไม่ใช้ endpoint ส่งออก extraction เดิมเป็นทางลัดข้าม privacy policy หน้า Shoes, Google authentication, logout และ admin ใช้เส้นทางเดิมโดยไม่เปลี่ยนความหมาย

## Import และประวัติ

รับ Original FIT และ ZIP โดยไม่สนตัวพิมพ์ของนามสกุล ครั้งละ 1–10 ไฟล์ ไฟล์ละไม่เกิน 20 MiB การตรวจใน browser ไม่แทนการตรวจเนื้อหาและ bounds ฝั่ง server ผลแต่ละไฟล์หรือ ZIP member แสดง `imported`, `duplicate`, `unsupported` หรือ `failed` พร้อมเหตุผลและ warnings แยกจากสถานะ queued/processing ของกิจกรรม การล้มเหลวบางรายการไม่ซ่อนรายการที่สำเร็จ และไม่สร้างกิจกรรมปลอมให้รายการที่ไม่รองรับ

ประวัติเรียงตาม start time ของกิจกรรม ไม่ใช่ upload time Checkbox เป็นการเลือกส่งออกอย่างชัดเจนและไม่เปลี่ยนหลักฐานที่ LT engine ใช้ การเลือกหน้านี้เพิ่มเฉพาะรายการบนหน้าปัจจุบัน การเปลี่ยนหน้าและการเรียงลำดับไม่เลือกเพิ่มหรือถอดรายการให้เอง ปุ่มล้างการเลือกเป็นวิธีล้าง selection ที่ชัดเจน เมื่อ selected ID ถูกลบหรือ server ปฏิเสธ export รายการที่เลือกยังอยู่เพื่อให้ผู้ใช้แก้ selection เอง

การเลือกในประวัติยังอยู่เมื่อเปิดรายละเอียดแล้วกลับมาประวัติภายในช่วงที่ผู้ใช้เดียวกันเข้าสู่ระบบอยู่ Selection อยู่ใน authenticated layout และแยกตาม user ID เมื่อออกจากระบบ ไม่มีผู้ใช้ที่เข้าสู่ระบบ หรือเปลี่ยนเจ้าของบัญชี การเลือกจะถูกล้าง รวมถึงการออกจากระบบบนหน้าสาธารณะที่ยังแสดงเนื้อหาได้ ไม่บันทึก selection ใน storage และไม่รับประกันว่าจะคงอยู่หลังโหลดหน้าใหม่หรือออกจาก authenticated layout

การนำทางรายละเอียดอ่าน search จาก current router location ไม่ใช่ loader match ที่อาจยังค้างจากหน้าก่อนระหว่างรอโหลด จึงรักษา `order` และ `offset` ของ URL ปัจจุบันไว้ในลิงก์และลิงก์กลับประวัติ Regression test หน่วง ascending list response และคลิกลิงก์เดิมขณะ pending เพื่อไม่ซ่อนปัญหาด้วยการรอให้ href เปลี่ยนก่อน จากนั้นตรวจ checkbox และจำนวนที่เลือกหลังกลับประวัติ ตรวจการล้าง selection อย่างชัดเจน และตรวจว่าเจ้าของบัญชีใหม่ไม่รับ selection เดิมเมื่อ detail polling โหลดข้อมูลผู้ใช้ใหม่โดยไม่โหลดเอกสารทั้งหน้า

## Detail, charts และ LT

รายละเอียดอ่าน coherent revision และ normalized streams ไม่โหลด decoded archive เพื่อวาดกราฟ Pace, HR และ Power ใช้แกนเวลาเดียวกัน มี inspector จาก samples ความละเอียดต้นฉบับ การซูม คืนช่วงเวลา และแยก Garmin laps จาก detected segments พร้อม target-specific eligibility/reasons ข้อมูลที่ขาด zero speed, timer pauses และช่องว่างเกิน 10 วินาทีไม่ถูกวาดเป็นเส้นต่อเนื่อง การลดจุดเป็น display-only boundary-preserving min/max buckets มี original/display count และชื่อวิธีบนหน้าจอ

Timer events ใช้เลข enum จาก native normalized wire: `event: 0` คือ timer, `eventType: 0` คือ start และ `1`, `4`, `8`, `9` คือ stop เส้นกราฟตัดช่วง stop/start แม้ห่างกันไม่เกิน 10 วินาทีและไม่มี sample ระหว่างหยุด ค่า event type อื่นหรือ null คงสถานะ UNKNOWN ไม่ถือว่าเป็น start

Inspector จำ `sample.index` ของ record ต้นฉบับ ไม่ใช้ elapsed time เป็น identity จึงเลือก record ที่ timestamp ซ้ำแยกกันได้ด้วย slider และลูกศร โดยไม่สร้างเวลาหรือ deduplicate ข้อมูล Pointer ใช้ค่า metric ต้นฉบับแยกจุดเวลาเดียวกัน และ cursor ของ Pace, HR, Power ยังคงเชื่อมด้วย elapsed time ของ record ที่เลือก

ผล LT ใน detail เป็น historical snapshot ณ evidence cutoff ของกิจกรรมนั้น ไม่แทนด้วยผลล่าสุดของบัญชี LT1 และ LT2 ใช้ status `estimated`, `low_confidence` หรือ `insufficient_data` แยกกัน engine availability, experimental/research gate, context limitations, freshness และ job failure เป็นคนละข้อมูล ค่าระบบระบุว่าเป็น VT proxy ไม่ใช่ validated blood-lactate measurement ค่า device-reported แสดงแยกพร้อม provenance

หน้า history แสดง latest attempt แยกจาก lastAvailable ของแต่ละ target ผลเก่าบอก activity ID, evidence cutoff, computed time และ stale อย่างชัดเจน ไม่ยกค่าจาก cutoff เก่าขึ้นเป็นค่าปัจจุบัน Trend เรียงตาม evidence cutoff ไม่ใช่ computed time ผู้ใช้เปิดดู method, parameters, actual trace, counts, input revision/hash, reasons และ limitations ได้ Suggestions เป็นตัวเลือกที่ปิดได้ ไม่ใช่ maximal prescription

Suggestions ของ LT1 และ LT2 ซ่อนและเปิดอีกครั้งได้แยกกัน มีปุ่มระบุ target ใช้งานด้วย keyboard และ `aria-expanded`/`aria-controls` เนื้อหาและเหตุผลต้นฉบับคงเดิมเมื่อเปิดอีกครั้ง การซ่อน target หนึ่งไม่ซ่อนอีก target

History และ detail ตรวจสถานะใหม่ทุก 3 วินาทีเมื่อกิจกรรมยัง queued/processing หรือ historical stage ยัง pending โดยตรวจต่อแม้ normalized revision พร้อมแล้ว หาก update failed จะแยกข้อผิดพลาดจากผลก่อนหน้าที่ stale กิจกรรมเดิมที่ไม่มี Original FIT ระบุ `sourceUnavailable` และข้อจำกัดอย่างตรงไปตรงมา ไม่มีการสร้าง FIT, normalized streams หรือ trace ที่ไม่เคยมีขึ้นมา และไม่เปิด reprocess

History refresh ใช้การโหลดข้อมูลแบบคงหน้าปัจจุบัน ทั้ง timer, หลังลบ และปุ่มลองโหลดอีกครั้ง หาก list request ล้มเหลวจะแสดงข้อผิดพลาดและเก็บข้อมูลล่าสุดที่โหลดสำเร็จไว้ ไม่ unmount หน้าที่มี selection และ pinned export token เมื่อ navigation หรือ loader ใหม่เข้ามา ผลจาก refresh เก่าจะไม่เขียนทับหน้าหรือการเรียงลำดับใหม่

## Coach JSON และ Full JSON

ทั้งสองโหมดใช้ explicit selection เดียวกัน แต่สร้าง snapshot token ของแต่ละโหมด การเตรียม snapshot แสดงจำนวนกิจกรรม ขนาด bytes, fixed generatedAt และเวลาหมดอายุ ก่อนโหลดข้อมูลเพื่อ Copy ตัวเลือก location กับ device identifiers เป็นคนละ checkbox และปิดไว้เริ่มต้น การเปลี่ยน selection หรือ privacy ทำให้ต้องเตรียม snapshot ใหม่

เมื่อรายการที่เลือกบนหน้าปัจจุบันมี `processing.historyStatus` เป็น pending หรือ failed จะยังไม่เปิด Prepare หรือสร้าง snapshot ใหม่ จนผลย้อนหลังพร้อม โดยไม่ล้าง selection หรือเรียกสถานะนี้ว่า insufficient_data รายการนอกหน้าปัจจุบันยังต้องผ่านการตรวจครบทุก ID ฝั่ง server; `EXPORT_NOT_READY` เป็นข้อผิดพลาดทั้ง selection ไม่ใช่การส่งออกบางส่วน Snapshot ที่เตรียมไว้แล้วไม่ถูกล้างเพียงเพราะ historical stage เปลี่ยน และ Copy/Download ยังคง GET token เดิมให้ server ตรวจทุกครั้ง

Copy และ Download ใช้ token และ owner URL เดียวกันของโหมดนั้น โดย HEAD ตรวจ owner/expiry/revocation ก่อนแต่ละ action และ GET ตรวจซ้ำเมื่ออ่าน bytes จริง HEAD ไม่รับประกันว่า GET ในอนาคตจะสำเร็จ ไม่เปลี่ยน token หรือส่งออกเฉพาะ subset เงียบ ๆ Copy อ่าน server text ตาม bytes ของ pretty JSON พร้อม newline แล้วส่งให้ Clipboard API โดยไม่ parse/stringify หรือ cache ข้อความขนาดใหญ่ Download ส่ง owner URL เดิมให้ native browser download โดยไม่สร้าง Blob หรือโหลด JSON ทั้งหมดใน JavaScript ดังนั้น pinned revisions และ generatedAt ไม่เปลี่ยนเพราะ activity reprocess หน้าจอระบุเพียงว่าส่งคำขอดาวน์โหลดแล้ว ไม่อ้างว่าไฟล์ถูกบันทึกครบ

Clipboard denial และข้อผิดพลาดจริงในการอ่าน response เป็นข้อความแสดงเหตุผลอย่างตรงไปตรงมา ไม่อ้างว่า Copy สำเร็จ และยังใช้ Download snapshot เดิมแบบเต็มได้ ไม่มี Copy cap ที่ตั้งขึ้นโดยไม่มีหลักฐาน Server errors ไม่ล้าง selection รายการ `privacyOmissions` ใช้ metadata ของ pinned snapshot จาก POST หลัง HEAD ตรวจ token แล้ว แสดง category, count, pathPattern และ reason โดยไม่อ่านหรือ parse archive ทั้งหมดและไม่แสดงค่าที่ซ่อนไว้

ChatGPT และ Claude prompt templates แก้แยกกัน Copy ใช้ข้อความปัจจุบันใน textarea ตามที่ผู้ใช้แก้ ไม่ฝัง prompt ใน JSON ไม่มี LLM network request หรือ API key หาก clipboard ใช้ไม่ได้ผู้ใช้เลือกข้อความใน textarea เพื่อคัดลอกเองได้

## PNG และภาพจากเครื่อง

ตัวเลข summary ใน detail และ PNG ใช้ rounding helper เดียวกัน PNG ตรึง coherent revision จนผู้ใช้กดใช้ revision ใหม่ มี square 1080×1080 และ portrait 1080×1920 พร้อม layouts ซ้ายล่าง กลางล่าง และขวาล่าง Light, Dark และ Transparent ทำงานได้โดยไม่ใส่ภาพ Transparent ไม่วาดพื้นหลัง checkerboard ลง PNG โดยมี alpha จริง ค่าเริ่มต้นเป็น distance, Timer time และ pace ส่วน HR/Power เลือกเพิ่มได้ ค่าไม่ทราบแสดง `—` ไม่ใช่ศูนย์ วันที่และชื่อที่กรอกเองต้องเปิดแสดง และไม่ใช้ upload filename

JPEG/PNG/WebP จากเครื่องจำกัด 10 MiB และ 24 ล้านพิกเซล ตรวจ format/dimensions ก่อน image allocation ใช้ native orientation และ crop position ของผู้ใช้ ไม่มี photo upload, EXIF copying, chart, route หรือข้อมูลบัญชีใน PNG เปลี่ยน preset แล้วรักษา metric choices และปิด image resources เมื่อไม่ใช้

## Verification boundaries

Focused tests ครอบคลุม upload bounds, snapshot/revocation/privacy lifecycle, historical/target display, gap-preserving chart data และ PNG header/rounding seams Browser consumer tests ใช้ explicit HTTP fixtures เพื่อตรวจ pending-navigation race, selection persistence, exact snapshot bytes, clipboard errors และกราฟจริง Native Canvas smoke ตรวจ PNG pixels, alpha, dimensions, orientation/crop, layouts และการดาวน์โหลดจริง

ผลดังกล่าวไม่ใช่หลักฐานว่า backend จริงหรือ protected compatibility ผ่านแล้ว Actual FIT/ZIP import, 3,600/100,000 sample fixtures, desktop/mobile full workflow, Shoes/auth/admin compatibility และ release budgets ตรวจหลังรวม backend dependencies ใน disposable integration environment เท่านั้น ไม่มี production reset หรือ deployment ใน workflow นี้ ผลคำสั่ง screenshot paths และข้อจำกัดที่สังเกตจริงอยู่ในรายงาน worker/integration ไม่ใช้เอกสารนี้แทน runtime evidence
