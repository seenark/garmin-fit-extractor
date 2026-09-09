import type { Catalog, Review, Size, SizeChart, SizeSystem, StepDelta } from "./catalog";

export type RecommendationStatus = "empty" | "single" | "agreement" | "split";

export interface RecommendationGroup {
  stepDelta: StepDelta;
  count: number;
  reviewerIds: string[];
}

export interface GeneralRecommendation {
  usableReviewerCount: number;
  groups: RecommendationGroup[];
  status: RecommendationStatus;
  majorityStepDelta?: StepDelta;
}

export type BridgeSource = "direct" | "confirmed-size" | "relative-fit";
export type BridgeStepDelta = -4 | -3 | -2 | -1 | 0 | 1 | 2 | 3 | 4;

export interface BridgeRecommendationGroup {
  stepDelta: BridgeStepDelta;
  count: number;
  reviewerIds: string[];
}

export interface ReviewerBridge {
  reviewerId: string;
  stepDelta: BridgeStepDelta;
  source: BridgeSource;
  referenceReviewId?: string;
  targetReviewId?: string;
  referenceTriedSize?: Size;
  targetTriedSize?: Size;
}

export type BridgeRecommendationStatus = "none" | "single" | "agreement" | "split";

export interface BridgeRecommendation {
  reviewerCount: number;
  groups: BridgeRecommendationGroup[];
  status: BridgeRecommendationStatus;
  agreedStepDelta?: BridgeStepDelta;
}

export interface PersonalizedComparison {
  bridges: ReviewerBridge[];
  aggregate: BridgeRecommendation;
  referenceSize: Size;
  exactTargetSizes: string[];
}

const VALID_STEP_DELTAS = new Set<number>([-2, -1, 0, 1, 2]);
const VALID_BRIDGE_STEP_DELTAS = new Set<number>([-4, -3, -2, -1, 0, 1, 2, 3, 4]);
const BRIDGE_SOURCE_PRIORITY: Record<BridgeSource, number> = {
  "relative-fit": 1,
  "confirmed-size": 2,
  direct: 3,
};
const SIZE_SYSTEMS: readonly SizeSystem[] = ["US_M", "US_W", "UK", "EU", "CM"];

function isStepDelta(value: number): value is StepDelta {
  return Number.isInteger(value) && VALID_STEP_DELTAS.has(value);
}

function isBridgeStepDelta(value: number): value is BridgeStepDelta {
  return Number.isInteger(value) && VALID_BRIDGE_STEP_DELTAS.has(value);
}

function isUsableRelativeRecommendation(
  recommendation: Review["recommendation"],
): recommendation is NonNullable<Review["recommendation"]> {
  return (
    recommendation !== undefined &&
    isStepDelta(recommendation.stepDelta) &&
    (recommendation.basis === "explicit_delta" || recommendation.stepDelta === 0)
  );
}

function compareBrands(left: string, right: string): boolean {
  return left.trim().toLocaleLowerCase() === right.trim().toLocaleLowerCase();
}

function assertDistinctShoes(
  catalog: Catalog,
  referenceShoeId: string,
  targetShoeId: string,
): void {
  if (referenceShoeId === targetShoeId) {
    throw new Error("Reference Shoe and Target Shoe must be different shoes");
  }
  if (!catalog.shoes.some((shoe) => shoe.id === referenceShoeId)) {
    throw new Error(`Unknown Reference Shoe: ${referenceShoeId}`);
  }
  if (!catalog.shoes.some((shoe) => shoe.id === targetShoeId)) {
    throw new Error(`Unknown Target Shoe: ${targetShoeId}`);
  }
}

function usableReviewsByReviewer(reviews: readonly Review[]): Map<string, Review> {
  const result = new Map<string, Review>();
  for (const review of reviews) {
    const stepDelta = review.recommendation?.stepDelta;
    if (stepDelta === undefined || !isStepDelta(stepDelta)) {
      continue;
    }
    // A reviewer is one vote even if callers pass several source rows. Prefer
    // the first usable opinion in catalog order for deterministic output.
    if (!result.has(review.reviewerId)) {
      result.set(review.reviewerId, review);
    }
  }
  return result;
}

