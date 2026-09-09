import { Link } from "@tanstack/react-router";

import {
  formatReleaseDate,
  type Catalog,
  type Review,
  type Shoe,
  type SizeSystem,
} from "../../domain/catalog";
import { aggregateGeneralRecommendation } from "../../domain/recommendations";
import { RecommendationPanel } from "./RecommendationPanel";
import { ShoeImage } from "./ShoeImage";

type ShoeDetailPageProps = {
  catalog: Catalog;
  shoe?: Shoe;
};

const SIZE_SYSTEM_LABELS: Record<SizeSystem, string> = {
  US_M: "US Men’s",
  US_W: "US Women’s",
  UK: "UK",
  EU: "EU",
  CM: "CM",
};

const SIZE_SYSTEMS: readonly SizeSystem[] = ["US_M", "US_W", "UK", "EU", "CM"];

function stepLabel(stepDelta: number) {
  if (stepDelta === 0) return "ใช้ป้ายไซซ์เดิม";
  return stepDelta > 0
    ? `ขยับขึ้น ${stepDelta} ระดับจากไซซ์เดิม`
    : `ขยับลง ${Math.abs(stepDelta)} ระดับจากไซซ์เดิม`;
}

function triedSizeLabel(review: Review) {
  return review.triedSize
    ? `${SIZE_SYSTEM_LABELS[review.triedSize.system]} ${review.triedSize.value}`
    : null;
}

function ShoeNotFound() {
  return (
    <section className="empty-state shoe-not-found" aria-labelledby="shoe-not-found-title">
      <span className="empty-state-mark" aria-hidden="true">?</span>
      <p className="shoe-eyebrow">SHOES / NOT FOUND</p>
      <h1 id="shoe-not-found-title">ไม่พบรองเท้ารุ่นนี้ในรายการ</h1>
      <p>ลิงก์อาจหมดอายุ หรือรุ่นนี้ยังไม่อยู่ในแคตตาล็อกที่ตรวจสอบแล้ว</p>
      <Link className="button secondary" to="/shoes">
        กลับไปดูรุ่นทั้งหมด
      </Link>
    </section>
  );
}

function ReviewCard({ review, catalog }: { review: Review; catalog: Catalog }) {
  const reviewer = catalog.reviewers.find(({ id }) => id === review.reviewerId);
  const triedSize = triedSizeLabel(review);
  return (
    <article className="shoe-review-card">
      <div className="shoe-review-heading">
        <div>
          <p className="shoe-reviewer-name">{reviewer?.name ?? "ผู้รีวิว"}</p>
          <p className="shoe-review-source-note">ข้อมูลจากแหล่งต้นทางที่เปิดดูได้</p>
        </div>
        {triedSize ? <span className="shoe-review-size">ใส่ {triedSize}</span> : null}
      </div>
      <p className="shoe-review-summary">{review.summary}</p>
      {review.recommendation ? (
        <p className="shoe-review-recommendation">
          <strong>คำแนะนำเรื่องไซซ์:</strong> {stepLabel(review.recommendation.stepDelta)}
        </p>
      ) : (
        <p className="shoe-review-evidence-limit">รีวิวนี้ไม่ได้มีคำแนะนำการขยับไซซ์ที่ใช้เป็นเสียงรวม</p>
      )}
      {review.comparisons && review.comparisons.length > 0 ? (
        <ul className="shoe-review-comparisons" aria-label="การเปรียบเทียบที่ผู้รีวิวระบุ">
          {review.comparisons.map((comparison) => {
            const comparisonShoe = catalog.shoes.find(({ id }) => id === comparison.shoeId);
            return (
              <li key={comparison.shoeId}>
                เทียบกับ {comparisonShoe?.brand} {comparisonShoe?.model}: {stepLabel(comparison.stepDelta)}
              </li>
            );
          })}
        </ul>
      ) : null}
      <div className="shoe-review-links">
        {review.sources.map((source, index) => (
          <a href={source} key={source} rel="noreferrer" target="_blank">
            เปิดแหล่งข้อมูล{review.sources.length > 1 ? ` ${index + 1}` : ""} <span aria-hidden="true">↗</span>
          </a>
        ))}
      </div>
    </article>
  );
}

