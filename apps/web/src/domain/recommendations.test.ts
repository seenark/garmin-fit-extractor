import { describe, expect, test } from "bun:test";

import { catalog as productionCatalog } from "../data/catalog";
import type { Catalog, Review, Size, SizeChart } from "./catalog";
import {
  aggregateBridgeRecommendation,
  aggregateGeneralRecommendation,
  buildPersonalizedComparison,
  findReviewerBridges,
  moveSizeBySteps,
} from "./recommendations";

const chart: SizeChart = {
  id: "puma-men",
  brand: "PUMA",
  audience: "men",
  sourceUrl: "https://puma.example/size-chart",
  rows: ["9", "9.5", "10", "10.5", "11"].map((US_M) => ({ US_M })),
};

const relativeChart: SizeChart = {
  ...chart,
  rows: ["8.5", "9", "9.5", "10", "10.5", "11"].map((US_M) => ({ US_M })),
};

const testReleaseDate = {
  month: 1,
  year: 2026,
  sourceUrl: "https://example.com/release-date",
} as const;

function review(reviewerId: string, stepDelta?: -1 | 0 | 1): Review {
  return {
    id: `${reviewerId}-target`,
    reviewerId,
    shoeId: "target",
    ...(stepDelta === undefined
      ? {}
      : { recommendation: { stepDelta, basis: "explicit_delta" as const } }),
    summary: "Sizing summary",
    sources: ["https://youtu.be/source"],
  };
}

function bridgeCatalog(
  targetComparisons: Array<{ reviewerId: string; stepDelta: -1 | 0 | 1 }>,
  sizeCharts: SizeChart[] = [chart],
): Catalog {
  const reviewerIds = [...new Set(targetComparisons.map(({ reviewerId }) => reviewerId))];
  return {
    shoes: [
      { id: "reference", brand: "PUMA", model: "Reference", releaseDate: testReleaseDate },
      { id: "target", brand: "PUMA", model: "Target", releaseDate: testReleaseDate },
    ],
    reviewers: reviewerIds.map((id) => ({ id, name: id })),
    reviews: targetComparisons.map(({ reviewerId, stepDelta }) => ({
      id: `${reviewerId}-target`,
      reviewerId,
      shoeId: "target",
      comparisons: [{ shoeId: "reference", stepDelta }],
      summary: "Direct comparison",
      sources: ["https://youtu.be/source"],
    })),
    sizeCharts,
  };
}

type TestStepDelta = -2 | -1 | 0 | 1 | 2;

function relativeCatalog(
  sourceStepDelta: TestStepDelta | undefined,
  targetStepDelta: TestStepDelta | undefined,
  options: {
    sourceReviewerId?: string;
    targetReviewerId?: string;
    sourceBasis?: "explicit_delta" | "usual_size";
    targetBasis?: "explicit_delta" | "usual_size";
    sourceTriedSize?: Size;
    targetTriedSize?: Size;
    targetComparison?: TestStepDelta;
    sizeCharts?: SizeChart[];
  } = {},
): Catalog {
  const sourceReviewerId = options.sourceReviewerId ?? "reviewer";
  const targetReviewerId = options.targetReviewerId ?? sourceReviewerId;
  const reviewerIds = [...new Set([sourceReviewerId, targetReviewerId])];
  const sourceRecommendation =
    sourceStepDelta === undefined
      ? {}
      : {
          recommendation: {
            stepDelta: sourceStepDelta,
            basis: options.sourceBasis ?? "explicit_delta",
          } as const,
        };
  const targetRecommendation =
    targetStepDelta === undefined
      ? {}
      : {
          recommendation: {
            stepDelta: targetStepDelta,
            basis: options.targetBasis ?? "explicit_delta",
          } as const,
        };

  return {
    shoes: [
      { id: "reference", brand: "PUMA", model: "Reference", releaseDate: testReleaseDate },
      { id: "target", brand: "PUMA", model: "Target", releaseDate: testReleaseDate },
    ],
    reviewers: reviewerIds.map((id) => ({ id, name: id })),
    reviews: [
      {
        id: `${sourceReviewerId}-reference`,
        reviewerId: sourceReviewerId,
        shoeId: "reference",
        ...sourceRecommendation,
        ...(options.sourceTriedSize === undefined ? {} : { triedSize: options.sourceTriedSize }),
        summary: "Reference sizing evidence",
        sources: ["https://youtu.be/reference"],
      },
      {
        id: `${targetReviewerId}-target`,
        reviewerId: targetReviewerId,
        shoeId: "target",
        ...targetRecommendation,
        ...(options.targetTriedSize === undefined ? {} : { triedSize: options.targetTriedSize }),
        ...(options.targetComparison === undefined
          ? {}
          : { comparisons: [{ shoeId: "reference", stepDelta: options.targetComparison }] }),
        summary: "Target sizing evidence",
        sources: ["https://youtu.be/target"],
      },
    ],
    sizeCharts: options.sizeCharts ?? [relativeChart],
  };
}

