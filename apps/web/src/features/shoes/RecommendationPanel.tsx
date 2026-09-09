import { useMemo, useState, type FormEvent } from "react";

import type { Catalog, Review, Size, SizeSystem } from "../../domain/catalog";
import {
  buildPersonalizedComparison,
  type BridgeSource,
  type PersonalizedComparison,
  type ReviewerBridge,
} from "../../domain/recommendations";

import { ShoeCombobox } from "./ShoeCombobox";

const SIZE_SYSTEM_LABELS: Record<SizeSystem, string> = {
  US_M: "US Men’s",
  US_W: "US Women’s",
  UK: "UK",
  EU: "EU",
  CM: "CM",
};

const SIZE_SYSTEMS: readonly SizeSystem[] = ["US_M", "US_W", "UK", "EU", "CM"];

type FitFeeling = "good" | "snug" | "roomy";

const FIT_FEELING_LABELS: Record<FitFeeling, string> = {
  good: "พอดี",
  snug: "คับนิดหน่อย",
  roomy: "หลวมหน่อย",
};

type RecommendationPanelProps = {
  catalog: Catalog;
  targetShoeId: string;
};

function stepLabel(stepDelta: number) {
  if (stepDelta === 0) return "ใช้ป้ายไซซ์เดิม";
  return stepDelta > 0
    ? `ขยับขึ้น ${stepDelta} ระดับไซซ์`
    : `ขยับลง ${Math.abs(stepDelta)} ระดับไซซ์`;
}

function bridgeSourceLabel(source: BridgeSource) {
  switch (source) {
    case "direct":
      return "เทียบตรงจากรีวิว";
    case "confirmed-size":
      return "ไซซ์ที่ผู้รีวิวใส่จริง";
    case "relative-fit":
      return "คำแนะนำเรื่องไซซ์";
  }
}

function bridgeEvidenceKind(source: BridgeSource) {
  switch (source) {
    case "direct":
      return "Direct evidence";
    case "confirmed-size":
      return "Direct bridge";
    case "relative-fit":
      return "Indirect evidence";
  }
}

function bridgeSourceDescription(source: BridgeSource) {
  switch (source) {
    case "direct":
      return "ผู้รีวิวระบุความสัมพันธ์ของรองเท้าสองรุ่นนี้ไว้โดยตรง";
    case "confirmed-size":
      return "เทียบจากไซซ์ที่ผู้รีวิวระบุว่าใส่ในทั้งสองรุ่น โดยมีตารางไซซ์ช่วยเรียงลำดับ";
    case "relative-fit":
      return "เทียบจากคำแนะนำเรื่องไซซ์ของผู้รีวิวในแต่ละรุ่น ไม่ใช่การเทียบตรง";
  }
}

function formatSize(size: Size) {
  return `${SIZE_SYSTEM_LABELS[size.system]} ${size.value}`;
}

function endpointLabel(
  size: ReviewerBridge["referenceTriedSize"],
  recommendation: Review["recommendation"],
) {
  if (size) return formatSize(size);
  return recommendation ? `คำแนะนำ: ${stepLabel(recommendation.stepDelta)}` : "ไม่มีไซซ์ที่ผู้รีวิวระบุไว้";
}

