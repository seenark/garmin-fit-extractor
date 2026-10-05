# การประเมิน decoder สำหรับ Runs rebuild

สถานะ: **implemented at the public Runs decode seam; release gates remain open** มี implementation ของ Rust decoder/normalization และ corpus สังเคราะห์ที่เก็บถาวรแล้วตามหลักฐานด้านล่าง ผล compatibility spike เดิมยังเป็นหลักฐานเฉพาะกรณี ไม่ใช่การรับรอง full fidelity, physiological validity หรือ deployment budgets งานนี้ไม่เปิด FIT ส่วนตัว, `.env`, DB หรือ production และไม่รัน official Garmin SDK เพิ่ม

## ข้อเสนอและข้อจำกัดในการตัดสินใจ

เสนอให้คง API และ worker ของ Rust แล้วปรับชั้น decode ที่ใช้ `fitparser` เป็นตัวเลือกแรก ไม่ย้าย API ไป Python เพียงเพื่อเปลี่ยน parser Dependency นี้มีอยู่แล้ว รองรับ compressed timestamps ที่ทดลอง และแยก deserialization จาก profile decoding ได้ อย่างไรก็ตาม **fitparser 0.11.0 ที่ใช้อยู่ยังไม่ผ่าน full-fidelity gate**: ทดลองพบ HR component expansion สร้าง event ที่ไม่มีใน wire payloadและไม่ carry anchor จึงต้องแก้ต้นเหตุใน component generator/accumulation พร้อม metadata retention ก่อน cutover ถ้า patch ที่ดูแลได้ไม่ผ่าน acceptance ให้พิจารณา fitdecode ใน isolated decode worker โดยคง Rust API ไม่บังคับ rewrite ทั้งระบบ

`fitdecode` เป็นเครื่องมือตรวจสอบอิสระที่มีประโยชน์ เพราะเปิดเผย frame/definition, raw chunks, typed field, raw value และ expanded field ในลำดับการอ่าน แต่การใช้เป็น production worker ต้องมีเหตุผลจาก fidelity gate ไม่ใช่สมมติว่า Python ดีกว่า และต้องจัดการ processor ที่แปลง HR timestamp เป็นพิเศษ

official `garmin-fit-sdk` Python และ `@garmin/fitsdk` JS ที่ pin ใน spike **ไม่ผ่าน gate compressed timestamp** จึงไม่ใช่ decoder เดี่ยวที่ครอบคลุมสัญญานี้ JS SDK เป็นทางเลือก integration ที่อยู่ใน CLI เดิมแล้ว แต่ default ปิด unknown data และ default merge HR เปลี่ยนค่าที่บันทึกไว้ การเปิด listener จึงไม่เท่ากับเก็บ native archive ครบ

### Gate ด้าน license

`fitparser` และ `fitdecode` ระบุ MIT ส่วน Garmin SDK ใช้ **FIT Protocol License** ไม่ใช่ MIT/OSI license โดย license ของ JS package ที่ติดตั้งจริง §2(f) ระบุว่าไม่ให้ใช้เพื่อ “benchmarking or a competitive analysis”; Python source ระบุ FIT Protocol License เช่นกัน [S8–S10]

พบข้อกำหนดนี้หลังรัน internal compatibility probe แล้ว จึงหยุดการรัน SDK เพิ่ม **ไม่เผยแพร่ตัวเลขเวลา/หน่วยความจำของ official SDK ใน repository** และไม่จัดอันดับความเร็ว เก็บ diagnostics ไว้ในพื้นที่ spike ภายในเท่านั้น ไม่อ้างสิทธิ์ในการเผยแพร่ benchmark หรือแจกจ่าย SDK ตั้ง legal review เป็น hard gate ก่อนเผยแพร่การเปรียบเทียบ compatibility ภายนอก, รัน benchmark ซ้ำ, vendor SDK หรือเลือก SDK สำหรับ distribution ผล correctness ไม่ใช่หลักฐานว่ามี legal permission

## เวอร์ชันและ provenance ที่ตรวจจริง

- Repository ระบุ `fitparser = "0.11.0"` ที่ `apps/api/Cargo.toml:17`; `Cargo.lock:762–765` resolve เป็น **0.11.0** และ checksum `56a834aed7c01a500afb06ebcfde59a4dcfd1de7ce15ae501462934d59655376` ซึ่งตรงกับ crate ที่ดาวน์โหลดและตรวจ SHA-256 จริง Profile source เป็น **21.202.0**, VCS revision `fb82202b34d10c2166784db333d9d23a29b79a49` [S3]
- ติดตั้ง Python **fitdecode 0.11.0**, profile **21.171**, ด้วย Python **3.12.13** ใน venv แยก Profile header ระบุ SDK 21.171 และวันที่สร้าง 2025-08-04 [S4–S5]
- ติดตั้ง official Python **garmin-fit-sdk 21.217.0**, profile **21.217.0Release**, source tag `production/release/21.217.0-0-g248b1c46` [S6]
- CLI เดิมระบุ `@garmin/fitsdk ^21.205.0`; `bun.lock:160` resolve **21.208.0** Spike JS pin เป็น **21.208.0**, profile **21.208.0Release**, source tag `production/release/21.208.0-0-g5209d509`; ใช้ Bun **1.3.14** [S7]
- wheel SHA-256: fitdecode `a1bdb9b4d1e9ebac0001fc58c22fe07a3f9967ca4e13258c6ef99cdb8d697a78`; Garmin Python `382bc4cba7cc3e26bd6d65fbf5ad6864cf15feeafcf21dd226e2697df1063e96` บันทึกใน `requirements.txt` พร้อม `--hash` และ metadata ของ PyPI [S5–S6]
- SHA-256 ของ reader/decoder/profile source ที่ติดตั้งจริงอยู่ใน `source-hashes.json` ไม่ใช้ branch `main` เป็นตัวแทน binary ที่ทดลอง