describe("general reviewer aggregation", () => {
  test("shows the raw 2 same / 1 up split", () => {
    const result = aggregateGeneralRecommendation([review("a", 0), review("b", 0), review("c", 1)]);
    expect(result.usableReviewerCount).toBe(3);
    expect(result.groups).toEqual([
      { stepDelta: 0, count: 2, reviewerIds: ["a", "b"] },
      { stepDelta: 1, count: 1, reviewerIds: ["c"] },
    ]);
    expect(result.status).toBe("split");
    expect(result.majorityStepDelta).toBe(0);
  });

  test("does not treat a missing recommendation as same size", () => {
    const result = aggregateGeneralRecommendation([review("a", 0), review("b")]);
    expect(result.usableReviewerCount).toBe(1);
    expect(result.groups[0]?.count).toBe(1);
  });

  test("multiple source URLs still contribute one vote", () => {
    const item = review("a", 0);
    item.sources.push("https://youtu.be/second", "https://youtu.be/third");
    expect(aggregateGeneralRecommendation([item]).usableReviewerCount).toBe(1);
  });

  test("a tie remains a split with no forced majority", () => {
    const result = aggregateGeneralRecommendation([review("a", 0), review("b", 1)]);
    expect(result.status).toBe("split");
    expect(result.majorityStepDelta).toBeUndefined();
  });
});