function BridgeEvidence({
  bridge,
  catalog,
  referenceShoeId,
  targetShoeId,
}: {
  bridge: ReviewerBridge;
  catalog: Catalog;
  referenceShoeId: string;
  targetShoeId: string;
}) {
  const reviewer = catalog.reviewers.find(({ id }) => id === bridge.reviewerId);
  const referenceShoe = catalog.shoes.find(({ id }) => id === referenceShoeId);
  const targetShoe = catalog.shoes.find(({ id }) => id === targetShoeId);
  const targetReview =
    (bridge.targetReviewId
      ? catalog.reviews.find(({ id }) => id === bridge.targetReviewId)
      : undefined) ??
    catalog.reviews.find(
      (review) => review.reviewerId === bridge.reviewerId && review.shoeId === targetShoeId,
    );
  const referenceReview =
    (bridge.referenceReviewId
      ? catalog.reviews.find(({ id }) => id === bridge.referenceReviewId)
      : undefined) ??
    catalog.reviews.find(
      (review) => review.reviewerId === bridge.reviewerId && review.shoeId === referenceShoeId,
    );
  const sources = [...new Set([...(targetReview?.sources ?? []), ...(referenceReview?.sources ?? [])])];

  return (
    <article className="recommendation-evidence">
      <div className="recommendation-evidence-heading">
        <div>
          <h4>{reviewer?.name ?? "ไม่ทราบชื่อผู้รีวิว"}</h4>
          <p>{bridgeSourceDescription(bridge.source)}</p>
        </div>
        <span className="status-badge" data-testid="recommendation-evidence-kind">
          <span className="status-dot" aria-hidden="true" />
          {bridgeEvidenceKind(bridge.source)} · {bridgeSourceLabel(bridge.source)}
        </span>
      </div>
      <div className="recommendation-evidence-flow">
        <div>
          <span className="recommendation-evidence-label">REFERENCE</span>
          <strong>{referenceShoe?.model ?? "รองเท้าอ้างอิง"}</strong>
          <span>{endpointLabel(bridge.referenceTriedSize, referenceReview?.recommendation)}</span>
        </div>
        <span className="recommendation-evidence-arrow" aria-hidden="true">
          {stepLabel(bridge.stepDelta)} →
        </span>
        <div>
          <span className="recommendation-evidence-label">TARGET</span>
          <strong>{targetShoe?.model ?? "รองเท้าเป้าหมาย"}</strong>
          <span>{endpointLabel(bridge.targetTriedSize, targetReview?.recommendation)}</span>
        </div>
      </div>
      {bridge.source === "relative-fit" && referenceReview?.recommendation && targetReview?.recommendation ? (
        <p className="recommendation-evidence-method">
          เราอ่านคำแนะนำของ {referenceShoe?.model} ว่า {stepLabel(referenceReview.recommendation.stepDelta)}
          และ {targetShoe?.model} ว่า {stepLabel(targetReview.recommendation.stepDelta)}
        </p>
      ) : null}
      {targetReview?.summary ? <p className="recommendation-evidence-summary">{targetReview.summary}</p> : null}
      {sources.length > 0 ? (
        <div className="recommendation-sources">
          {sources.map((source, index) => (
            <a href={source} key={source} rel="noreferrer" target="_blank">
              เปิดแหล่งข้อมูล{sources.length > 1 ? ` ${index + 1}` : ""} <span aria-hidden="true">↗</span>
            </a>
          ))}
        </div>
      ) : null}
    </article>
  );
}

function Consensus({ comparison }: { comparison: PersonalizedComparison }) {
  const total = comparison.aggregate.reviewerCount;
  if (total === 0) return null;

  return (
    <section className="recommendation-consensus" aria-labelledby="recommendation-consensus-title">
      <div className="recommendation-consensus-heading">
        <div>
          <p className="recommendation-result-label">CONSENSUS</p>
          <h4 id="recommendation-consensus-title">เสียงจาก reviewer ที่ใช้เทียบ</h4>
        </div>
        <span>{total} คน</span>
      </div>
      <ul>
        {comparison.aggregate.groups.map((group) => (
          <li key={group.stepDelta}>
            <div className="recommendation-consensus-row">
              <span>{stepLabel(group.stepDelta)}</span>
              <strong>{group.count} คน</strong>
            </div>
            <span className="recommendation-consensus-bar" aria-hidden="true">
              <span style={{ width: `${(group.count / total) * 100}%` }} />
            </span>
          </li>
        ))}
      </ul>
      {comparison.aggregate.status === "split" ? (
        <p className="recommendation-consensus-note">เสียงยังไม่ไปทางเดียวกัน จึงแสดงทุกทางเลือกไว้ให้คุณตัดสินใจ</p>
      ) : null}
    </section>
  );
}

function defaultReferenceId(catalog: Catalog, targetShoeId: string) {
  const preferredId = "asics-novablast-6";
  if (targetShoeId === "new-balance-fuelcell-supercomp-elite-v6" && catalog.shoes.some(({ id }) => id === preferredId)) {
    return preferredId;
  }
  return catalog.shoes.find((shoe) => shoe.id !== targetShoeId)?.id ?? "";
}