### Corpus เดียวกัน

ใช้ corpus สุดท้าย **13 ไฟล์ รวม 1,743 bytes** กับทุก decoder: 12 ไฟล์สังเคราะห์ที่ generator อุทิศเป็น **CC0-1.0** และ fixture สาธารณะ 1 ไฟล์ภายใต้ **MIT** ไม่มีตัวอย่างจาก Garmin Connect หรือเครื่องของผู้ใช้

`public_activity.fit` มาจาก `fitparser 0.11.0` crate ตำแหน่ง `tests/fixtures/Activity.fit` ตรวจ byte-for-byte แล้วตรงกับ `apps/api/tests/fixtures/activity.fit` จำนวน 771 bytes; repository มี attribution ที่ `README.md:170–172` License upstream ณ revision ที่ pin ตรวจได้จาก [S8] ไม่ได้เปิด ZIP fixture หรือกิจกรรมอื่นใน checkout

ไฟล์สังเคราะห์มี `file_id` ก่อน data, header/profile และ CRC ที่สร้างเอง แต่เป็น **protocol-focused streams** ไม่ใช่กิจกรรมวิ่ง Garmin ที่ผ่าน semantic validation ทั้งหมด: ไม่ครอบคลุม session/sport/subtype และ deliberately ไม่เก็บ serial/application ID จริง Developer collision case ทดลอง numeric metadata index และชื่อซ้ำ ไม่ได้พิสูจน์ความครบของ application identity `sparse_hr_stress.fit` เป็น stress case ที่จงใจไม่ให้ HR merge anchor ครบ จึงไม่นับเป็นหลักฐานว่าทุก HR activity ที่ถูกต้องต้อง merge ได้

ไม่มี GPS, account/device identifiers หรือ decoded values จาก fixture สาธารณะในรายงาน มีเฉพาะจำนวน, digest และข้อสรุป fidelity ผลละเอียดอยู่ในพื้นที่ spike ที่ไม่อยู่ใน checkout และไม่ใช่ข้อมูลส่วนตัว

| Corpus | bytes | SHA-256 |
| --- | ---: | --- |
| bad_file_crc.fit | 64 | f6b2ae2fb13f728561e4dafc0b4fc820ab62e89423e37bbf0f70b78dad99b35f |
| bad_header_crc.fit | 64 | 69fcdcbb6ff96521fc409abb5b02c4dd42005ffe1ea438b297c014195092f66b |
| bad_signature.fit | 64 | c5e486d7c944f07faa663c718bc693859245805ce7ff609b740a23b3d1d92b9d |
| components.fit | 57 | 260c940fde6af7cbabf5d6695eb359d2441878438ff7d7c7409850cd2503c629 |
| compressed_timestamp.fit | 63 | e6c64e3022465d65fe071a47264069ac8f6c58559b3748f57e7a1d25b1241501 |
| developer_collisions.fit | 171 | 3165b0fc017b772358c8905b7b132a6af853768671cc0185d9597976f5f7f386 |
| header12.fit | 41 | 5f464b12cff74d2c0f924481468af1dc78cb72e7186597afa1f6cf01816c5104 |
| hr_rr_order.fit | 120 | 362d625389b5c84a3df4592c71a062f652ae3298bc7c49e53f3897c4c8309d74 |
| native_unknown.fit | 64 | c4439eab53f9db24cc755b8616d9513ac8038d129281cea52c56701243db96d9 |
| public_activity.fit | 771 | 90bf03369eb0bf70275d3c2d3f9496d2b6fea54edd6ed03be271eb02b4e9ab93 |
| sparse_hr_stress.fit | 118 | db5467daa29e3b55cc18f082a73ce1e7b543adc378c050b03c1f72006731f91c |
| truncated.fit | 61 | 9e67476437ea682f1310c8fe6d5cd4cd3f8a64b6c944298c7baa6c669827ea1a |
| unknown_arrays_integers.fit | 85 | 6f54955ae716e73dad39ef1ba32f5b589348f9a3fc67ddbccdf9ec59a58fa3a5 |

Manifest SHA-256: `a62ebeed9404233c9487b86798b96ff86f2f3ce886e77bbac3e3f3839f83a4c7`; generator SHA-256: `b315140b11c02b62ebd4790dcf7f39eec1b3a71adfa5b79f9b45fbcb2f1f4f6b` รอบแรกเป็น corpus ย่อยก่อนตรวจ protocol; ผลในตารางสุดท้ายต้องมาจาก corpus นี้เท่านั้น ไม่รวมเวลาหรือความสำเร็จจากรอบแรก

## Options และความหมายของ raw/native/expanded

### Rust fitparser

Default คือ option set ว่าง: ตรวจ header/file CRC, เก็บ unknown message/field, แปลง enum เป็นชื่อ, apply profile scale/subfields/components และไม่ merge HR Source `de/decode.rs:54–69` sort native field output ตามหมายเลข ไม่เก็บลำดับ field ตาม definition ใน decoded record; developer fields มาจาก HashMap จึงต้องไม่ใช้ตำแหน่งเป็น identity [S3]

Probe ใช้ `FitStreamProcessor.deserialize_next()` เก็บ definition กับ native typed data ก่อน `decode_message()` และทดลอง 3 modes: `default`; `archive` เปิด **KeepCompositeFields เท่านั้น**; `numeric_enum` เปิด KeepCompositeFields + ReturnNumericEnumValues แยกเป็น negative experiment ไม่เสนอใช้ร่วมกันโดยยังไม่แก้ metadata decoder