describe("personalized reviewer bridges", () => {
  test("production catalog maps PUMA US_M 9 to ASICS US_M 9", () => {
    const result = buildPersonalizedComparison(
      productionCatalog,
      "puma-deviate-pure-nitro",
      "asics-gel-kayano-33",
      { system: "US_M", value: "9" },
    );
    expect(result.aggregate.agreedStepDelta).toBe(0);
    expect(result.exactTargetSizes).toEqual(["9"]);
  });

  test("returns no exact personalized size without a bridge", () => {
    const catalog = bridgeCatalog([]);
    catalog.reviewers.push({ id: "a", name: "A" });
    catalog.reviews.push(review("a", 0));
    const result = buildPersonalizedComparison(catalog, "reference", "target", {
      system: "US_M",
      value: "10",
    });
    expect(result.aggregate.status).toBe("none");
    expect(result.exactTargetSizes).toEqual([]);
  });

  test("one reliable bridge produces a tentative exact result", () => {
    const result = buildPersonalizedComparison(
      bridgeCatalog([{ reviewerId: "a", stepDelta: 1 }]),
      "reference",
      "target",
      {
        system: "US_M",
        value: "10",
      },
    );
    expect(result.aggregate.status).toBe("single");
    expect(result.exactTargetSizes).toEqual(["10.5"]);
  });

  test("multiple agreeing bridges preserve reviewer count", () => {
    const result = buildPersonalizedComparison(
      bridgeCatalog([
        { reviewerId: "a", stepDelta: 1 },
        { reviewerId: "b", stepDelta: 1 },
      ]),
      "reference",
      "target",
      { system: "US_M", value: "10" },
    );
    expect(result.aggregate.status).toBe("agreement");
    expect(result.aggregate.reviewerCount).toBe(2);
    expect(result.exactTargetSizes).toEqual(["10.5"]);
  });

  test("conflicting bridges show alternatives without a winner", () => {
    const result = buildPersonalizedComparison(
      bridgeCatalog([
        { reviewerId: "a", stepDelta: 0 },
        { reviewerId: "b", stepDelta: 1 },
      ]),
      "reference",
      "target",
      { system: "US_M", value: "10" },
    );
    expect(result.aggregate.status).toBe("split");
    expect(result.aggregate.groups.map(({ stepDelta }) => stepDelta)).toEqual([0, 1]);
    expect(result.exactTargetSizes).toEqual([]);
  });

  test("a direct comparison wins over confirmed sizes for the same reviewer", () => {
    const catalog = bridgeCatalog([{ reviewerId: "a", stepDelta: 1 }]);
    catalog.reviews.push({
      id: "a-reference",
      reviewerId: "a",
      shoeId: "reference",
      triedSize: { system: "US_M", value: "10" },
      summary: "Reference fit size",
      sources: ["https://youtu.be/reference"],
    });
    catalog.reviews[0]!.triedSize = { system: "US_M", value: "10" };
    expect(findReviewerBridges(catalog, "reference", "target")).toEqual([
      expect.objectContaining({ reviewerId: "a", source: "direct", stepDelta: 1 }),
    ]);
  });

  test("unsupported conversion preserves the step relation without an exact size", () => {
    const result = buildPersonalizedComparison(
      bridgeCatalog([{ reviewerId: "a", stepDelta: 1 }], []),
      "reference",
      "target",
      { system: "US_M", value: "10" },
    );
    expect(result.aggregate.agreedStepDelta).toBe(1);
    expect(result.exactTargetSizes).toEqual([]);
  });

  test("bridge aggregation deduplicates a reviewer and prefers direct data", () => {
    const result = aggregateBridgeRecommendation([
      { reviewerId: "a", stepDelta: 0, source: "relative-fit" },
      { reviewerId: "a", stepDelta: 1, source: "direct" },
    ]);
    expect(result.reviewerCount).toBe(1);
    expect(result.agreedStepDelta).toBe(1);
  });
});

