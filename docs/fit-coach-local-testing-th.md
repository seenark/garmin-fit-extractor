# FIT Coach: คู่มือทดสอบในเครื่อง (Local Smoke Test)

คู่มือนี้ทดสอบเส้นทางเว็บและข้อมูลกิจกรรมด้วย debug users โดย **ไม่ใช้ Google OAuth จริงและไม่เชื่อม Custom GPT** เหมาะสำหรับตรวจว่า session, upload, history และ activity projection ทำงานตามเจ้าของข้อมูล

## ขอบเขตและข้อควรระวัง

- ใช้ `GARMIN_FIT_TEST_AUTH=true` เฉพาะการทดสอบในเครื่อง ห้ามเปิดบนเซิร์ฟเวอร์สาธารณะ
- ไม่ต้องตั้งค่า `GARMIN_FIT_CHATGPT_CLIENT_ID`, `GARMIN_FIT_CHATGPT_CLIENT_SECRET` หรือ `GARMIN_FIT_CHATGPT_REDIRECT_URI` สำหรับ smoke test นี้ ปล่อยว่างทั้งชุดได้
- อย่าใส่ค่า secret จริงในไฟล์ที่ commit หรือในคำสั่งที่บันทึกลง shell history
- ต้องมีไฟล์ fixture `apps/api/tests/fixtures/activity.zip` ตาม repository ปัจจุบัน

## 1. เตรียมตัวแปรและรันเว็บ

จาก root ของ repository:

```sh
export GARMIN_FIT_TEST_AUTH=true
export GARMIN_FIT_BIND=127.0.0.1:3000
export GARMIN_FIT_DATABASE_URL=sqlite:///tmp/garmin-fit-local.sqlite3
export GARMIN_FIT_GOOGLE_CLIENT_ID=
export GARMIN_FIT_GOOGLE_CLIENT_SECRET=
export GARMIN_FIT_GOOGLE_REDIRECT_URI=
export GARMIN_FIT_CHATGPT_CLIENT_ID=
export GARMIN_FIT_CHATGPT_CLIENT_SECRET=
export GARMIN_FIT_CHATGPT_REDIRECT_URI=
bun run build

cargo run -p garmin-fit-extractor-api

```

เปิดอีก terminal แล้วตรวจ health endpoint:

```sh
curl --fail http://127.0.0.1:3000/healthz
```

ควรได้ HTTP 200 จากแอป หากใช้ port อื่น ให้เปลี่ยน `127.0.0.1:3000` ให้ตรงกับ `GARMIN_FIT_BIND`.

## 2. สร้าง debug sessions และอัปโหลดแยกผู้ใช้

กลไก debug login ของ repository ใช้สำหรับจำลอง Google session เท่านั้น ชื่อ endpoint/รูปแบบที่แน่นอนให้ยึดจาก `scripts/e2e.ts` และเส้นทาง `test-login` ที่มีอยู่ในโค้ดปัจจุบัน อย่าเพิ่ม `owner_id`, `user_id` หรือ `email` ลง multipart เพราะเจ้าของต้องมาจาก session:

```sh
curl -i -c /tmp/fit-user-a.cookies \
  'http://127.0.0.1:3000/api/v1/auth/test-login?user=a@example.test'

curl -i -c /tmp/fit-user-b.cookies \
  'http://127.0.0.1:3000/api/v1/auth/test-login?user=b@example.test'

curl --fail -b /tmp/fit-user-a.cookies \
  -F 'files=@apps/api/tests/fixtures/activity.zip' \
  http://127.0.0.1:3000/api/v1/extractions

curl --fail -b /tmp/fit-user-b.cookies \
  -F 'files=@apps/api/tests/fixtures/activity.zip' \
  http://127.0.0.1:3000/api/v1/extractions
```

ถ้า debug harness ใน checkout นี้ใช้ชื่อ query หรือ path ต่างจากตัวอย่าง ให้ copy รูปแบบจาก `scripts/e2e.ts` โดยตรง; ห้ามเดาค่า identity หรือส่งค่าเจ้าของจาก browser

ตรวจ history ของแต่ละ session:

```sh
curl --fail -b /tmp/fit-user-a.cookies http://127.0.0.1:3000/api/v1/extractions
curl --fail -b /tmp/fit-user-b.cookies http://127.0.0.1:3000/api/v1/extractions
```

แต่ละรายการควรเป็นของผู้ใช้ตัวเองเท่านั้น การอัปโหลดที่ไม่มี start time ของ FIT อาจอยู่ใน web history แต่จะไม่ถูกฉายเข้า coach activities.