`FitDataField` เก็บ numeric field และ developer index ใน serde output แต่ developer index ไม่มี public getter ใน version นี้; `FieldDefinition` มีสมาชิก number/size/base type เป็น private ส่วน definition order เก็บอยู่ใน slice Native fields เป็น HashMap ไม่ใช่ ordered fields และ invalid scalar ถูกตัดใน parser ก่อน profile decode จึงต้องเก็บ definition/byte provenance เพิ่ม ไม่ใช้ Debug string ใน spike เป็น production API

### Python fitdecode

Default `CrcCheck.WARN`, `ErrorHandling.WARN`, `DefaultDataProcessor`, `keep_raw_chunks=False`; mode `archive` ของ spike เปลี่ยนเป็น **RAISE ทั้ง CRC และ parse errors**, `keep_raw_chunks=True` โดยยังใช้ processor เดิม ไม่ได้แปลว่า unprocessed native-only mode `FieldData.raw_value` ของ native field ยังเป็นค่าก่อน scale แต่ sentinel กลายเป็น `None`; raw chunks จำเป็นถ้าต้องการ exact sentinel bytes Expanded field มี `field_def=None`/`is_expanded=True`; อย่าเข้าใจว่า raw_value ของ expanded fieldคือ raw integer ที่เขียนอยู่ในไฟล์ [S4]

### Official Python / existing JS SDK

Default เปิด scale, enum/date conversions, subfields/components และ HR merge Listener ส่ง message หลัง options ถูกใช้แล้ว; grouped return ไม่รักษา interleaved global message order ต้องเก็บ ordered listener และ definition listener เอง

Spike `archive` ปิด scale/date/enum/subfield/component/HR merge และเปิด CRC; JS เพิ่ม `includeUnknownData:true`, `legacyArrayMode:false` Python ไม่มี option ปิด unknown data แบบ JS Developer fields ใช้ key จาก field-description sequence ไม่ใช่ชื่อ จึง join กลับกับ numeric metadata ได้โดยไม่ชนกับ native name แต่ dictionary ที่ได้รับไม่มี wire definition/invalid-presence ครบด้วยตัวเอง [S6–S7]

## ผลที่สังเกตและการตัดสินตาม spec

### จำนวนผลจาก corpus สุดท้าย

จำนวนต่อแถวรวมครบ 13 ไฟล์ “ไม่มี error” ไม่ได้แปลว่า valid import หรือ full fidelity เพราะอาจมี unknown/invalid fieldsถูกละ, header integrity false หรือ HR expansionผิด

| Decoder / mode | ไม่มี parse error | มี error | ไฟล์ที่มี warning |
| --- | ---: | ---: | ---: |
| fitparser default | 9 | 4 | 0 |
| fitparser archive: KeepCompositeFields | 9 | 4 | 0 |
| fitparser numeric_enum experiment | 8 | 5 | 0 |
| fitdecode default | 11 | 2 | 2 |
| fitdecode strict archive | 9 | 4 | 0 |
| official Python default | 8 | 5 | 0 |
| official Python native-only archive | 9 | 4 | 0 |
| existing JS default | 8 | 5 | 0 |
| existing JS native-only archive | 9 | 4 | 0 |

Fixture สาธารณะให้ 22 messages ทุก decoder/mode Compressed caseคาด 4 messagesรวม file_id; Rustและfitdecodeให้ครบ 4 และ timestampsตรง spec ส่วน official SDKทั้งสองให้ 2-message prefixก่อน error Developer collision caseคาด 6 messages; Rust default/archiveและdecoderอื่นอ่านครบ 6 แต่ Rust numeric_enumหยุดที่ 3 messagesด้วย error `fit_base_type_id must be string` ไม่เปิด optionนี้เพื่อแก้ปัญหา numeric identity

Smoke assertionsจาก stored resultsผ่านจริง: manifest 13 hashesตรง, 4 librariesตรวจ, public fixture22 messages, compressed reconstruction2 implementations, corruption4 cases, developer collision3 values พร้อมยืนยัน negative casesของ Rust numeric metadataและ HR expansion ไม่ใช่การรับรอง production importer

### Known/unknown numeric identity, arrays, invalid, zero, integers

- `fitdecode` เก็บ unknown message และ unknown fields พร้อม number/base type; array สังเคราะห์แบบ mixed-valid 3 elements และ all-invalid 2 elements ยังอยู่ครบ รวมทั้ง invalid scalar และ zero-invalid type
- Rust default/archive เก็บ unknown numeric schemaและ mixed array แต่ละ invalid scalar, zero-invalid scalar และ all-invalid array เช่น official Python; signed/unsigned 64-bit 2 ค่าที่ทดลองยัง exactทั้ง nativeและexpanded การมี enum labelของ manufacturer-range boundary ไม่แปลว่ามี message schema ต้องเก็บ global numberแยกจาก labelเสมอ
- official Python เก็บ unknown numeric identities และ mixed array แต่ **ละ invalid scalar, zero-invalid scalar และ all-invalid array** ทั้ง default/archive ถ้าดู dictionary อย่างเดียวจะไม่แยก absent กับ encoded-invalid ได้
- JS default ละ unknown message ทั้ง message และ unknown field ใน known message; archive ที่เปิด `includeUnknownData:true` เก็บ numeric identities ได้ แต่ invalid-presence ยังถูกละเช่น Python
- signed/unsigned 64-bit สังเคราะห์ 2 ค่าเกิน JavaScript safe-integer range เก็บได้ตรงใน Python integer ทั้งสอง decoder และ JS ส่งเป็น **BigInt** Spike serializer ใส่ tagged decimal representation แทน cast เป็น Number ผลนี้ไม่พิสูจน์ว่าทุก export callsite ในระบบ lossless
- ordinary zero HR/speed ไม่ถูกแปลงเป็น absent; zero-invalid base type ต้องเป็น invalid ตาม Table 7 ไม่ใช่นำ rule ของ ordinary numeric zero มาใช้ทั้งหมด
- Spec กำหนด repeated base-type elements ตาม size และ sentinel ตามชนิด [S1, Table 7] จึงให้ definition + element validity เป็นหลัก ไม่ใช้เสียงส่วนใหญ่ของ decoder ตัดสิน การรักษา original FIT เป็นหลักฐาน byte-for-byte ยังจำเป็นแม้ decoder มี raw_value