describe("relative-fit reviewer bridges", () => {
  test.each([
    ["true to size to true to size", 0, 0, 0, "9"],
    ["true to size to half size up", 0, 1, 1, "9.5"],
    ["true to size to half size down", 0, -1, -1, "8.5"],
    ["half size up to true to size", 1, 0, -1, "8.5"],
    ["half size down to true to size", -1, 0, 1, "9.5"],
    ["both shoes half size up", 1, 1, 0, "9"],
  ] as const)(
    "%s",
    (_, sourceStepDelta, targetStepDelta, expectedBridgeStepDelta, expectedTargetSize) => {
      const result = buildPersonalizedComparison(
        relativeCatalog(sourceStepDelta, targetStepDelta),
        "reference",
        "target",
        { system: "US_M", value: "9" },
      );

      expect(result.bridges).toEqual([
        expect.objectContaining({
          reviewerId: "reviewer",
          source: "relative-fit",
          stepDelta: expectedBridgeStepDelta,
        }),
      ]);
      expect(result.exactTargetSizes).toEqual([expectedTargetSize]);
    },
  );

  test("accepts usual-size and explicit-delta recommendations as relative evidence", () => {
    const result = buildPersonalizedComparison(
      relativeCatalog(0, 1, { sourceBasis: "usual_size", targetBasis: "explicit_delta" }),
      "reference",
      "target",
      { system: "US_M", value: "9" },
    );

    expect(result.bridges).toEqual([
      expect.objectContaining({ source: "relative-fit", stepDelta: 1 }),
    ]);
  });

  test("does not trust a nonzero delta with the usual-size basis", () => {
    const result = findReviewerBridges(
      relativeCatalog(1, 0, { sourceBasis: "usual_size", targetBasis: "usual_size" }),
      "reference",
      "target",
    );

    expect(result).toEqual([]);
  });

  test("keeps direct evidence ahead of conflicting relative-fit evidence", () => {
    const result = findReviewerBridges(
      relativeCatalog(0, 1, { targetComparison: 0 }),
      "reference",
      "target",
    );

    expect(result).toEqual([expect.objectContaining({ source: "direct", stepDelta: 0 })]);
  });

  test("keeps confirmed numeric sizes ahead of conflicting relative-fit evidence", () => {
    const result = findReviewerBridges(
      relativeCatalog(0, 1, {
        sourceTriedSize: { system: "US_M", value: "9" },
        targetTriedSize: { system: "US_M", value: "10" },
      }),
      "reference",
      "target",
    );

    expect(result).toEqual([
      expect.objectContaining({
        source: "confirmed-size",
        stepDelta: 2,
        referenceTriedSize: { system: "US_M", value: "9" },
        targetTriedSize: { system: "US_M", value: "10" },
      }),
    ]);
  });

  test("does not combine relative evidence from different reviewers", () => {
    const result = buildPersonalizedComparison(
      relativeCatalog(0, 1, { sourceReviewerId: "reviewer-a", targetReviewerId: "reviewer-b" }),
      "reference",
      "target",
      { system: "US_M", value: "9" },
    );

    expect(result.aggregate.status).toBe("none");
    expect(result.exactTargetSizes).toEqual([]);
  });

  test("does not infer a bridge when one side has no usable fit evidence", () => {
    const result = findReviewerBridges(relativeCatalog(0, undefined), "reference", "target");

    expect(result).toEqual([]);
  });

  test("does not turn ambiguous sizing language into an exact offset", () => {
    const catalog = relativeCatalog(undefined, undefined);
    catalog.reviews[0]!.summary = "Runs small; consider going up.";
    catalog.reviews[1]!.summary = "True to size.";

    const result = buildPersonalizedComparison(catalog, "reference", "target", {
      system: "US_M",
      value: "9",
    });

    expect(result.bridges).toEqual([]);
    expect(result.exactTargetSizes).toEqual([]);
  });

  test("supports a relative bridge wider than one full size when chart rows allow it", () => {
    const wideChart: SizeChart = {
      ...chart,
      rows: ["7", "7.5", "8", "8.5", "9", "9.5", "10", "10.5", "11", "11.5", "12"].map((US_M) => ({
        US_M,
      })),
    };
    const result = buildPersonalizedComparison(
      relativeCatalog(-2, 2, { sizeCharts: [wideChart] }),
      "reference",
      "target",
      { system: "US_M", value: "9" },
    );

    expect(result.bridges).toEqual([
      expect.objectContaining({ source: "relative-fit", stepDelta: 4 }),
    ]);
    expect(result.exactTargetSizes).toEqual(["11"]);
  });

  test("retains both source review IDs for relative-fit provenance", () => {
    const result = findReviewerBridges(relativeCatalog(0, 1), "reference", "target");

    expect(result).toEqual([
      expect.objectContaining({
        source: "relative-fit",
        referenceReviewId: "reviewer-reference",
        targetReviewId: "reviewer-target",
      }),
    ]);
  });

  test("does not derive confirmed-size evidence across incompatible systems", () => {
    const result = findReviewerBridges(
      relativeCatalog(undefined, undefined, {
        sourceTriedSize: { system: "US_M", value: "9" },
        targetTriedSize: { system: "EU", value: "42" },
      }),
      "reference",
      "target",
    );

    expect(result).toEqual([]);
  });

  test("keeps a relative bridge when no target chart can produce an exact label", () => {
    const result = buildPersonalizedComparison(
      relativeCatalog(0, 1, { sizeCharts: [] }),
      "reference",
      "target",
      { system: "US_M", value: "9" },
    );

    expect(result.bridges).toEqual([
      expect.objectContaining({ source: "relative-fit", stepDelta: 1 }),
    ]);
    expect(result.exactTargetSizes).toEqual([]);
  });

  test("bridge aggregation prefers confirmed sizes over relative-fit data", () => {
    const result = aggregateBridgeRecommendation([
      { reviewerId: "a", stepDelta: 0, source: "relative-fit" },
      { reviewerId: "a", stepDelta: 1, source: "confirmed-size" },
    ]);

    expect(result.reviewerCount).toBe(1);
    expect(result.agreedStepDelta).toBe(1);
  });
});

