# FIT Coach: คู่มือทดสอบในเครื่อง (Local Smoke Test)

This is a disposable local smoke procedure for debug sessions and the Runs v2/FIT Coach boundary. It does not verify real Google login or a connected Custom GPT. Core must be merged before these commands exercise the new contract; no successful run of this procedure is claimed here.

## ขอบเขตและข้อควรระวัง

- ใช้ `GARMIN_FIT_TEST_AUTH=true` เฉพาะการทดสอบในเครื่อง ห้ามเปิดบนเซิร์ฟเวอร์สาธารณะ
- ไม่ต้องตั้งค่า `GARMIN_FIT_CHATGPT_CLIENT_ID`, `GARMIN_FIT_CHATGPT_CLIENT_SECRET` หรือ `GARMIN_FIT_CHATGPT_REDIRECT_URI` สำหรับ smoke test นี้ ปล่อยว่างทั้งชุดได้
- อย่าใส่ค่า secret จริงในไฟล์ที่ commit หรือในคำสั่งที่บันทึกลง shell history
- Use `apps/api/tests/fixtures/runs/garmin_run.zip`, the synthetic Garmin-running fixture. The legacy `activity.zip` fixture is not supported Garmin-running input.
- Follow the README's disposable PostgreSQL 18 verification prerequisites. Do not use normal Compose, `./db-data`, a shared development database, or an ambient `.env` for this smoke.

## 1. เตรียมตัวแปรและรันเว็บ

Use a secret-free source archive. Supply the verified disposable PostgreSQL URL
explicitly; the database must contain no valuable data. Clear inherited Google
and FIT Coach credentials. Debug auth must bind only to loopback.

```sh
: "${TEST_DATABASE_URL:?Set the verified disposable PostgreSQL 18 URL}"
export DATABASE_URL="$TEST_DATABASE_URL"
export GARMIN_FIT_TEST_AUTH=true
export GARMIN_FIT_BIND=127.0.0.1:3000
unset GARMIN_FIT_GOOGLE_CLIENT_ID GARMIN_FIT_GOOGLE_CLIENT_SECRET GARMIN_FIT_GOOGLE_REDIRECT_URI
unset GARMIN_FIT_CHATGPT_CLIENT_ID GARMIN_FIT_CHATGPT_CLIENT_SECRET GARMIN_FIT_CHATGPT_REDIRECT_URI
bun --no-env-file install --frozen-lockfile
cargo build --locked -p garmin-fit-extractor-api
./target/debug/garmin-fit-extractor-api
```

If `CARGO_TARGET_DIR` changes the executable location, use that actual built
binary instead. The running API must be able to launch its own decoder-child
mode and create a private temporary workspace. Do not point
`RUNS_DECODER_EXECUTABLE` at a Rust test harness or an old API binary.

เปิดอีก terminal แล้วตรวจ health endpoint:

```sh
curl --fail http://127.0.0.1:3000/healthz
```

ควรได้ HTTP 200 จากแอป หากใช้ port อื่น ให้เปลี่ยน `127.0.0.1:3000` ให้ตรงกับ `GARMIN_FIT_BIND`.

## 2. สร้าง debug sessions และอัปโหลดแยกผู้ใช้

Create two debug sessions. Owner identity comes only from the session cookie,
never an `owner_id`, `user_id`, or `email` multipart field. Keep cookies private
in a unique directory in the terminal used for the following commands:

```sh
umask 077
SMOKE_DIR="$(mktemp -d /tmp/fit-coach-smoke.XXXXXX)"
curl --fail -c "$SMOKE_DIR/user-a.cookies" \
  'http://127.0.0.1:3000/api/v1/auth/test-login?user=a@example.test'
curl --fail -c "$SMOKE_DIR/user-b.cookies" \
  'http://127.0.0.1:3000/api/v1/auth/test-login?user=b@example.test'

curl --fail -b "$SMOKE_DIR/user-a.cookies" \
  -F 'files=@apps/api/tests/fixtures/runs/garmin_run.zip' \
  http://127.0.0.1:3000/api/v2/runs/imports
curl --fail -b "$SMOKE_DIR/user-b.cookies" \
  -F 'files=@apps/api/tests/fixtures/runs/garmin_run.zip' \
  http://127.0.0.1:3000/api/v2/runs/imports
```

Review `items` and `counts`; import acceptance can still mean queued processing.
Use each returned `activityId` to poll `/api/v2/runs/{id}` until ready or failed,
with a finite local deadline. A timeout is a failed smoke, not a ready result.

ตรวจ history ของแต่ละ session:

```sh
curl --fail -b "$SMOKE_DIR/user-a.cookies" http://127.0.0.1:3000/api/v2/runs
curl --fail -b "$SMOKE_DIR/user-b.cookies" http://127.0.0.1:3000/api/v2/runs
```

Confirm each session sees only its own activities. Identical source bytes are
deduplicated within one owner, not across owners. Readiness and owner isolation
must be observed before proceeding to the preserved OAuth projection.

## 3. ตรวจ activity API ด้วย OAuth debug config (ทางเลือก)

การทดสอบ local แบบเว็บไม่จำเป็นต้องเปิด ChatGPT OAuth หากต้องการตรวจ activity API แบบ end-to-end ให้ตั้งค่ากลุ่มตัวแปรสามตัวเป็นค่าทดสอบที่ไม่ใช่ secret production และใช้ client ID ตาม contract:

```sh
export GARMIN_FIT_CHATGPT_CLIENT_ID=FIT_COACH_CHATGPT
export GARMIN_FIT_CHATGPT_CLIENT_SECRET=local-only-test-secret
export GARMIN_FIT_CHATGPT_REDIRECT_URI=https://local.test/oauth/callback
```

หลัง restart แอป ให้เปิด authorize ด้วย cookie ของ User A (เปลี่ยน `state` เป็นค่าสุ่มของคุณ):

```sh
curl -i -b "$SMOKE_DIR/user-a.cookies" -G \
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

Stop only the API and disposable cluster you created. Remove the two cookie
files and then the owned empty directory:

```sh
rm "$SMOKE_DIR/user-a.cookies" "$SMOKE_DIR/user-b.cookies"
rmdir "$SMOKE_DIR"
```

Do not stop a shared Compose stack or reuse its `db-data`. Debug sessions do
not prove real Google login. This local procedure does not deploy or reset Runs.