function groupStepDeltas<Delta extends number>(
  deltas: Iterable<readonly [string, Delta]>,
): Array<{ stepDelta: Delta; count: number; reviewerIds: string[] }> {
  const groups = new Map<Delta, string[]>();
  for (const [reviewerId, stepDelta] of deltas) {
    const reviewerIds = groups.get(stepDelta);
    if (reviewerIds) {
      reviewerIds.push(reviewerId);
    } else {
      groups.set(stepDelta, [reviewerId]);
    }
  }

  return [...groups.entries()]
    .sort(([left], [right]) => left - right)
    .map(([stepDelta, reviewerIds]) => ({
      stepDelta,
      count: reviewerIds.length,
      reviewerIds,
    }));
}

function majorityStepDelta(groups: readonly RecommendationGroup[]): StepDelta | undefined {
  if (groups.length === 0) {
    return undefined;
  }
  const largestCount = Math.max(...groups.map((group) => group.count));
  const largest = groups.filter((group) => group.count === largestCount);
  return largest.length === 1 ? largest[0].stepDelta : undefined;
}

/** Aggregate explicit reviewer guidance without weighting any reviewer. */
export function aggregateGeneralRecommendation(reviews: readonly Review[]): GeneralRecommendation {
  const usable = usableReviewsByReviewer(reviews);
  const groups = groupStepDeltas(
    [...usable.values()].map(
      (review) => [review.reviewerId, review.recommendation!.stepDelta] as const,
    ),
  );
  const status: RecommendationStatus =
    groups.length === 0
      ? "empty"
      : usable.size === 1
        ? "single"
        : groups.length === 1
          ? "agreement"
          : "split";

  const majority = majorityStepDelta(groups);
  return {
    usableReviewerCount: usable.size,
    groups,
    status,
    ...(majority === undefined ? {} : { majorityStepDelta: majority }),
  };
}

function audienceForSizeSystem(system: SizeSystem): "men" | "women" | null {
  if (system === "US_M") {
    return "men";
  }
  if (system === "US_W") {
    return "women";
  }
  return null;
}

function chartSupportsSystem(chart: SizeChart, system: SizeSystem): boolean {
  return chart.rows.some((row) => row[system] !== undefined);
}

function chartsForShoe(catalog: Catalog, shoeId: string, system: SizeSystem): SizeChart[] {
  const shoe = catalog.shoes.find((candidate) => candidate.id === shoeId);
  if (!shoe) {
    return [];
  }
  const audience = audienceForSizeSystem(system);
  return catalog.sizeCharts.filter(
    (chart) =>
      compareBrands(chart.brand, shoe.brand) &&
      (audience === null || chart.audience === audience || chart.audience === "unisex") &&
      chartSupportsSystem(chart, system),
  );
}

function chartValuesForSystem(chart: SizeChart, system: SizeSystem): string[] {
  const values: string[] = [];
  for (const row of chart.rows) {
    const value = row[system];
    if (value !== undefined) {
      values.push(value);
    }
  }
  return values;
}

function uniqueSizeRowIndex(chart: SizeChart, size: Size): number | null {
  const values = chartValuesForSystem(chart, size.system);
  const matches = values.reduce<number[]>((indexes, value, index) => {
    if (value === size.value) {
      indexes.push(index);
    }
    return indexes;
  }, []);
  return matches.length === 1 ? matches[0] : null;
}

function deriveConfirmedSizeStepDelta(
  catalog: Catalog,
  targetShoeId: string,
  referenceTriedSize: Size,
  targetTriedSize: Size,
): StepDelta | null {
  // Exact same labels in the same system are the only conversion that does
  // not need an official chart.
  if (
    referenceTriedSize.system === targetTriedSize.system &&
    referenceTriedSize.value === targetTriedSize.value
  ) {
    return 0;
  }

  if (referenceTriedSize.system !== targetTriedSize.system) {
    return null;
  }

  const targetCharts = chartsForShoe(catalog, targetShoeId, targetTriedSize.system);
  if (targetCharts.length !== 1) {
    return null;
  }

  const referenceIndex = uniqueSizeRowIndex(targetCharts[0], referenceTriedSize);
  const targetIndex = uniqueSizeRowIndex(targetCharts[0], targetTriedSize);
  if (referenceIndex === null || targetIndex === null) {
    return null;
  }

  const stepDelta = targetIndex - referenceIndex;
  return isStepDelta(stepDelta) ? stepDelta : null;
}