describe("production direct sizing evidence", () => {
  test("Por VRR's explicit PUMA to ASICS comparison remains direct", () => {
    const result = findReviewerBridges(
      productionCatalog,
      "puma-deviate-pure-nitro",
      "asics-gel-kayano-33",
    );

    expect(result).toEqual([
      expect.objectContaining({ reviewerId: "por-vrr", source: "direct", stepDelta: 0 }),
    ]);
  });

  test("JAY RUNS connects true-to-size reviews through the user's fitting size", () => {
    const result = buildPersonalizedComparison(
      productionCatalog,
      "hoka-mach-6",
      "puma-deviate-pure-nitro",
      { system: "US_M", value: "9" },
    );

    expect(result.bridges).toEqual([
      expect.objectContaining({ reviewerId: "jay-runs", source: "relative-fit", stepDelta: 0 }),
    ]);
    expect(result.exactTargetSizes).toEqual(["9"]);
  });

  test("Papziza's tried sizes provide a conservative ASICS to New Balance bridge", () => {
    const result = buildPersonalizedComparison(
      productionCatalog,
      "asics-novablast-6",
      "new-balance-fuelcell-supercomp-elite-v6",
      { system: "US_M", value: "10" },
    );

    expect(result.bridges).toEqual([
      expect.objectContaining({ reviewerId: "papziza", source: "confirmed-size", stepDelta: -1 }),
    ]);
    expect(result.exactTargetSizes).toEqual(["9.5"]);
  });
});

describe("ordered size-chart navigation", () => {
  test("moves using row order rather than arithmetic", () => {
    expect(moveSizeBySteps(chart, { system: "US_M", value: "10" }, 1)).toBe("10.5");
    expect(moveSizeBySteps(chart, { system: "US_M", value: "10" }, -1)).toBe("9.5");
  });

  test("preserves official non-decimal labels when moving by a step", () => {
    const officialChart: SizeChart = {
      ...chart,
      rows: [{ US_M: "9" }, { US_M: "9H" }, { US_M: "10" }],
    };
    expect(moveSizeBySteps(officialChart, { system: "US_M", value: "9" }, 1)).toBe("9H");
  });

  test("returns null at chart boundaries", () => {
    expect(moveSizeBySteps(chart, { system: "US_M", value: "9" }, -1)).toBeNull();
    expect(moveSizeBySteps(chart, { system: "US_M", value: "11" }, 1)).toBeNull();
  });

  test("navigates sparse chart rows within the requested size system", () => {
    const sparseChart: SizeChart = {
      ...chart,
      rows: [{ US_M: "9", UK: "8" }, { UK: "8.5" }, { US_M: "10" }],
    };

    expect(moveSizeBySteps(sparseChart, { system: "US_M", value: "9" }, 1)).toBe("10");
    expect(moveSizeBySteps(sparseChart, { system: "US_M", value: "9" }, 2)).toBeNull();
  });

  test("derives confirmed-size deltas from rows filtered to one system", () => {
    const sparseChart: SizeChart = {
      ...chart,
      rows: [{ US_M: "9", UK: "8" }, { UK: "8.5" }, { US_M: "10" }],
    };
    const result = findReviewerBridges(
      relativeCatalog(undefined, undefined, {
        sourceTriedSize: { system: "US_M", value: "9" },
        targetTriedSize: { system: "US_M", value: "10" },
        sizeCharts: [sparseChart],
      }),
      "reference",
      "target",
    );

    expect(result).toEqual([expect.objectContaining({ source: "confirmed-size", stepDelta: 1 })]);
  });
});