export function RecommendationPanel({ catalog, targetShoeId }: RecommendationPanelProps) {
  const referenceOptions = useMemo(
    () => catalog.shoes.filter((shoe) => shoe.id !== targetShoeId),
    [catalog.shoes, targetShoeId],
  );
  const [referenceShoeId, setReferenceShoeId] = useState(() => defaultReferenceId(catalog, targetShoeId));
  const [referenceSizeSystem, setReferenceSizeSystem] = useState<SizeSystem>("US_M");
  const [referenceSize, setReferenceSize] = useState("10");
  const [fitFeeling, setFitFeeling] = useState<FitFeeling>("good");
  const [comparison, setComparison] = useState<PersonalizedComparison | null>(null);
  const [hasCompared, setHasCompared] = useState(false);
  const targetShoe = catalog.shoes.find((shoe) => shoe.id === targetShoeId);
  const referenceShoe = catalog.shoes.find((shoe) => shoe.id === referenceShoeId);

  function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!referenceShoeId || !referenceSize.trim()) return;
    setComparison(
      buildPersonalizedComparison(catalog, referenceShoeId, targetShoeId, {
        system: referenceSizeSystem,
        value: referenceSize.trim(),
      }),
    );
    setHasCompared(true);
  }

  return (
    <section className="recommendation-panel" aria-labelledby="recommendation-title">
      <div className="shoe-section-heading recommendation-heading">
        <div>
          <p className="shoe-eyebrow">START HERE</p>
          <h2 id="recommendation-title">กำลังหาว่าคู่นี้ควรใส่ไซซ์อะไร?</h2>
          <p>
            เลือกคู่ที่คุณใส่พอดี ระบุไซซ์และความรู้สึกที่มี แล้วอ่านผลเทียบจากข้อมูล reviewer ที่มีจริง
          </p>
        </div>
      </div>

      {referenceOptions.length === 0 ? (
        <div className="empty-state recommendation-empty">
          <span className="empty-state-mark" aria-hidden="true">?</span>
          <h3>ยังไม่มีรองเท้าคู่อื่นให้เทียบ</h3>
          <p>ลองกลับมาใหม่เมื่อมีข้อมูลรองเท้าเพิ่ม</p>
        </div>
      ) : (
        <form className="recommendation-form" onSubmit={handleSubmit}>
          <div className="recommendation-form-heading">
            <span>REFERENCE SHOE</span>
            <p>คู่ที่คุณรู้ไซซ์ของตัวเองอยู่แล้ว</p>
          </div>
          <div className="recommendation-form-grid">
            <ShoeCombobox
              id="reference-shoe"
              label="รองเท้าที่คุณใส่อยู่"
              onChange={setReferenceShoeId}
              options={referenceOptions}
              value={referenceShoeId}
            />
            <label className="shoe-field" htmlFor="reference-size-system">
              <span>ระบบไซซ์</span>
              <select
                id="reference-size-system"
                onChange={(event) => setReferenceSizeSystem(event.target.value as SizeSystem)}
                value={referenceSizeSystem}
              >
                {SIZE_SYSTEMS.map((system) => (
                  <option key={system} value={system}>{SIZE_SYSTEM_LABELS[system]}</option>
                ))}
              </select>
            </label>
            <label className="shoe-field" htmlFor="reference-size">
              <span>ไซซ์ที่ใส่อยู่</span>
              <input
                id="reference-size"
                inputMode="decimal"
                onChange={(event) => setReferenceSize(event.target.value)}
                placeholder="เช่น 10"
                required
                value={referenceSize}
              />
            </label>
          </div>
          <fieldset className="fit-feeling-fieldset">
            <legend>คู่ที่ใส่อยู่รู้สึกอย่างไร?</legend>
            <div className="fit-feeling-options">
              {(Object.keys(FIT_FEELING_LABELS) as FitFeeling[]).map((feeling) => (
                <label className={fitFeeling === feeling ? "is-selected" : ""} key={feeling}>
                  <input
                    checked={fitFeeling === feeling}
                    name="fit-feeling"
                    onChange={() => setFitFeeling(feeling)}
                    type="radio"
                    value={feeling}
                  />
                  <span>{FIT_FEELING_LABELS[feeling]}</span>
                </label>
              ))}
            </div>
            <p>เก็บไว้เป็นบริบทให้คุณอ่านผลเทียบ; ยังไม่ถูกนำไปเปลี่ยน bridge ที่มาจาก reviewer</p>
          </fieldset>
          <button type="submit" disabled={!referenceShoeId || !referenceSize.trim()}>
            เทียบไซซ์
          </button>
        </form>
      )}

      {hasCompared && comparison ? (
        <div className="recommendation-result" aria-live="polite" data-testid="shoe-size-result">
          <div className="recommendation-result-header">
            <div>
              <p className="recommendation-result-label">YOUR STARTING POINT</p>
              <h3>ผลเทียบสำหรับ {targetShoe?.model ?? "รองเท้ารุ่นนี้"}</h3>
            </div>
            <span className="recommendation-fit-context">คู่เดิมรู้สึกว่า{FIT_FEELING_LABELS[fitFeeling]}</span>
          </div>

          {comparison.bridges.length === 0 ? (
            <div className="recommendation-no-result" data-testid="shoe-size-evidence">
              <h4>ยังไม่มีข้อมูลตรงพอให้เทียบคู่นี้</h4>
              <p>
                ไม่มี reviewer bridge ระหว่าง {referenceShoe?.model ?? "รองเท้าอ้างอิง"} และ {targetShoe?.model ?? "รองเท้าเป้าหมาย"}
                ที่อธิบายได้ เราจะไม่เดาไซซ์ให้คุณ ลองเลือก reference รุ่นอื่นได้เลย
              </p>
            </div>
          ) : (
            <>
              <div className="recommendation-primary-result">
                <p>ถ้าจะเริ่มลอง ให้เริ่มจาก</p>
                <strong>
                  {comparison.exactTargetSizes.length > 0
                    ? `${SIZE_SYSTEM_LABELS[comparison.referenceSize.system]} ${comparison.exactTargetSizes.join(" หรือ ")}`
                    : comparison.aggregate.status === "split"
                      ? "ยังไม่มีคำตอบเดียว"
                      : comparison.aggregate.agreedStepDelta === undefined
                        ? "ยังแปลเป็นเลขไซซ์ไม่ได้"
                        : stepLabel(comparison.aggregate.agreedStepDelta)}
                </strong>
                <span>
                  จาก {referenceShoe?.model ?? "รองเท้าอ้างอิง"} {formatSize(comparison.referenceSize)}
                  {comparison.exactTargetSizes.length > 0
                    ? " โดยเรียงตามตารางไซซ์ของรุ่นเป้าหมาย"
                    : " · อ่านรายละเอียดของ reviewer ด้านล่างก่อนตัดสินใจ"}
                </span>
              </div>

              {comparison.aggregate.status === "split" ? (
                <div className="recommendation-split" role="note">
                  <h4>ความเห็นยังแยกเป็นหลายทาง</h4>
                  <p>ไม่มีเสียงข้างมากที่ชัดพอให้เลือกแทนคุณ ลองดูจำนวนคนในแต่ละทางด้านล่าง</p>
                </div>
              ) : null}

              <Consensus comparison={comparison} />

              <div className="recommendation-evidence-list">
                <div className="recommendation-evidence-list-heading">
                  <div>
                    <p className="recommendation-result-label">SOURCE NOTES</p>
                    <h4>หลักฐานที่ใช้เทียบ</h4>
                  </div>
                  <p>
                    {comparison.aggregate.reviewerCount} reviewer · {comparison.bridges.filter(({ source }) => source !== "relative-fit").length} direct bridge · {comparison.bridges.filter(({ source }) => source === "relative-fit").length} indirect evidence
                  </p>
                </div>
                {comparison.bridges.map((bridge) => (
                  <BridgeEvidence
                    bridge={bridge}
                    catalog={catalog}
                    key={bridge.reviewerId}
                    referenceShoeId={referenceShoeId}
                    targetShoeId={targetShoeId}
                  />
                ))}
              </div>
            </>
          )}
        </div>
      ) : null}
    </section>
  );
}
