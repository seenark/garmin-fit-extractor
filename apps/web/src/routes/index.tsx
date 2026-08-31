import { Link, createFileRoute } from "@tanstack/react-router";

export const Route = createFileRoute("/")({ component: HomePage });

function HomePage() {
  return (
    <div className="home-page">
      <section className="home-hero" aria-labelledby="home-title">
        <div className="home-hero-copy">
          <p className="home-kicker">ดูข้อมูลจาก Garmin ให้ละเอียดขึ้น</p>
          <h1 id="home-title">ข้อมูลที่ Garmin บันทึกไว้ ยังดูได้ละเอียดกว่านี้</h1>
          <p className="home-lede">
            ส่งออกไฟล์ ZIP จาก Garmin แล้วเปิดดูข้อมูล FIT แบบละเอียดได้เลย
            ถ้าสรุปที่มีอยู่ยังตอบคำถามไม่พอ ก็คัดลอกข้อมูลดิบ (Raw JSON)
            ไปถาม ChatGPT หรือ Claude ต่อได้
          </p>
          <div className="home-actions">
            <Link
              className="button home-primary-action"
              data-testid="home-upload-cta"
              to="/upload"
            >
              อัปโหลดไฟล์ ZIP
            </Link>
            <Link
              className="button secondary"
              data-testid="home-history-cta"
              to="/history"
              search={{ offset: 0, order: "desc" }}
            >
              ดูประวัติ
            </Link>
          </div>
          <p className="home-constraint-note">
            อัปโหลดครั้งละ 1–10 ไฟล์ ZIP · ไฟล์ละไม่เกิน 20 เมกะไบต์
          </p>
        </div>

        <figure
          className="home-workflow"
          data-testid="home-workflow"
          aria-labelledby="workflow-title"
        >
          <figcaption id="workflow-title" className="home-workflow-heading">
            จากไฟล์ส่งออกไปจนถึงข้อมูลพร้อมถาม AI
          </figcaption>
          <div className="workflow-board">
            <div className="workflow-node workflow-node--source">
              <span className="workflow-node-label">01 / ไฟล์จาก Garmin</span>
              <span className="workflow-file">
                <span className="workflow-file-mark" aria-hidden="true">
                  ZIP
                </span>
                <span>activity.zip</span>
              </span>
              <span className="workflow-node-note">ไฟล์ส่งออกจาก Garmin</span>
            </div>

            <div className="workflow-connector" aria-hidden="true">
              <span />
            </div>

            <div className="workflow-node workflow-node--extract">
              <span className="workflow-node-label">02 / แยกข้อมูล FIT</span>
              <span className="workflow-file">
                <span className="workflow-file-mark" aria-hidden="true">
                  FIT
                </span>
                <span>activity.fit</span>
              </span>
              <span className="workflow-node-note">ข้อมูลกิจกรรมแบบละเอียด</span>
            </div>

            <div className="workflow-branch" aria-hidden="true">
              <span />
              <span />
            </div>

            <div className="workflow-outputs">
              <div className="workflow-output workflow-output--analysis">
                <span className="workflow-output-label">วิเคราะห์แบบมาตรฐาน (Normalized)</span>
                <strong>ตัวเลขอ่านง่าย</strong>
                <span>รอบ · เพซ · อัตราการเต้นหัวใจ</span>
              </div>
              <div className="workflow-output workflow-output--raw">
                <span className="workflow-output-label">ข้อมูลดิบ (Raw JSON)</span>
                <strong>เห็นข้อมูลทุกฟิลด์</strong>
                <span>คัดลอกหรือดาวน์โหลดได้</span>
              </div>
            </div>

            <div className="workflow-handoff">
              <span className="workflow-node-label">03 / ส่งต่อให้ AI</span>
              <div className="workflow-ai-list">
                <span>ChatGPT</span>
                <span>Claude</span>
              </div>
              <span className="workflow-node-note">ถามรายละเอียดจากข้อมูลได้มากขึ้น</span>
            </div>
          </div>
          <p className="home-workflow-caption">ไฟล์เดียว ดูได้สองแบบ</p>
        </figure>
      </section>

      <section className="home-section home-process" aria-labelledby="process-title">
        <div className="home-section-heading">
          <h2 id="process-title">เปลี่ยนไฟล์จาก Garmin ให้เป็นข้อมูลที่ถามต่อได้</h2>
          <p>
            ขั้นตอนมีแค่นี้: เอาไฟล์มา ดูสิ่งที่นาฬิกาบันทึกไว้
            แล้วเลือกมุมมองที่ตรงกับสิ่งที่อยากรู้
          </p>
        </div>

        <ol className="home-steps">
          <li className="home-step">
            <span className="home-step-number" aria-hidden="true">
              1.0
            </span>
            <div className="home-step-copy">
              <h3>1. ส่งออกไฟล์จาก Garmin</h3>
              <p>
                เริ่มจากไฟล์ ZIP ที่ดาวน์โหลดจากเว็บไซต์ Garmin
                จะอัปโหลดกิจกรรมเดียวหรือหลายไฟล์พร้อมกันก็ได้
                โดยไม่แตะต้องไฟล์ต้นฉบับ
              </p>
            </div>
            <div className="home-step-proof" aria-label="ไฟล์เข้า">
              <span className="home-proof-label">ไฟล์เข้า</span>
              <code>activity.zip</code>
              <span>ไฟล์ส่งออกจาก Garmin</span>
            </div>
          </li>
          <li className="home-step">
            <span className="home-step-number" aria-hidden="true">
              2.0
            </span>
            <div className="home-step-copy">
              <h3>ดูข้อมูลที่ซ่อนอยู่หลังตัวเลขสรุป</h3>
              <p>
                เปิดดูข้อมูลแบบจัดรูปแบบมาตรฐาน (Normalized) ได้ทั้งสรุปกิจกรรม รอบ เพซ
                อัตราการเต้นหัวใจ กำลัง รอบขา ความสูง อุณหภูมิ และแคลอรี
              </p>
            </div>
            <div className="home-step-proof home-step-proof--metrics" aria-label="ฟิลด์วิเคราะห์ข้อมูล">
              <span className="home-proof-label">วิเคราะห์</span>
              <span>สรุป</span>
              <span>รอบ</span>
              <span>ไดนามิกการวิ่ง</span>
            </div>
          </li>
          <li className="home-step">
            <span className="home-step-number" aria-hidden="true">
              3.0
            </span>
            <div className="home-step-copy">
              <h3>เอาข้อมูลดิบไปถามต่อ</h3>
              <p>
                เปิดหรือดาวน์โหลดข้อมูลดิบ (Raw JSON) แล้วนำไปวางใน ChatGPT หรือ Claude
                ถ้าคำถามต้องใช้บริบทมากกว่าสรุปทั่วไป
              </p>
            </div>
            <div className="home-step-proof home-step-proof--json" aria-label="การส่งต่อ Raw JSON">
              <span className="home-proof-label">ส่งต่อ</span>
              <code>{'{ "activity": … }'}</code>
              <span>คัดลอกหรือดาวน์โหลด</span>
            </div>
          </li>
        </ol>
      </section>

      <section className="home-section home-depth" aria-labelledby="depth-title">
        <div className="home-depth-intro">
          <h2 id="depth-title">ดูรายละเอียดเพิ่ม โดยไม่ทิ้งข้อมูลต้นฉบับ</h2>
          <p>
            Garmin บันทึกข้อมูลไว้ละเอียดอยู่แล้ว แอปนี้ช่วยจัดให้อยู่ในรูปที่เปิดดู
            บันทึก และนำไปใช้กับเครื่องมือที่คุณคุ้นเคยได้ง่ายขึ้น
          </p>
        </div>
        <ul className="home-benefits">
          <li>
            <h3>เริ่มจากข้อมูลที่อ่านง่าย</h3>
            <p>
              เริ่มจากชื่อฟิลด์ที่สม่ำเสมอและหน่วยที่อ่านง่าย
              ไม่ต้องไล่ดูไฟล์ที่ยังจัดรูปแบบไม่เรียบร้อย
            </p>
          </li>
          <li>
            <h3>ใช้ข้อมูลดิบ (Raw JSON) เมื่อต้องการรายละเอียด</h3>
            <p>
              เปิดข้อมูลดิบ (Raw JSON) ได้ทุกเมื่อที่คำถามต้องใช้ข้อมูลทั้งชุด
              ไม่ใช่แค่ค่าที่อยู่ในสรุป
            </p>
          </li>
          <li>
            <h3>กลับมาดูประวัติได้</h3>
            <p>
              รายการที่แยกข้อมูลไว้จะยังอยู่ให้กลับมาเปิดดูทีหลัง
              งานจึงไม่จบแค่ตอนตอบคำถามแรกเสร็จ
            </p>
          </li>
        </ul>
      </section>

      <section className="home-closing" aria-labelledby="closing-title">
        <div>
          <h2 id="closing-title">เริ่มจากไฟล์ที่มีอยู่ได้เลย</h2>
          <p>
            อัปโหลด ZIP เลือกมุมมอง แล้วเอาข้อมูลไปใช้กับคำถามต่อไป
          </p>
        </div>
        <div className="home-actions home-closing-actions">
          <Link className="button" to="/upload">
            อัปโหลดไฟล์ ZIP
          </Link>
          <Link
            className="button quiet"
            to="/history"
            search={{ offset: 0, order: "desc" }}
          >
            ดูประวัติ
          </Link>
        </div>
      </section>
    </div>
  );
}
