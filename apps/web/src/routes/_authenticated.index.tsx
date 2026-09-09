import { Link, createFileRoute } from "@tanstack/react-router";

import { catalog } from "../data/catalog";
import { ShoeImage } from "../features/shoes/ShoeImage";

export const Route = createFileRoute("/_authenticated/")({ component: HomePage });

const featuredShoe =
  catalog.shoes.find(({ id }) => id === "asics-gel-kayano-33") ?? catalog.shoes[0]!;
const targetShoe = catalog.shoes.find(
  ({ id }) => id === "new-balance-fuelcell-supercomp-elite-v6",
);
const referenceShoe = catalog.shoes.find(({ id }) => id === "asics-novablast-6");

function HomePage() {
  return (
    <div className="home-page garage-page">
      <section className="garage-hero" aria-labelledby="home-title">
        <div className="garage-hero-copy">
          <p className="garage-wordmark">Runner’s Garage</p>
          <h1 id="home-title">เรื่องวิ่งของคุณ มีอะไรให้ดูมากกว่าที่คิด</h1>
          <p className="garage-hero-lede">
            พื้นที่รวมของเล่นสำหรับคนวิ่ง แกะข้อมูลจากนาฬิกา ดูตัวเลขที่ซ่อนอยู่
            และใช้ประสบการณ์ของนักวิ่งคนอื่นช่วยเทียบไซซ์รองเท้า
          </p>
          <div className="garage-actions" aria-label="เลือกพื้นที่ทำงาน">
            <Link className="button" data-testid="home-runs-cta" to="/history" search={{ offset: 0, order: "desc" }}>
              ดู Runs
            </Link>
            <Link className="button quiet" data-testid="home-shoes-cta" to="/shoes">
              ดูเรื่องรองเท้า
            </Link>
          </div>
        </div>

        <div className="garage-hero-workbench" aria-label="Run data และ Shoe size">
          <section className="garage-readout garage-readout--runs" aria-labelledby="hero-runs-title">
            <div className="garage-readout-topline">
              <span id="hero-runs-title">RUN DATA</span>
              <code>inspect</code>
            </div>
            <div className="garage-chart" data-testid="home-run-visualization">
              <svg viewBox="0 0 440 150" role="img" aria-labelledby="home-chart-title home-chart-description">
                <title id="home-chart-title">ตัวอย่างเส้นข้อมูลจากกิจกรรมวิ่ง</title>
                <desc id="home-chart-description">
                  เส้นภาพประกอบของข้อมูล pace, heart rate และ cadence โดยไม่แสดงค่าผลลัพธ์แทนข้อมูลจริง
                </desc>
                <path className="garage-chart-grid" d="M0 30H440M0 75H440M0 120H440" />
                <path className="garage-chart-line garage-chart-line--pace" d="M0 101 C34 91 42 112 70 84 S112 49 139 73 S180 97 208 55 S249 28 281 63 S321 109 348 72 S395 46 440 22" />
                <path className="garage-chart-line garage-chart-line--heart" d="M0 120 C32 115 48 118 73 103 S112 97 137 109 S175 82 204 91 S244 120 272 101 S314 89 344 99 S390 79 440 86" />
                <path className="garage-chart-line garage-chart-line--cadence" d="M0 56 C40 60 55 44 84 55 S124 72 151 50 S195 42 221 52 S264 74 290 48 S328 38 356 47 S404 64 440 45" />
              </svg>
              <div className="garage-chart-labels" aria-label="ชนิดข้อมูลในภาพประกอบ">
                <span><i className="garage-chart-dot garage-chart-dot--pace" />pace</span>
                <span><i className="garage-chart-dot garage-chart-dot--heart" />heart rate</span>
                <span><i className="garage-chart-dot garage-chart-dot--cadence" />cadence</span>
              </div>
            </div>
            <p className="garage-readout-note">ดูสิ่งที่ Garmin FIT เก็บไว้ ก่อนค่อยถามคำถามต่อ</p>
          </section>

          <section className="garage-readout garage-readout--shoes" aria-labelledby="hero-shoes-title">
            <div className="garage-readout-topline">
              <span id="hero-shoes-title">SHOE SIZE</span>
              <code>compare</code>
            </div>
            <div className="garage-shoe-object">
              <ShoeImage
                alt={`${featuredShoe.brand} ${featuredShoe.model}`}
                imageUrl={featuredShoe.imageUrl}
                priority
              />
            </div>
            <div className="garage-shoe-caption">
              <strong>{featuredShoe.brand} {featuredShoe.model}</strong>
              <span>reviews · size chart · comparison</span>
            </div>
          </section>
        </div>
      </section>

      <section className="garage-section garage-inventory" aria-labelledby="inventory-title">
        <div className="garage-section-heading">
          <p className="garage-section-kicker">WHAT’S IN THE GARAGE?</p>
          <h2 id="inventory-title">หยิบใช้ทีละเรื่อง ไม่ต้องเปิดทุกอย่างพร้อมกัน</h2>
        </div>
        <div className="garage-inventory-grid">
          <article className="garage-area garage-area--runs" aria-labelledby="runs-area-title">
            <div className="garage-area-header">
              <span className="garage-area-index">01</span>
              <span className="garage-area-type">RUN DATA</span>
            </div>
            <div className="garage-area-copy">
              <h3 id="runs-area-title">เก็บข้อมูลของเราเอง ดูให้ลึกขึ้น</h3>
              <p className="garage-area-product">Garmin FIT Extractor</p>
              <p>
                นำไฟล์ ZIP จาก Garmin Connect มาอ่านทั้งภาพรวมและรายละเอียดที่อยู่ในกิจกรรม
                แล้วเก็บผลลัพธ์ไว้กลับมาดูใน Runs
              </p>
              <ul className="garage-list">
                <li>Normalized สำหรับเริ่มอ่านกิจกรรม</li>
                <li>Raw JSON สำหรับเปิดดูข้อมูลทั้งชุด</li>
                <li>ดาวน์โหลดไปอ่านต่อกับ ChatGPT หรือ Claude</li>
              </ul>
            </div>
            <Link className="garage-arrow-link" to="/history" search={{ offset: 0, order: "desc" }}>
              ดู Runs <span aria-hidden="true">↗</span>
            </Link>
          </article>

          <article className="garage-area garage-area--shoes" aria-labelledby="shoes-area-title">
            <div className="garage-area-header">
              <span className="garage-area-index">02</span>
              <span className="garage-area-type">SHOE SIZE</span>
            </div>
            <div className="garage-area-copy">
              <h3 id="shoes-area-title">ไซซ์เดียวกัน ไม่ได้แปลว่าฟิตเหมือนกัน</h3>
              <p className="garage-area-product">Running Shoe Library</p>
              <p>
                รองเท้าสองคู่ที่เขียนไซซ์เท่ากัน อาจเหลือพื้นที่หน้าเท้าไม่เท่ากันเลย
                เริ่มจากคู่ที่คุณรู้ไซซ์ แล้วดู reviewer ที่เคยใส่ทั้งสองรุ่น
              </p>
              <ul className="garage-list">
                <li>ค้นหาจากแบรนด์และรุ่น</li>
                <li>อ่านชื่อ reviewer และแหล่งข้อมูลต้นทาง</li>
                <li>เทียบกับคู่ที่คุณมีเมื่อมี bridge รองรับ</li>
              </ul>
            </div>
            <Link
              className="garage-arrow-link"
              to="/shoes/$shoeId"
              params={{ shoeId: "new-balance-fuelcell-supercomp-elite-v6" }}
            >
              เทียบไซซ์รองเท้า <span aria-hidden="true">↗</span>
            </Link>
          </article>
        </div>
      </section>

      <section className="garage-note" aria-labelledby="garage-note-title">
        <p className="garage-note-mark" aria-hidden="true">✳</p>
        <div>
          <h2 id="garage-note-title">Built from curiosity, mileage, and too many tabs about shoes.</h2>
          <p>
            ที่นี่ไม่ต้องรีบสรุปทุกอย่างให้เป็นคำตอบเดียว บางวันเราอยากแกะข้อมูลจาก Run
            บางวันเราอยากเปิดตารางไซซ์ของรองเท้าคู่หนึ่ง แล้วค่อยตัดสินใจด้วยข้อมูลที่มี
          </p>
        </div>
      </section>

      <section className="garage-section garage-run-detail" aria-labelledby="run-detail-title">
        <div className="garage-section-heading garage-section-heading--split">
          <div>
            <p className="garage-section-kicker">RUN DATA</p>
            <h2 id="run-detail-title">นาฬิกาเก็บอะไรไว้บ้าง?</h2>
          </div>
          <p>
            เริ่มจากสรุปที่อ่านง่าย แล้วเปิดรายละเอียดเมื่ออยากรู้ว่ากิจกรรมหนึ่งครั้งประกอบด้วยอะไรบ้าง
          </p>
        </div>
        <div className="garage-run-detail-grid">
          <div className="garage-data-register" aria-label="ตัวอย่างกลุ่มข้อมูลจาก Garmin FIT">
            <div className="garage-register-line"><span>activity</span><strong>เวลารวม · ระยะทาง · ชนิดกิจกรรม</strong></div>
            <div className="garage-register-line"><span>laps</span><strong>รายละเอียดแยกตามรอบ</strong></div>
            <div className="garage-register-line"><span>heart rate</span><strong>ค่าเฉลี่ย · ค่าสูงสุด · โซน</strong></div>
            <div className="garage-register-line"><span>running dynamics</span><strong>cadence · stride length</strong></div>
            <div className="garage-register-line"><span>environment</span><strong>elevation · temperature · calories</strong></div>
          </div>
          <div className="garage-run-detail-copy">
            <p>
              ข้อมูลบางอย่างเหมาะกับการดูเป็นภาพรวม บางอย่างต้องเปิด Raw JSON เพื่อให้บริบทไม่หาย
              Runner’s Garage แยกสองมุมมองนี้ไว้ให้เดินต่อจากไฟล์เดียวกันได้
            </p>
            <Link className="button" to="/upload">เพิ่มข้อมูลวิ่ง</Link>
          </div>
        </div>
      </section>

      <section className="garage-section garage-shoe-detail" aria-labelledby="shoe-detail-title">
        <div className="garage-section-heading garage-section-heading--split">
          <div>
            <p className="garage-section-kicker">SHOE SIZE</p>
            <h2 id="shoe-detail-title">ไซซ์ที่ดีควรอธิบายได้ว่ามาจากไหน</h2>
          </div>
          <p>
            ตารางไซซ์ช่วยแปลป้ายไซซ์ ส่วน reviewer bridge ช่วยบอกความสัมพันธ์จากคนที่เคยใส่ทั้งสองรุ่น
            ถ้าไม่มีข้อมูลตรง เราจะบอกว่าไม่มี
          </p>
        </div>
        <div className="garage-shoe-proof-grid">
          <article className="garage-proof-card">
            <span className="garage-proof-label">EVIDENCE FIRST</span>
            <h3>{referenceShoe ? `${referenceShoe.brand} ${referenceShoe.model}` : "Reference shoe"}</h3>
            <p>เลือกคู่ที่คุณรู้ไซซ์อยู่แล้ว แล้วดูว่ามี reviewer คนไหนเชื่อมไปยังคู่เป้าหมายได้บ้าง</p>
          </article>
          <article className="garage-proof-card garage-proof-card--accent">
            <span className="garage-proof-label">KNOWN TARGET</span>
            <h3>{targetShoe ? `${targetShoe.brand} ${targetShoe.model}` : "Target shoe"}</h3>
            <p>ผลลัพธ์จะบอกระบบไซซ์ ค่าที่แนะนำ และจำนวนเสียงที่รองรับ ไม่ทำเป็นคะแนนความมั่นใจลอย ๆ</p>
          </article>
        </div>
        <div className="garage-shoe-detail-footer">
          <ShoeImage alt={`${featuredShoe.brand} ${featuredShoe.model}`} imageUrl={featuredShoe.imageUrl} compact />
          <div>
            <p>เปิดรุ่นที่สนใจ แล้วเริ่มจากคำถามเรื่องไซซ์ได้เลย</p>
            <Link className="garage-arrow-link" to="/shoes">เปิดคลังรองเท้า <span aria-hidden="true">↗</span></Link>
          </div>
        </div>
      </section>

      <section className="garage-section garage-ai" aria-labelledby="ai-title">
        <div className="garage-ai-copy">
          <p className="garage-section-kicker">OPTIONAL HANDOFF</p>
          <h2 id="ai-title">AI เป็นผู้ช่วยอ่านข้อมูลต่อ ไม่ใช่เจ้าของข้อมูล</h2>
          <p>
            เมื่ออยากถามคำถามที่เฉพาะขึ้น คัดลอกหรือดาวน์โหลด Raw JSON ไปใช้กับ ChatGPT หรือ Claude ได้
            ข้อมูลยังเป็นของคุณ และคุณเป็นคนเลือกว่าจะส่งอะไรออกไป
          </p>
        </div>
        <div className="garage-ai-register" aria-label="เครื่องมือที่ใช้ต่อจากข้อมูล Run">
          <span>YOUR RUN</span>
          <div className="garage-ai-tools">
            <span>FIT Coach</span>
            <span>Raw Data</span>
          </div>
          <Link to="/history" search={{ offset: 0, order: "desc" }}>ดู Runs ↗</Link>
        </div>
      </section>

      <footer className="garage-footer">
        <div>
          <strong>Runner’s Garage</strong>
          <span>by น้ำเน่ารันคลับ</span>
        </div>
        <p>สองพื้นที่ทำงานสำหรับเรื่องวิ่งที่อยากเก็บไว้ใช้จริง</p>
      </footer>
    </div>
  );
}