### Developer-name collisions

Case มี native field 1 ตัวและ developer field 2 ตัวชื่อเดียวกัน field number ของ developer ซ้ำกันแต่ developer index ต่างกัน **ทุก decoder ใน modeที่อ่านครบเก็บค่าครบ 3 values** โดย identity แยกจากชื่อ Rust outputแสดง developer orderต่างจาก definition ซึ่งสอดคล้องกับ HashMap implementation ไม่ควร flatten เป็น object keyed by field nameหรือใช้ field positionเป็นidentity

สัญญา integration ต้องใช้ `(global message number, native field number)` หรือ `(developer_data_index, field_definition_number)` พร้อม source/definition และ mapping ของ field-description key ชื่อ, units และ native-field equivalence เป็น metadata ไม่ใช่ uniqueness key [S1, Tables 8–10]

### Compressed timestamps

Case ใช้ full timestamp ก่อน แล้ว local definition อีกชุดไม่มี timestamp field ตามตัวอย่าง Figure 10 ของ protocol มี compressed records 2 ตัวและ rollover 5-bit [S1] `fitdecode` reconstruct ลำดับ 3 timestamps ได้ตรงกับ arithmetic ของ spec

official Python error: `Compressed timestamp messages are not currently supported`; JS error ความหมายเดียวกัน ทั้งสองให้ข้อมูล prefix ก่อน error แต่ **ห้าม publish prefix เป็น import สำเร็จ** Integrity ของไฟล์เป็น valid จึงไม่ใช่ CRC failure

รอบแรก generator ผิดโดยใช้ definition ที่ยังมี timestamp แล้วตัด timestamp payload ทำให้ fitdecode assert หลังอ่าน prefix แก้ generator ตาม primary spec ไม่ special-case parser ผลรอบนั้นไม่ใช่หลักฐาน fitdecode ไม่รองรับ compressed timestamp

### Components, HR/RR และ order

- Components case มี native speed และ packed speed/distance; `fitdecode` เก็บ composite พร้อม expanded fields แต่ให้ speed identity ซ้ำจาก native/expanded การเก็บ array ของ fields พร้อม `is_expanded` สำคัญกว่า object ชื่อเดียว
- Rust defaultแทน compositeด้วย expanded fields ส่วน KeepCompositeFieldsเก็บ compositeได้ แต่ไม่มี expanded-provenance flagพร้อมใช้ จึงต้องแยก native captureจาก profile-expanded output ไม่เดา sourceจาก field numberหรือชื่อที่ซ้ำ
- official Python default สร้าง enhanced_speed เป็น array ของ component results ใน case นี้ ส่วน JS ให้ scalar ความต่างนี้เป็น representation ไม่ได้พิสูจน์ native sample เพิ่ม การปิด expansion เก็บ composite bytes และ native speed ก่อน scaleได้ ต้องบันทึก provenance ไม่เดา source จากชื่อ
- HR/RR case interleave record, HR anchor, HRV array, record, HR delta ทุก decoder ที่อ่านสำเร็จรักษา message order ใน ordered stream/listener HRV array มี 3 elements รวม invalid element กลาง; decoder Python preserve order และ apply seconds scale ตรง profile ส่วน RR ไม่ใช่ reciprocal ของ sampled HR
- Official profile ระบุ `hrv.time` เป็น “Time between beats”, uint16 array, scale 1000 และ units seconds; `hr.event_timestamp` เป็น accumulated uint32 scale 1024; 12-bit components target event_timestamp [S6, profile source] ต้องคำนวณ beat event time ตาม anchor/accumulation ไม่แปลงเป็น UTC epochโดยตรง
- **Observed blocker ของ fitparser 0.11.0:** packed HR payload 3 bytesมี event components 2 ตัว แต่ expanded outputสร้าง event_timestamp 10 elements รวม 8 zero entriesที่ไม่มีใน payload และ event timesไม่ได้ carry preceding full anchor `profile/decode.rs:57914–57975` ไม่ seed accumulatorจาก full event_timestampและเรียก extract_componentครบ 10 ครั้ง; `profile/mod.rs:87–108` เติม zeroเมื่อ bytesหมด Protocol Componentsกำหนดให้หยุดเมื่อบิตหมด [S1] จึงถือเป็น fidelity failure ไม่ใช่ representation choiceหรือผลโหวตของ libraries ต้องแก้ general component boundsและaccumulation แล้วเก็บ failing-before/passing-after fixtureก่อนปล่อย
- `fitdecode` DefaultDataProcessor แปลง expanded HR event timestamps เป็น datetime เทียบกับ HR anchor ส่วน official SDK แสดง accumulated event time เป็น seconds ทั้งสอง representationต้องย้อนกลับไปตรวจ raw component + anchor ไม่เทียบ JSON ตรง ๆ หรือเลือกส่วนใหญ่ การทดลองนี้ **ไม่พิสูจน์ physiological RR usability** และไม่ทดสอบ artifact detection ของ RR
- Official Python/JS default merge เปลี่ยน record HR ทั้ง 2 ตัวจาก native record readings ใน case สังเคราะห์ Source `hr_mesg_utils` / `utils-hr-mesg` มี carry-forward สูงสุด 5 seconds ที่ 250 ms และ average เข้า record window นี่เป็น derived transform ไม่ใช่ raw decode ห้ามเปิดใน immutable decoded archive [S6–S7]
- sparse HR stress: Python defaultเกิด missing fractional_timestamp error; JS defaultเกิด `anchor HR mesg must have 1 event_timestamp` หลัง decode messagesครบ ส่วน archive modeที่ไม่ mergeอ่านได้ ไม่ใช้ stress caseนี้กล่าวว่า valid full HR filesทั้งหมดผิด และห้ามกลบ errorด้วยfallback

