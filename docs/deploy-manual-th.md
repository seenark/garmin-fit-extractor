# คู่มือ deploy เว็บแบบ production-like (ยังไม่เชื่อม Custom GPT)

คู่มือนี้ deploy เว็บ Garmin FIT Extractor ด้วย Docker และ Cloudflare Tunnel โดยใช้ Google OAuth จริง แต่ยังไม่ตั้งค่า ChatGPT/Custom GPT. ทำตามลำดับและแทนค่าทุก `REPLACE_WITH_*` ก่อนรัน

## ภาพรวมและค่าที่ต้องเตรียม

- Docker Hub repository เริ่มต้นคือ `hadesgod/garmin-fit-extractor`
- Cloudflare Tunnel ส่ง hostname สาธารณะ `https://REPLACE_WITH_HOST` ไปยัง app port `3000`
- Google callback ต้องเป็น URL exact: `https://REPLACE_WITH_HOST/api/v1/auth/callback`
- เว็บใช้ Google session สำหรับ upload และ history; ไม่รับ owner จาก request
- ตัวแปร ChatGPT ทั้งชุดปล่อยว่างได้เมื่อ deploy เว็บอย่างเดียว ห้ามสร้าง fake ChatGPT credentials ใน production
- Docker volume SQLite ต้อง persist และต้อง backup ก่อนการบำรุงรักษา ห้ามใช้ `docker compose down -v` เพราะจะลบ volume และประวัติ

## 1. ตั้งค่า environment อย่างปลอดภัย

สร้าง `.env` บนเครื่อง deploy (อย่า commit และอย่าแปะค่าจริงใน issue/chat):

```dotenv
GARMIN_FIT_IMAGE=hadesgod/garmin-fit-extractor
GARMIN_FIT_TAG=latest
GARMIN_FIT_PORT=3000
GARMIN_FIT_BIND=0.0.0.0:3000
GARMIN_FIT_DATABASE_URL=sqlite:///data/garmin-fit-extractor.sqlite3
GARMIN_FIT_STATIC_DIR=/app/public
GARMIN_FIT_GOOGLE_CLIENT_ID=REPLACE_WITH_GOOGLE_CLIENT_ID
GARMIN_FIT_GOOGLE_CLIENT_SECRET=REPLACE_WITH_GOOGLE_CLIENT_SECRET
GARMIN_FIT_GOOGLE_REDIRECT_URI=https://REPLACE_WITH_HOST/api/v1/auth/callback
GARMIN_FIT_CHATGPT_CLIENT_ID=
GARMIN_FIT_CHATGPT_CLIENT_SECRET=
GARMIN_FIT_CHATGPT_REDIRECT_URI=
RUST_LOG=info
```

ตั้ง permission ให้เฉพาะผู้ดูแลอ่านได้:

```sh
chmod 600 .env
```

`GARMIN_FIT_IMAGE`, `GARMIN_FIT_TAG` และ `GARMIN_FIT_PORT` เป็น convention ที่ compose ใช้ override ได้; compose map ตัวแปร Google และ ChatGPT ให้ container แล้ว ค่า ChatGPT ว่างทั้งกลุ่มไม่กระทบ Google login หรือเว็บ upload/history.

ใน Google Cloud Console เพิ่ม Authorized redirect URI เพียงค่า exact นี้:

```text
https://REPLACE_WITH_HOST/api/v1/auth/callback
```

อย่าใช้ callback ของ GPT แทน Google callback. หาก hostname เปลี่ยน ต้องแก้ทั้ง `.env`, Google Console และ tunnel ให้เป็น hostname เดียวกัน

## 2. สร้าง image ในเครื่องและ push Docker Hub

Login ด้วยวิธีที่ไม่เขียน password ลง command history (เช่น credential helper หรือ stdin):

```sh
echo 'REPLACE_WITH_DOCKERHUB_TOKEN' | docker login --username REPLACE_WITH_DOCKERHUB_USER --password-stdin
```

แทน `REPLACE_WITH_*` ก่อนใช้ และอย่า commit token. Build สำหรับ architecture ของเครื่องแล้วโหลด image เข้า Docker Desktop:

```sh
docker buildx build \
  --tag hadesgod/garmin-fit-extractor:REPLACE_WITH_VERSION \
  --tag hadesgod/garmin-fit-extractor:latest \
  --load .
```

หากต้องการ push image ที่รองรับทั้ง server `linux/amd64` และเครื่อง ARM `linux/arm64` ให้ใช้ multi-platform build โดยตรง:

```sh
docker buildx build \
  --platform linux/amd64,linux/arm64 \
  --tag hadesgod/garmin-fit-extractor:REPLACE_WITH_VERSION \
  --tag hadesgod/garmin-fit-extractor:latest \
  --push .
```

หรือใช้ Compose สำหรับ build/push architecture เดียวกับเครื่องที่กำลังรัน Docker:

```sh
export GARMIN_FIT_IMAGE=hadesgod/garmin-fit-extractor
export GARMIN_FIT_TAG=REPLACE_WITH_VERSION
export GARMIN_FIT_PORT=3000
docker compose build app
docker compose push app
```

Compose มีทั้ง `build` และ `image` ใน service เดียวกัน จึงใช้ build จาก source หรือ push image ที่ tag ไว้ได้. หาก deploy ไป host คนละ architecture ให้ใช้คำสั่ง multi-platform ด้านบนแทน.