function SizeChartTable({
  chart,
  shoe,
  index,
}: {
  chart: Catalog["sizeCharts"][number];
  shoe: Shoe;
  index: number;
}) {
  const systems = SIZE_SYSTEMS.filter((system) => chart.rows.some((row) => row[system] !== undefined));
  const audienceLabel = chart.audience === "unisex" ? "Unisex" : chart.audience === "men" ? "Men’s" : "Women’s";

  return (
    <div className="shoe-size-chart-wrap">
      <table className="shoe-size-chart">
        <caption>
          <span className="shoe-size-chart-caption-title">
            {shoe.brand} · {audienceLabel} size chart{index > 0 ? ` ${index + 1}` : ""}
          </span>
          <a href={chart.sourceUrl} rel="noreferrer" target="_blank">เปิดแหล่งข้อมูล ↗</a>
        </caption>
        <thead>
          <tr>
            {systems.map((system) => (
              <th key={system} scope="col">{SIZE_SYSTEM_LABELS[system]}</th>
            ))}
          </tr>
        </thead>
        <tbody>
          {chart.rows.map((row, rowIndex) => (
            <tr key={`${chart.id}-${rowIndex}`}>
              {systems.map((system) => (
                <td data-label={SIZE_SYSTEM_LABELS[system]} key={system}>{row[system] ?? "—"}</td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

function ReviewSummary({ reviews }: { reviews: Review[] }) {
  const general = aggregateGeneralRecommendation(reviews);

  return (
    <section className="shoe-detail-section shoe-recommendation-summary" aria-labelledby="shoe-summary-title">
      <div className="shoe-section-heading">
        <div>
          <p className="shoe-eyebrow">SINGLE-SHOE REVIEWS</p>
          <h2 id="shoe-summary-title">คำแนะนำที่มีในรีวิวของรุ่นนี้</h2>
        </div>
        <span className="status-badge">
          <span className="status-dot" aria-hidden="true" />
          {general.usableReviewerCount} ความเห็นที่ใช้สรุป
        </span>
      </div>
      {general.status === "empty" ? (
        <div className="shoe-summary-empty">
          <h3>รุ่นนี้ยังไม่มีคำแนะนำไซซ์แบบสรุปตรง ๆ</h3>
          <p>ดูผลเทียบกับ reference shoe ด้านบนแทนได้ เมื่อมี bridge ที่อธิบายที่มาได้</p>
        </div>
      ) : (
        <div className="shoe-summary-content">
          <h3>
            {general.status === "split"
              ? general.majorityStepDelta === undefined
                ? "ผู้รีวิวให้ความเห็นเรื่องไซซ์คนละทาง"
                : `ผู้รีวิวส่วนใหญ่${stepLabel(general.majorityStepDelta)}`
              : `ผู้รีวิว${stepLabel(general.majorityStepDelta ?? 0)}`}
          </h3>
          <ul aria-label="สรุปความเห็นเรื่องไซซ์">
            {general.groups.map((group) => (
              <li key={group.stepDelta}>
                <span>{stepLabel(group.stepDelta)}</span>
                <strong>{group.count} คน</strong>
              </li>
            ))}
          </ul>
          <p className="shoe-summary-note">นับหนึ่งคนต่อหนึ่งความเห็น และไม่นับรีวิวที่ไม่ได้ให้คำแนะนำเรื่องไซซ์</p>
        </div>
      )}
    </section>
  );
}

export function ShoeDetailPage({ catalog, shoe }: ShoeDetailPageProps) {
  if (!shoe) return <ShoeNotFound />;

  const shoeReviews = catalog.reviews.filter((review) => review.shoeId === shoe.id);
  const charts = catalog.sizeCharts.filter(
    (chart) => chart.brand.trim().toLocaleLowerCase() === shoe.brand.trim().toLocaleLowerCase(),
  );

  return (
    <article className="shoe-detail-page" aria-labelledby="shoe-detail-title" data-shoe-id={shoe.id}>
      <Link className="shoe-back-link" to="/shoes">
        ← Shoes
      </Link>
      <header className="shoe-detail-header">
        <ShoeImage alt={`${shoe.brand} ${shoe.model}`} imageUrl={shoe.imageUrl} priority />
        <div className="shoe-detail-heading">
          <p className="shoe-eyebrow">{shoe.brand}</p>
          <h1 id="shoe-detail-title">{shoe.model}</h1>
          <p className="shoe-detail-question">กำลังหาว่าคู่นี้ควรใส่ไซซ์อะไร?</p>
          <p className="shoe-detail-lede">
            เริ่มจากคู่ที่คุณรู้ไซซ์ของตัวเอง แล้วอ่านผลเทียบจาก reviewer, ตารางไซซ์ และแหล่งข้อมูลต้นทาง
          </p>
          <div className="shoe-detail-facts">
            <span>เริ่มขาย {formatReleaseDate(shoe.releaseDate)}</span>
            <a className="shoe-fact-source" href={shoe.releaseDate.sourceUrl} rel="noreferrer" target="_blank">
              ดูแหล่งข้อมูลวันวางขาย <span aria-hidden="true">↗</span>
            </a>
          </div>
        </div>
      </header>

      <RecommendationPanel catalog={catalog} targetShoeId={shoe.id} />

      <ReviewSummary reviews={shoeReviews} />

      {shoeReviews.length > 0 ? (
        <section className="shoe-detail-section" aria-labelledby="shoe-reviews-title">
          <div className="shoe-section-heading">
            <div>
              <p className="shoe-eyebrow">FIELD REPORTS</p>
              <h2 id="shoe-reviews-title">คนใส่จริงพูดถึงอะไรไว้บ้าง</h2>
              <p>อ่านสรุปของแต่ละคน และเปิดแหล่งข้อมูลต้นทางได้จากการ์ดแต่ละใบ</p>
            </div>
          </div>
          <div className="shoe-reviews-list">
            {shoeReviews.map((review) => (
              <ReviewCard catalog={catalog} key={review.id} review={review} />
            ))}
          </div>
        </section>
      ) : null}

      {charts.length > 0 ? (
        <section className="shoe-detail-section" aria-labelledby="shoe-charts-title">
          <div className="shoe-section-heading">
            <div>
              <p className="shoe-eyebrow">OFFICIAL SIZE CHART</p>
              <h2 id="shoe-charts-title">ตารางไซซ์ทางการ</h2>
              <p>แถวในตารางคงลำดับจากแหล่งข้อมูลเดิม ใช้แปลป้ายไซซ์ ไม่ใช่การรับประกันว่ารองเท้าจะพอดี</p>
            </div>
          </div>
          <div className="shoe-size-charts">
            {charts.map((chart, index) => (
              <SizeChartTable chart={chart} index={index} key={chart.id} shoe={shoe} />
            ))}
          </div>
        </section>
      ) : null}

      <aside className="shoe-trust-note" aria-labelledby="shoe-trust-title">
        <p className="shoe-eyebrow">READ THE EVIDENCE</p>
        <h2 id="shoe-trust-title">ผลนี้คือจุดเริ่มต้น ไม่ใช่คำรับรองฟิต</h2>
        <p>
          ตารางไซซ์บอกลำดับป้ายไซซ์ ส่วน bridge บอกความสัมพันธ์ที่หาได้จาก reviewer ใน catalog
          ถ้าไม่มีข้อมูลตรง เราจะไม่สร้างคำตอบขึ้นมาแทนคุณ
        </p>
        <p>
          เปิดลิงก์ต้นทางเพื่ออ่านบริบทเต็ม และลองรองเท้าจริงเมื่อทำได้ โดยเฉพาะเมื่อทรงเท้าและความรู้สึกของคุณต่างจาก reviewer
        </p>
      </aside>
    </article>
  );
}