### Corruption / CRC

ทดลอง corruption แยก 4 แบบ: footer CRC ผิด; header CRC ผิดแต่ recompute footer ให้ถูก; signature ผิด; truncated payload

`fitdecode` default อ่าน CRC-corrupt cases ต่อพร้อม warning จึงไม่เหมาะเป็น import policy ส่วน strict archive mode raise ทั้ง header/footer CRC error `read()` ของ official Python/JS ไม่ reject header CRC case ที่ footerถูก แต่ `check_integrity()` / `checkIntegrity()` ให้ false ทั้งคู่ จึงต้องเช็ก integrity และไม่ยอมรับ partial output หากใช้ SDK

Header 12 bytes อ่านได้ทุก implementation ที่ทดลอง Protocol อนุญาต header CRC ของ header 14 bytes เป็น zero ไม่ใช่ corruptionโดยอัตโนมัติ [S1, File Header] Case zero-header-CRC ยังไม่ได้รัน การตั้ง rule ว่า 14-byte header ต้องมี nonzero CRC จะขัด spec

Rust default/archive reject corruptionทั้ง 4 cases แต่ helperยังบันทึก prefixเพื่อวิเคราะห์ API `from_bytes()` productionคืน Errเมื่ออ่านไม่ครบ จึงต้องให้ wrapper discard staged decoded outputทั้งหมด ส่วน Rust Value::Timestampที่ serializeโดยlibraryใช้ process-local timezone; timestamp instantsตรงspecทั้ง 3 ตัวแต่ offsetนั้นไม่ใช่ offsetจาก source FIT Wrapperปัจจุบัน convert UTCอยู่แล้ว ห้ามนำ process offsetไปใช้เป็น device timezone

## ต้นทุน runtime ที่วัดจริงและสิ่งที่ยังไม่วัด

เครื่องทดลอง Darwin arm64, hardware `Mac13,1`, RAM 68,719,476,736 bytes, physical/logical CPU 10/10 วัดหนึ่ง cold process ต่อ decoderใน corpus จิ๋วนี้ ไม่ใช่ capacity benchmark ไม่อ้าง throughput, concurrency, peak production RAM หรือความเร็วของกิจกรรมจริง

บันทึกการรันของ MIT libraries เท่านั้น โดยไม่จัดอันดับข้าม implementation:

- fitdecode final process: `/usr/bin/time -l` **0.06 s real**, maximum RSS **29,605,888 bytes**; measured module import **3.8275 ms**
- Rust fitparser final isolated processที่ parent compile และรัน: **0.60 s real**, maximum RSS **11,452,416 bytes**, peak memory footprint **6,242,640 bytes** รวม default/archive/numeric_enum 3 modes ไม่รวม compilation
- Official Python และ JS SDK: วัด diagnostics ไปแล้วก่อนพบข้อกำหนด license แต่ **ไม่เผยแพร่ตัวเลขใน repository** และไม่รันเพิ่ม รอ legal review ตัวเลขจาก Bun Node-compatible memory API รอบแรกมี unit conversionผิดจึง reject measurement นั้น ไม่ใช้เป็น evidence ใด ๆ

Whole-process times/RSS รวม interpreter/runtime startup, module initialization, read ทั้ง corpus ทั้ง modes, conversion, JSON serialization และ output ไม่รวมติดตั้ง package หรือ compilation Per-file `elapsed_ms` ใน JSON ไม่รวม interpreter startup/module import และไม่รวมการเขียน result fileครั้งสุดท้าย Python fitdecode รวม file open, reader initialization, CRC, decoding, capture conversion; official SDK รวม file read, integrity pass แยกอีก 1 pass, decoder initialization และ capture conversion จึงเทียบตรงกับ Rust core decoder ไม่ได้ Rust probe อ่าน bytes ก่อน per-file timer แต่ timer รวม stream processor, native/definition capture และ profile decode

ไม่ได้ทำ repetitions, warm/cold filesystem separation, RSS attributionต่อ activity, largest-file/ZIP-bomb/timeouts/cancellation, worker pool contention, multi-hour samples หรือ statistical latency distribution การตั้ง byte/time/memory/concurrency budgets ต้องมี phase acceptance แยก ไม่ใช้ผลนี้รับรอง scale

## หลักฐาน implementation ที่ตรวจจริง — 2026-10-05

ฐานงานคือ `94a812e0a83fbd5d59a6a05833ea45be36cca139` และ dependency/root-fix commit คือ `7584198c60be7e854c7311d52a6c3b0aedf4b76a` ใช้ `fitparser` 0.11.0 ที่ vendor เฉพาะ Rust source/profile ที่จำเป็น พร้อม MIT `LICENSE`, upstream revision และขอบเขต patch ใน `vendor/fitparser/PATCHES.txt` ไม่รวม official SDK และไม่เพิ่ม runtime service