function deriveRelativeStepDelta(
  referenceReview: Review,
  targetReview: Review,
): BridgeStepDelta | null {
  const referenceRecommendation = referenceReview.recommendation;
  const targetRecommendation = targetReview.recommendation;
  if (
    !isUsableRelativeRecommendation(referenceRecommendation) ||
    !isUsableRelativeRecommendation(targetRecommendation)
  ) {
    return null;
  }

  const relativeStepDelta = targetRecommendation.stepDelta - referenceRecommendation.stepDelta;
  return isBridgeStepDelta(relativeStepDelta) ? relativeStepDelta : null;
}

function reviewProvenance(
  referenceReview: Review | undefined,
  targetReview: Review | undefined,
): Pick<ReviewerBridge, "referenceReviewId" | "targetReviewId"> {
  return {
    ...(referenceReview === undefined ? {} : { referenceReviewId: referenceReview.id }),
    ...(targetReview === undefined ? {} : { targetReviewId: targetReview.id }),
  };
}

function preferredBridge(
  existing: ReviewerBridge | undefined,
  candidate: ReviewerBridge,
): ReviewerBridge {
  if (
    existing === undefined ||
    BRIDGE_SOURCE_PRIORITY[candidate.source] > BRIDGE_SOURCE_PRIORITY[existing.source]
  ) {
    return candidate;
  }
  return existing;
}

function setPreferredBridge(bridges: Map<string, ReviewerBridge>, bridge: ReviewerBridge): void {
  bridges.set(bridge.reviewerId, preferredBridge(bridges.get(bridge.reviewerId), bridge));
}

/**
 * Find one direct, confirmed-size, or relative-fit bridge per reviewer.
 * Higher-priority evidence always wins over weaker evidence for that reviewer.
 */
export function findReviewerBridges(
  catalog: Catalog,
  referenceShoeId: string,
  targetShoeId: string,
): ReviewerBridge[] {
  assertDistinctShoes(catalog, referenceShoeId, targetShoeId);

  const reviewsByReviewer = new Map<string, Map<string, Review>>();
  for (const review of catalog.reviews) {
    let byShoe = reviewsByReviewer.get(review.reviewerId);
    if (!byShoe) {
      byShoe = new Map<string, Review>();
      reviewsByReviewer.set(review.reviewerId, byShoe);
    }
    if (!byShoe.has(review.shoeId)) {
      byShoe.set(review.shoeId, review);
    }
  }

  const bridges = new Map<string, ReviewerBridge>();

  for (const [reviewerId, byShoe] of reviewsByReviewer) {
    const referenceReview = byShoe.get(referenceShoeId);
    const targetReview = byShoe.get(targetShoeId);

    // Canonical direct comparisons live on the reviewed (target) shoe and
    // name the reference shoe. A reverse declaration is also direct, with its
    // relation inverted so the returned delta is always target minus reference.
    const targetComparison = targetReview?.comparisons?.find(
      (comparison) => comparison.shoeId === referenceShoeId,
    );
    if (targetComparison && isBridgeStepDelta(targetComparison.stepDelta)) {
      setPreferredBridge(bridges, {
        reviewerId,
        stepDelta: targetComparison.stepDelta,
        source: "direct",
        ...reviewProvenance(referenceReview, targetReview),
        ...(referenceReview?.triedSize === undefined
          ? {}
          : { referenceTriedSize: referenceReview.triedSize }),
        ...(targetReview?.triedSize === undefined
          ? {}
          : { targetTriedSize: targetReview.triedSize }),
      });
      continue;
    }

    const referenceComparison = referenceReview?.comparisons?.find(
      (comparison) => comparison.shoeId === targetShoeId,
    );
    if (referenceComparison && isBridgeStepDelta(referenceComparison.stepDelta)) {
      const inverted = -referenceComparison.stepDelta;
      if (isBridgeStepDelta(inverted)) {
        setPreferredBridge(bridges, {
          reviewerId,
          stepDelta: inverted,
          source: "direct",
          ...reviewProvenance(referenceReview, targetReview),
          ...(referenceReview?.triedSize === undefined
            ? {}
            : { referenceTriedSize: referenceReview.triedSize }),
          ...(targetReview?.triedSize === undefined
            ? {}
            : { targetTriedSize: targetReview.triedSize }),
        });
        continue;
      }
    }

    if (referenceReview?.triedSize && targetReview?.triedSize) {
      const confirmedStepDelta = deriveConfirmedSizeStepDelta(
        catalog,
        targetShoeId,
        referenceReview.triedSize,
        targetReview.triedSize,
      );
      if (confirmedStepDelta !== null && isBridgeStepDelta(confirmedStepDelta)) {
        setPreferredBridge(bridges, {
          reviewerId,
          stepDelta: confirmedStepDelta,
          source: "confirmed-size",
          ...reviewProvenance(referenceReview, targetReview),
          referenceTriedSize: referenceReview.triedSize,
          targetTriedSize: targetReview.triedSize,
        });
        continue;
      }
    }

    const relativeStepDelta =
      referenceReview && targetReview
        ? deriveRelativeStepDelta(referenceReview, targetReview)
        : null;
    if (relativeStepDelta === null) {
      continue;
    }
    setPreferredBridge(bridges, {
      reviewerId,
      stepDelta: relativeStepDelta,
      source: "relative-fit",
      ...reviewProvenance(referenceReview, targetReview),
      ...(referenceReview?.triedSize === undefined
        ? {}
        : { referenceTriedSize: referenceReview.triedSize }),
      ...(targetReview?.triedSize === undefined ? {} : { targetTriedSize: targetReview.triedSize }),
    });
  }

  return [...bridges.values()];
}