`REPLACE_WITH_VERSION` ควรเป็น immutable version เช่น `1.2.3`; หลีกเลี่ยงการพึ่ง `latest` ใน production เมื่อ rollback สำคัญ

## 3. GitHub Actions release tag

Workflow release ของ repository ทำงานเมื่อ push Git tag รูปแบบ `vX.Y.Z` และ publish Docker tags เป็น `X.Y.Z` (ตัด `v` ออก) กับ `latest`:

```sh
git tag vX.Y.Z
git push origin vX.Y.Z
```

แทน `X.Y.Z` ด้วย version จริง เช่น `v1.2.3`. ตรวจ GitHub Actions ให้สำเร็จก่อน deploy แล้วตั้ง `GARMIN_FIT_TAG=1.2.3` บน host (ไม่ใช่ `v1.2.3`) หรือใช้ `latest` หากยอมรับความเสี่ยงของ mutable tag.

## 4. Pull/build และ start เว็บด้วย Compose

บน host deploy คัดลอก `compose.yaml` และ `.env` ที่ไม่เปิดเผย secret แล้วเลือกวิธีใดวิธีหนึ่ง:

ใช้ image ที่ push แล้ว:

```sh
docker compose pull app
docker compose up -d app
```

หรือ build จาก source บน host:

```sh
docker compose build app
docker compose up -d app
```

ตรวจสถานะและ log:

```sh
docker compose ps
docker compose logs --tail=100 app
```

ตรวจ healthz จาก host ก่อนเปิด tunnel:

```sh
curl --fail http://127.0.0.1:${GARMIN_FIT_PORT:-3000}/healthz
```

ต้องได้ HTTP 200. หาก port host เปลี่ยน ให้ใช้ค่า `GARMIN_FIT_PORT` เดียวกับ compose. อย่าใช้ `docker compose down -v`; หากต้องหยุดชั่วคราวใช้ `docker compose stop` หรือ `docker compose down` โดยไม่ใส่ `-v`.

## 5. Cloudflare Tunnel

สร้าง tunnel ตามชื่อที่คุณควบคุม และ route hostname เดียวไปยัง app:

```sh
cloudflared tunnel route dns REPLACE_WITH_TUNNEL_NAME REPLACE_WITH_HOST
cloudflared tunnel --config REPLACE_WITH_CLOUDFLARED_CONFIG tunnel run REPLACE_WITH_TUNNEL_NAME
```

ใน config ให้ service ชี้ไปที่ `http://127.0.0.1:3000` (หรือ address ของ Docker host) และให้ tunnel ใช้ `REPLACE_WITH_HOST`. Application routing จัดการ `/oauth/*`, `/api/v1/*` และหน้า SPA/upload เอง ไม่ต้องสร้าง route แยกสำหรับแต่ละ path. Cloudflare Service Token ไม่ใช่ user identity และห้ามนำมาแทน Google session หรือ bearer OAuth ของ FIT Coach

ตรวจจากอินเทอร์เน็ต:

```sh
curl --fail https://REPLACE_WITH_HOST/healthz
```

เปิด `https://REPLACE_WITH_HOST/` แล้วทำ Google login จริง ตรวจว่า callback ที่ browser ใช้ตรงกับ:

```text
https://REPLACE_WITH_HOST/api/v1/auth/callback
```

## 6. SQLite persistence และ backup

Compose ใช้ named volume `garmin_fit_data` mount ที่ `/data`; database URL ชี้ไปที่ `/data/garmin-fit-extractor.sqlite3`. สำรองก่อน upgrade/ย้ายเครื่อง โดยหยุด app เพื่อให้ไฟล์นิ่ง:

```sh
mkdir -p backups
docker compose stop app
docker run --rm \
  -v garmin_fit_data:/data:ro \
  -v "$PWD/backups":/backup \
  alpine:3.20 \
  cp /data/garmin-fit-extractor.sqlite3 /backup/garmin-fit-extractor-REPLACE_WITH_DATE.sqlite3
docker compose start app
```

แทน `REPLACE_WITH_DATE` เช่น `2026-08-13`. เก็บ backup นอก host และทดสอบการกู้คืนตามนโยบายองค์กร. ห้ามใช้ `docker compose down -v` เพราะ `-v` ลบ `garmin_fit_data` และทำให้ SQLite, users, sessions, extractions และ history หาย

## 7. ขอบเขตของ deployment นี้

เมื่อ healthz และ Google login ผ่านแล้ว เว็บพร้อมใช้งาน: ผู้ใช้ upload FIT และอ่าน history ด้วย Google session ได้ตามปกติ. ยังไม่ต้องตั้งค่า `GARMIN_FIT_CHATGPT_*`, ไม่ต้องสร้าง Custom GPT, ไม่ต้อง import OpenAPI, ไม่ต้องตั้ง OAuth callback ของ GPT และไม่ต้องสร้าง fake credentials. เมื่อพร้อมทำ integration ภายหลัง ค่อยใช้ client ID `FIT_COACH_CHATGPT`, secret จริงที่เก็บนอก repository และ callback URL exact ที่ GPT editor แสดง; อย่าใส่ค่าดังกล่าวลงคู่มือนี้หรือใน image