- แก้ general component bounds และ accumulated targets ใน library ไม่กรอง zero หลัง decode: packed HR 3 bytes มีสอง events เท่านั้น และ native anchor 100 s ให้ events 101/102 s การ carry ครอบคลุม distance/cycles/power/HR ตาม profile พร้อม source parent ของ expanded field
- Archive เปิด `KeepCompositeFields`, `PreserveInvalidValues` และ `PreserveUnknownDeveloperFields` โดยไม่เปิด numeric-enum mode ทั้งไฟล์ จึงเก็บ wire definition/order, global/local/field identity, developer/application metadata, original composite, expanded parent, byte references และ actual profile/developer scale/offset ได้ Ordinary zero คง zero; encoded invalid มี null/validity และตำแหน่ง array; unsafe integers เป็น decimal strings พร้อม type identity
- Public seam มีเพียง `fit::runs::decode_run_to_writer` ซึ่งเขียน JSON decoded/normalized/preserved v1 ลง private sink ของ caller ไม่เก็บ whole-archive `Value` และไม่เหลือ normalizer อีกชุด Caller ต้องทิ้ง private output ทุกกรณีที่ decode ไม่สำเร็จ รุ่นนี้ยืนยัน Garmin activity เดียวและ subtype generic/treadmill/street/trail/track โดยไม่ปฏิเสธ foreign accessory เพียงเพราะ manufacturer ของ sensor ต่างกัน Records นอก session คงใน archive แต่ไม่ใส่ใน normalized samples
- Normalization คง source resolution/order, enhanced speed/altitude precedence, cadence cycles-to-steps ตาม field definition และ source references แยก recorded summaries จาก interval-weighted derived means พร้อม coverage/pause/gap policy ไม่สร้าง pace จาก zero speed และไม่ใช้ cadence heuristic เดิมใน Runs ใหม่ Source timezone context มาจาก FIT เท่านั้น
- Native `hrv.time` รักษา invalid positions แต่ไม่ให้ UTC alignment จาก message arrival ส่วน packed HR ใช้ timestamp/fractional/full event counter anchor ที่อ่านจริง พร้อม `anchorSourceReferences`; RR มาจาก beat-counter differences ไม่ใช่ reciprocal sampled HR Unknown/developer extensions คงภายในและจำแนก fail-closed
- Corpus ถาวรอยู่ `apps/api/tests/fixtures/runs/` มี stdlib generator, CC0 license, deterministic hashes, field dictionary, expected arithmetic, synthetic developer identities, malformed streams และ ZIP member manifest Numerical progressive fixture มี source-backed packed RR 1,063 intervals และ independent quantized-input oracle แยกจาก physiological validation

Scoped verification ที่รันจริง:

```sh
cargo test -p garmin-fit-extractor-api --test fit_decode
cargo test -p garmin-fit-extractor-api --test runs_decode
cargo test -p garmin-fit-extractor-api --lib fit::stream::tests
python3 apps/api/tests/fixtures/runs/generate.py
```

ผลล่าสุดคือ `fit_decode` 3 tests, `runs_decode` 17 tests และ `fit::stream::tests` 2 tests ผ่าน รวมความผิดพลาดด้าน CRC/header, endian, invalid string/byte/array, rollover, component fallback, zero RR, recorded zero speed และ exact fractional RR anchors ไม่ใช้ non-empty/length-grew checks เป็น consumer proof Packed regression ล้มก่อน root fix ส่วน byte/string sentinel และ selected-summary pace มี failing-before/passing-after หลักฐาน Final review พบ JSON spool parser เปลี่ยน native fractional anchor 9/32768 จน elapsed เป็น 1.000274896621704 แทน 1.000274658203125; permanent public-writer regression ล้มก่อนเปิด `serde_json/float_roundtrip` แล้วผ่าน พร้อม exact UTC timestamp การตรวจ progressive native ทั้งสองไฟล์เปรียบเทียบทุก RR interval กับ independent quantized beat bounds ไม่อ้างว่าการทดสอบ engine อย่างเดียว decode native FIT Generator สร้าง bytes และ manifest ซ้ำได้โดยไม่ใช้ SDK หรือข้อมูลส่วนตัว คำสั่ง `cargo test -p fitparser --lib` ไม่รัน tests เพราะ dependency ไม่ใช่ workspace member จึงไม่อ้างผล vendor suite จากคำสั่งนี้

Standalone CLI smoke ล่าสุดเรียก public `decode_run_to_writer` และ parse JSON จริง: Garmin run 24 messages/4 bounded samples; rich archive 41 messages/12 samples/5 RR ที่ไม่ aligned ทั้งหมด; packed HR 26 messages/2 aligned RR; progressive fixtures ทั้ง original boundary-negative และ positive-oracle ให้ 596 messages/481 samples/1,063 aligned RR ขนาด JSON ล่าสุดของ original/positive คือ 2,540,170/2,539,912 bytes โดย RR source references เก็บ array positions จริงและ metadata แสดง library/profile/options/schema/version/source hashes ที่ตรวจแล้ว Public MIT `activity.fit` มี manufacturer code 15 จึง unsupported สำหรับ Garmin-only Runs ใหม่ แต่ legacy `decode_raw` ยัง decode ได้ตามเดิม

Capacity root fix วัดกับ CC0 `full_export_100k.fit` ที่ generator ถาวรสร้างได้ ไม่เปิดกิจกรรมส่วนตัว Whole-Value path เดิมให้ JSON 825,052,485 bytes, 100.54 s และ max RSS 9,894,985,728 bytes หลังลบ known-field extensions ที่ซ้ำและย้าย arrays โดยไม่ clone ผลเปลี่ยนเป็น 496,794,368 bytes, 45.95 s และ max RSS 4,110,041,088 bytes จึงยังไม่ผ่าน memory budget ไม่รัน baseline เดิมซ้ำเพื่อยืนยันผล

Typed streaming path เขียน decoded messages ลง caller โดยตรง เก็บเฉพาะ typed numerical samples/source references และ legacy fields ที่ใช้งาน ไม่สร้าง decoded-message spool อีก 371 MB Internal arrays ใช้ owned directory 0700, create-new files 0600 และ byte counter ร่วมกับ final writer ที่ hard limit 512 MiB Caller ตั้ง child `TMPDIR` ให้ซ้อนใน private directory เพื่อให้ parent cleanup ครอบคลุม timeout/forced kill Output/control contract ของ subprocess คือ private `run.json` และ `runs-spool-1` control ที่ stdout ไม่เกิน 64 KiB โดย disk bound แยกจาก stdout bound