/** Aggregate one bridge vote per reviewer and preserve raw disagreement. */
export function aggregateBridgeRecommendation(
  bridges: readonly ReviewerBridge[],
): BridgeRecommendation {
  const usable = new Map<string, ReviewerBridge>();
  for (const bridge of bridges) {
    if (!isBridgeStepDelta(bridge.stepDelta)) {
      continue;
    }
    usable.set(bridge.reviewerId, preferredBridge(usable.get(bridge.reviewerId), bridge));
  }

  const groups = groupStepDeltas(
    [...usable.values()].map((bridge) => [bridge.reviewerId, bridge.stepDelta] as const),
  );
  const status: BridgeRecommendationStatus =
    groups.length === 0
      ? "none"
      : usable.size === 1
        ? "single"
        : groups.length === 1
          ? "agreement"
          : "split";

  return {
    reviewerCount: usable.size,
    groups,
    status,
    ...(groups.length === 1 ? { agreedStepDelta: groups[0].stepDelta } : {}),
  };
}

/** Move through official chart rows, returning null for unsupported movement. */
export function moveSizeBySteps(chart: SizeChart, size: Size, delta: number): string | null {
  if (!isBridgeStepDelta(delta)) {
    return null;
  }
  if (!SIZE_SYSTEMS.includes(size.system)) {
    return null;
  }

  const sourceIndex = uniqueSizeRowIndex(chart, size);
  if (sourceIndex === null) {
    return null;
  }
  const values = chartValuesForSystem(chart, size.system);
  const destinationIndex = sourceIndex + delta;
  if (destinationIndex < 0 || destinationIndex >= values.length) {
    return null;
  }
  return values[destinationIndex] ?? null;
}

function uniqueTargetChart(
  catalog: Catalog,
  targetShoeId: string,
  system: SizeSystem,
): SizeChart | null {
  const charts = chartsForShoe(catalog, targetShoeId, system);
  return charts.length === 1 ? charts[0] : null;
}

/**
 * Resolve an exact target label only when an agreed bridge and one reliable
 * target chart can translate the user's requested reference size.
 */
export function buildPersonalizedComparison(
  catalog: Catalog,
  referenceShoeId: string,
  targetShoeId: string,
  referenceSize: Size,
): PersonalizedComparison {
  assertDistinctShoes(catalog, referenceShoeId, targetShoeId);
  const bridges = findReviewerBridges(catalog, referenceShoeId, targetShoeId);
  const aggregate = aggregateBridgeRecommendation(bridges);
  const exactTargetSizes: string[] = [];

  if (aggregate.agreedStepDelta !== undefined) {
    const chart = uniqueTargetChart(catalog, targetShoeId, referenceSize.system);
    if (chart) {
      const exactTargetSize = moveSizeBySteps(chart, referenceSize, aggregate.agreedStepDelta);
      if (exactTargetSize !== null) {
        exactTargetSizes.push(exactTargetSize);
      }
    }
  }

  return {
    bridges,
    aggregate,
    referenceSize,
    exactTargetSizes,
  };
}