## 3. ตรวจ activity API ด้วย OAuth debug config (ทางเลือก)

การทดสอบ local แบบเว็บไม่จำเป็นต้องเปิด ChatGPT OAuth หากต้องการตรวจ activity API แบบ end-to-end ให้ตั้งค่ากลุ่มตัวแปรสามตัวเป็นค่าทดสอบที่ไม่ใช่ secret production และใช้ client ID ตาม contract:

```sh
export GARMIN_FIT_CHATGPT_CLIENT_ID=FIT_COACH_CHATGPT
export GARMIN_FIT_CHATGPT_CLIENT_SECRET=local-only-test-secret
export GARMIN_FIT_CHATGPT_REDIRECT_URI=https://local.test/oauth/callback
```

หลัง restart แอป ให้เปิด authorize ด้วย cookie ของ User A (เปลี่ยน `state` เป็นค่าสุ่มของคุณ):

```sh
curl -i -b /tmp/fit-user-a.cookies -G \
  --data-urlencode 'client_id=FIT_COACH_CHATGPT' \
  --data-urlencode 'redirect_uri=https://local.test/oauth/callback' \
  --data-urlencode 'response_type=code' \
  --data-urlencode 'scope=activities:read' \
  --data-urlencode 'state=REPLACE_WITH_RANDOM_STATE' \
  http://127.0.0.1:3000/oauth/authorize
```

เมื่อใช้ session ที่ถูกต้อง ระบบจะ redirect ไป `https://local.test/oauth/callback` พร้อม `code`; ห้ามบันทึก code/secret ลงเอกสาร ให้คัดลอก code ชั่วคราวไปแลก token ใน body แบบ form:

```sh
curl --fail -X POST http://127.0.0.1:3000/oauth/token \
  -H 'Content-Type: application/x-www-form-urlencoded' \
  --data-urlencode grant_type=authorization_code \
  --data-urlencode client_id=FIT_COACH_CHATGPT \
  --data-urlencode client_secret=local-only-test-secret \
  --data-urlencode code=REPLACE_WITH_ONE_TIME_CODE \
  --data-urlencode redirect_uri=https://local.test/oauth/callback
```

ใช้ `access_token` ที่ได้เรียกข้อมูล โดยไม่ส่ง identity parameter ใด ๆ:

```sh
curl --fail \
  -H 'Authorization: Bearer REPLACE_WITH_ACCESS_TOKEN' \
  'http://127.0.0.1:3000/api/v1/activities/latest?detail=summary'
curl --fail \
  -H 'Authorization: Bearer REPLACE_WITH_ACCESS_TOKEN' \
  'http://127.0.0.1:3000/api/v1/activities?limit=10'
```

ควรตรวจว่า User A เห็นเฉพาะกิจกรรมของ A, การเรียกโดยไม่มี/มี Bearer ผิดรูปแบบได้ 401, token scope ไม่ครบได้ 403 และ ID ของอีกผู้ใช้ได้ 404. ไม่ควรพบ `owner_id`, `user_id`, email, Google subject หรือ token ใน response. ห้ามใช้ cookie แทน Bearer กับ activity API

## 4. Google OAuth จริงและ tunnel (manual check แยกต่างหาก)

Local smoke test ข้างต้น **ไม่ใช่** การตรวจ Google จริง หากต้องตรวจ flow จริง ให้ใช้ HTTPS tunnel ที่มี hostname ของคุณ เช่น `https://REPLACE_WITH_HOST` และลงทะเบียน callback แบบ exact URL ใน Google Cloud:

```text
https://REPLACE_WITH_HOST/api/v1/auth/callback
```

ตั้ง `GARMIN_FIT_GOOGLE_CLIENT_ID`, `GARMIN_FIT_GOOGLE_CLIENT_SECRET` และ `GARMIN_FIT_GOOGLE_REDIRECT_URI` ให้ตรงกันทั้งหมด แล้วตรวจผ่าน browser ที่ `https://REPLACE_WITH_HOST/`. ต้องไม่ใช้ HTTP หรือ localhost เป็น callback ของ production-like tunnel. Cloudflare tunnel ต้องส่ง hostname เดียวกันมายัง app port 3000.

ChatGPT ยังไม่เกี่ยวข้องกับการตรวจนี้: ตัวแปร ChatGPT ทั้งชุดปล่อยว่างได้ และ upload/history ของเว็บยังใช้ Google session ได้ตามปกติ