Actual typed CLI smoke บน Darwin arm64 แบบ debug ให้ 496,794,368 bytes, 42.77 s real (38.89 s user, 0.66 s sys), max RSS 230,719,488 bytes และ peak footprint 223,773,224 bytes ตรวจ JSON ครบ 100,000 samples, indices 0–99,999, distance 0–10,800 m และ cadence แรก 340 steps/min โดยไม่ materialize whole document Layer sizes decoded/normalized/legacy คือ 371,797,035/113,538,063/11,459,234 bytes เท่ากับ changed path ก่อน streaming Private modes 0700/0600 ตรวจจริง และ document SHA-256 คือ `07b0a7907c8f8e715c58177ca2afdf8aa5d60ef43953be86468ec0e8e4b3251c` ผลนี้ยังไม่ใช่ Linux `RLIMIT_AS=512 MiB`/60 s subprocess proof

การวัด capacity ข้างบนเกิดก่อนเพิ่ม safe implementation provenance metadata 613 bytes และ precision regression fix ที่ไม่เปลี่ยน 100k fixture ซึ่งไม่มี RR ไม่รัน 100k path เดิมซ้ำ Final runtime ใช้ `fit::runs::decoder_metadata()` เป็น definition ร่วมสำหรับ archive และ revision metadata: upstream library 0.11.0, maintained decoder `0.11.0+runs.1`, profile 21.202.0, normalizer `native-runs-stream.1`, schemas 2.0.0, actual options และ JSON float-roundtrip flag พร้อม SHA-256 ของ sorted vendored Rust sources/profile decode source/normalizer source/archive adapter/raw projection Source hashes คำนวณจาก bytes ที่ตรวจจริงและต้องปรับเมื่อ source เปลี่ยน แยกจาก private uploaded FIT hash/byte length

ข้อจำกัดที่ยังไม่ผ่านใน slice นี้: Linux production subprocess budgets, physiological reference validation, large real-world FIT distributions, HTTP/store/export round-trip และ private export policy เป็นงาน integration/release Extended headers นอก 12/14 bytes, trailing/chained FIT และ declared-size mismatch ถูกปฏิเสธอย่างชัดเจน ไม่ publish partial output เป็น success Subfield reference metadata ที่ library ไม่เปิดและ unknown/later developer descriptions ต้องมีข้อจำกัดหรือ warning; ไม่อ้าง extraction 100% และไม่อ้างว่าการเก็บ Original FIT ชดเชยทุก semantic gap

## Hard gates ก่อน implementation/cutover

1. **Archive schema:** ordered message sequence, definition reference, numeric message/field/developer identity, wire field order, base type/size, units/scale/offset, encoded-invalid vs absent, ordinary zero, raw/composite และ expanded provenance ต้องผ่าน corpus expected assertions ห้ามใช้ name-only object และห้ามใช้ Debug string เป็น production schema
2. **Rust metadata retention:** เปิด stable access ของ field definition metadataและ developer index หรือแก้ upstream/ใช้ minimal maintained patch โดยได้รับอนุมัติ; preserve presence จาก definition และ raw bytes/offset ก่อน parser ตัด invalid scalar เก็บ explicit decoded nullพร้อม validity ไม่สร้างค่าที่ไม่เคยอยู่ในไฟล์ อย่าเพิ่ม decoder framework/factory หลายตัวเพื่อกลบช่องนี้
3. **Numeric enum negative case:** ไม่เปิด ReturnNumericEnumValues ทั่วไฟล์ถ้ายังทำ field_description type resolutionเสีย Numeric identity ไม่จำเป็นต้องแลกกับ numeric enums: native representation และ separate enum label สามารถอยู่ร่วมกัน
4. **Compressed/integrity:** ต้องได้ reconstructed timestamp arithmetic และ rollover ตรง spec; reject corrupt files/partial prefix; zero-header CRC, extended header, declared-size/trailing/chained file policy ต้องมี explicit behavior
5. **HR/RR:** immutable archive ปิด merge/smoothing/interpolation; validate arrays/anchor/accumulator against official profile มี traceable raw components; RR engineใช้เฉพาะเวลาระหว่าง beat ที่มีจริง ไม่ประมาณจาก sampled HR ให้ quality/abstention แยกจาก parser success
6. **Exact JSON boundary:** typed decimal string/tag สำหรับ integers เกิน safe range; invalid เป็น nullพร้อม validity; nonfinite floatไม่ serialize เป็น NaN/Infinity อย่า cast BigInt/Rust64 to Number ต้อง exercise import/store/full export/download ครบ ไม่ใช่เฉพาะ library smoke
7. **Compatibility/import scope:** เพิ่ม synthetic Garmin running session/sport/subtype, unknown enums, developer scale/offsetและ full application metadata, missing/later descriptions, invalid byte arrays, floats/nonfinite, endian, subfields, multiple sessions, source timezone/local time, clock resets และ RR rollover/gaps ก่อนเรียก full fidelity Caseเหล่านี้ยังไม่ได้ exercise ใน spikeนี้
8. **License/operations:** Garmin SDK use/distribution/externally published diagnostics ต้อง review FIT Protocol License; corpus MIT redistributionต้อง include copyright/license notice; การนำ CC0 generator into tests ต้องลงทะเบียน fixture manifest/data dictionary; enforce resource limitsและdurable worker failure atomicityแยกจาก library comparison

## Artifacts และคำสั่งตรวจซ้ำ

เก็บ artifacts ภายนอก checkout ที่ `/tmp/runs-decoder-spike-20261004/` จน parent verifyเสร็จ ไม่ลบทิ้งก่อน handoff:

- `generate.py`, `manifest.json`, `corpus/`: generator และ provenance/digests สุดท้าย
- `requirements.txt`, `package-evidence.json`, `source-hashes.json`, `sources/fitparser-0.11.0.crate`, extracted source; JS `package.json`/`bun.lock` pin version
- `compare.py`, `fitdecode-results.json`, `garmin-fit-sdk-results.json`; `js/compare.ts`, `js-results.json`
- `rust-probe/Cargo.toml`, `Cargo.lock`, `src/main.rs`, compiled binary และ `fitparser-results.json`
- `check_fidelity.py`: assertionsจาก stored JSON; ไม่ execute Garmin SDKซ้ำและไม่พิมพ์ decoded values ของ fixture

ตรวจผลที่เก็บไว้ได้โดยไม่ decode SDK ซ้ำ:

```sh
/tmp/runs-decoder-spike-20261004/bin/python /tmp/runs-decoder-spike-20261004/check_fidelity.py
```

Rust probe ไม่แก้ repository manifests; parentเป็นผู้ compile isolated helper ตามข้อกำหนดของงาน:

```sh
cargo build --release --locked --manifest-path /tmp/runs-decoder-spike-20261004/rust-probe/Cargo.toml
/usr/bin/time -l /tmp/runs-decoder-spike-20261004/rust-probe/target/release/runs-decoder-probe /tmp/runs-decoder-spike-20261004/corpus /tmp/runs-decoder-spike-20261004/fitparser-results.json
```

คำสั่ง generatorจะเขียน corpus เดิมซ้ำ ต้องไม่มี probe อ่านพร้อมกัน การรัน SDK comparisonซ้ำถูก legal gateด้านบนกั้นไว้ ไฟล์ scriptsยังคง runnable สำหรับผู้ได้รับสิทธิ์และอนุมัติ ไม่ควรเพิ่ม SDK comparisonเข้า CIโดยอัตโนมัติ

## Primary sources

- **S1 — FIT Protocol ฉบับที่อ่าน full content ได้:** [official embedded protocol document](https://developer.garmin.com/fit/articles/fit-protocol/fit_protocol.html), โดยเฉพาะ Tables 5–10, Figure 10, File Header, CRC, Dynamic Fields, Components และ Common Fields หน้า wrapper `/fit/protocol/` แสดงข้อความไม่ครบจึงไม่ใช้เป็นหลักฐานเดี่ยว
- **S2 — Integrity และ partial decoding:** [Garmin IsFIT / CheckIntegrity / Read cookbook](https://developer.garmin.com/fit/cookbook/isfit-checkintegrity-read/)
- **S3 — Rust source ที่ pin:** [fitparser 0.11.0 source](https://docs.rs/crate/fitparser/0.11.0/source/), [lib.rs](https://docs.rs/crate/fitparser/0.11.0/source/src/lib.rs), [de/mod.rs](https://docs.rs/crate/fitparser/0.11.0/source/src/de/mod.rs), [de/parser.rs](https://docs.rs/crate/fitparser/0.11.0/source/src/de/parser.rs), [de/decode.rs](https://docs.rs/crate/fitparser/0.11.0/source/src/de/decode.rs); crate download/checksumตรวจจริงตามข้อความด้านบน
- **S4 — fitdecode primary source/docs:** [polyvertex/fitdecode](https://github.com/polyvertex/fitdecode), [reader source](https://github.com/polyvertex/fitdecode/blob/master/fitdecode/reader.py), [data processor source](https://github.com/polyvertex/fitdecode/blob/master/fitdecode/processors.py); exact installed source hashesอยู่ artifact ไม่กล่าวว่า branchเป็น immutable pin
- **S5 — Exact fitdecode package:** [PyPI 0.11.0 metadata](https://pypi.org/pypi/fitdecode/0.11.0/json), [release](https://pypi.org/project/fitdecode/0.11.0/)
- **S6 — Official Python SDK:** [Garmin repository and option contract](https://github.com/garmin/fit-python-sdk), [decoder](https://github.com/garmin/fit-python-sdk/blob/main/garmin_fit_sdk/decoder.py), [profile](https://github.com/garmin/fit-python-sdk/blob/main/garmin_fit_sdk/profile.py), [HR merge source](https://github.com/garmin/fit-python-sdk/blob/main/garmin_fit_sdk/hr_mesg_utils.py), [exact PyPI metadata 21.217.0](https://pypi.org/pypi/garmin-fit-sdk/21.217.0/json)
- **S7 — Official JS SDK:** [Garmin fit-javascript-sdk](https://github.com/garmin/fit-javascript-sdk), [decoder source](https://github.com/garmin/fit-javascript-sdk/blob/main/src/decoder.js), [package 21.208.0](https://www.npmjs.com/package/@garmin/fitsdk/v/21.208.0); installed source/tag/hashเป็นหลักฐานสำหรับเวอร์ชันที่ทดลอง
- **S8 — Rust license ณ pinned revision:** [MIT license](https://raw.githubusercontent.com/stadelmanma/fitparse-rs/fb82202b34d10c2166784db333d9d23a29b79a49/LICENSE); repository attributionที่ `README.md:170–172` และ fixture bytesตรวจตรงกับ crate
- **S9 — fitdecode MIT:** [upstream LICENSE.txt](https://github.com/polyvertex/fitdecode/blob/master/LICENSE.txt); installed wheel `fitdecode-0.11.0.dist-info/licenses/LICENSE.txt`
- **S10 — Garmin license:** [official Python LICENSE.txt](https://github.com/garmin/fit-python-sdk/blob/main/LICENSE.txt), [official JS LICENSE.txt](https://github.com/garmin/fit-javascript-sdk/blob/main/LICENSE.txt); installed JS license SHA-256 `6cc7ff94b5afc8c3a2b14aeb3e90da97a9fb6c8d40644304da94df5cf56428cf` และ §2(f)ตรวจข้อความจริง
