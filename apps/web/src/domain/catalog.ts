import { z } from "zod";

/** The size systems that can be shown or translated by the static catalog. */
export const SizeSystemSchema = z.enum(["US_M", "US_W", "UK", "EU", "CM"]);
export type SizeSystem = z.infer<typeof SizeSystemSchema>;

const IdSchema = z.string().trim().min(1, "must not be empty");
const NameSchema = z.string().trim().min(1, "must not be empty");
const UrlSchema = z.string().url("must be a valid URL");
const ImageUrlSchema = z
  .string()
  .trim()
  .min(1)
  .refine(
    (value) =>
      (value.startsWith("/") && !value.startsWith("//")) || UrlSchema.safeParse(value).success,
    "must be a valid URL or site-relative path",
  );

/** A visible size value. Values stay strings so chart labels are not rewritten. */
export const SizeSchema = z
  .object({
    system: SizeSystemSchema,
    value: z
      .string()
      .trim()
      .min(1, "size value must not be empty")
      .refine(
        (value) => Number.isFinite(Number(value)) && Number(value) > 0,
        "size value must be a positive number",
      ),
  })
  .strict();
export type Size = z.infer<typeof SizeSchema>;

export const ShoeSchema = z
  .object({
    id: IdSchema,
    brand: NameSchema,
    model: NameSchema,
    imageUrl: ImageUrlSchema.optional(),
    releaseDate: z
      .object({
        month: z
          .int()
          .min(1, "release month must be between 1 and 12")
          .max(12, "release month must be between 1 and 12"),
        year: z
          .int()
          .min(2000, "release year must be 2000 or later")
          .max(2100, "release year must be 2100 or earlier"),
        sourceUrl: UrlSchema,
      })
      .strict(),
  })
  .strict();
export type Shoe = z.infer<typeof ShoeSchema>;

const THAI_SHORT_MONTHS = [
  "ม.ค.",
  "ก.พ.",
  "มี.ค.",
  "เม.ย.",
  "พ.ค.",
  "มิ.ย.",
  "ก.ค.",
  "ส.ค.",
  "ก.ย.",
  "ต.ค.",
  "พ.ย.",
  "ธ.ค.",
] as const;

export type ShoeSortOption =
  | "latest"
  | "oldest"
  | "name-asc"
  | "name-desc"
  | "year-desc"
  | "year-asc"
  | "month-desc"
  | "month-asc";

function compareShoeNames(left: Shoe, right: Shoe): number {
  const byBrand = left.brand.localeCompare(right.brand, "th");
  if (byBrand !== 0) return byBrand;
  return left.model.localeCompare(right.model, "th");
}

export function formatReleaseMonth(month: Shoe["releaseDate"]["month"]): string {
  return THAI_SHORT_MONTHS[month - 1];
}

export function formatReleaseDate(releaseDate: Shoe["releaseDate"]): string {
  return `${formatReleaseMonth(releaseDate.month)} ${releaseDate.year}`;
}

export function getAvailableReleaseYears(shoes: readonly Shoe[]): number[] {
  return [...new Set(shoes.map((shoe) => shoe.releaseDate.year))].sort(
    (left, right) => right - left,
  );
}

export function filterShoesByQuery(shoes: readonly Shoe[], query: string): Shoe[] {
  const terms = query
    .trim()
    .toLocaleLowerCase("en-US")
    .split(/\s+/)
    .filter(Boolean);

  if (terms.length === 0) return [...shoes];

  return shoes.filter((shoe) => {
    const searchable = `${shoe.brand} ${shoe.model}`.toLocaleLowerCase("en-US");
    return terms.every((term) => searchable.includes(term));
  });
}

export function filterShoesByReleaseDate(
  shoes: readonly Shoe[],
  options: {
    query?: string;
    year?: number | null;
    month?: number | null;
  } = {},
): Shoe[] {
  return filterShoesByQuery(shoes, options.query ?? "").filter((shoe) => {
    if (options.year && shoe.releaseDate.year !== options.year) {
      return false;
    }
    if (options.month && shoe.releaseDate.month !== options.month) {
      return false;
    }
    return true;
  });
}

export function sortShoes(shoes: readonly Shoe[], sortOption: ShoeSortOption): Shoe[] {
  const sorted = [...shoes];

  sorted.sort((left, right) => {
    switch (sortOption) {
      case "latest": {
        const byYear = right.releaseDate.year - left.releaseDate.year;
        if (byYear !== 0) return byYear;
        const byMonth = right.releaseDate.month - left.releaseDate.month;
        if (byMonth !== 0) return byMonth;
        return compareShoeNames(left, right);
      }
      case "oldest": {
        const byYear = left.releaseDate.year - right.releaseDate.year;
        if (byYear !== 0) return byYear;
        const byMonth = left.releaseDate.month - right.releaseDate.month;
        if (byMonth !== 0) return byMonth;
        return compareShoeNames(left, right);
      }
      case "name-asc":
        return compareShoeNames(left, right);
      case "name-desc":
        return compareShoeNames(right, left);
      case "year-desc": {
        const byYear = right.releaseDate.year - left.releaseDate.year;
        if (byYear !== 0) return byYear;
        const byMonth = right.releaseDate.month - left.releaseDate.month;
        if (byMonth !== 0) return byMonth;
        return compareShoeNames(left, right);
      }
      case "year-asc": {
        const byYear = left.releaseDate.year - right.releaseDate.year;
        if (byYear !== 0) return byYear;
        const byMonth = left.releaseDate.month - right.releaseDate.month;
        if (byMonth !== 0) return byMonth;
        return compareShoeNames(left, right);
      }
      case "month-desc": {
        const byMonth = right.releaseDate.month - left.releaseDate.month;
        if (byMonth !== 0) return byMonth;
        const byYear = right.releaseDate.year - left.releaseDate.year;
        if (byYear !== 0) return byYear;
        return compareShoeNames(left, right);
      }
      case "month-asc": {
        const byMonth = left.releaseDate.month - right.releaseDate.month;
        if (byMonth !== 0) return byMonth;
        const byYear = right.releaseDate.year - left.releaseDate.year;
        if (byYear !== 0) return byYear;
        return compareShoeNames(left, right);
      }
    }
  });

  return sorted;
}

const BRAND_COLLATOR = new Intl.Collator("en", {
  numeric: true,
  sensitivity: "base",
});

export type ShoeBrandGroup = {
  brand: string;
  shoes: Shoe[];
};

export function groupShoesByBrand(shoes: readonly Shoe[]): ShoeBrandGroup[] {
  const shoesByBrand = new Map<string, ShoeBrandGroup>();

  for (const shoe of shoes) {
    const brandKey = shoe.brand.toLocaleLowerCase("en-US");
    const group = shoesByBrand.get(brandKey);
    if (group) {
      group.shoes.push(shoe);
    } else {
      shoesByBrand.set(brandKey, { brand: shoe.brand, shoes: [shoe] });
    }
  }

  return [...shoesByBrand.values()]
    .sort((left, right) => BRAND_COLLATOR.compare(left.brand, right.brand))
    .map(({ brand, shoes: groupedShoes }) => ({
      brand,
      shoes: [...groupedShoes].sort((left, right) =>
        BRAND_COLLATOR.compare(left.model, right.model),
      ),
    }));
}

export const ReviewerSchema = z
  .object({
    id: IdSchema,
    name: NameSchema,
  })
  .strict();
export type Reviewer = z.infer<typeof ReviewerSchema>;

export const StepDeltaSchema = z
  .number()
  .int("stepDelta must be an integer")
  .min(-2, "stepDelta must be between -2 and 2")
  .max(2, "stepDelta must be between -2 and 2");
export type StepDelta = z.infer<typeof StepDeltaSchema>;

export const RecommendationSchema = z
  .object({
    stepDelta: StepDeltaSchema,
    basis: z.enum(["explicit_delta", "usual_size"]),
  })
  .strict()
  .superRefine((recommendation, context) => {
    if (recommendation.basis === "usual_size" && recommendation.stepDelta !== 0) {
      context.addIssue({
        code: "custom",
        path: ["stepDelta"],
        message: "usual_size recommendation must use stepDelta 0",
      });
    }
  });
export type Recommendation = z.infer<typeof RecommendationSchema>;

export const ComparisonSchema = z
  .object({
    shoeId: IdSchema,
    stepDelta: StepDeltaSchema,
  })
  .strict();
export type Comparison = z.infer<typeof ComparisonSchema>;

export const ReviewSchema = z
  .object({
    id: IdSchema,
    reviewerId: IdSchema,
    shoeId: IdSchema,
    triedSize: SizeSchema.optional(),
    recommendation: RecommendationSchema.optional(),
    comparisons: z.array(ComparisonSchema).optional(),
    summary: z.string().trim().min(1, "summary must not be empty"),
    sources: z.array(UrlSchema).min(1, "at least one source is required"),
  })
  .strict();
export type Review = z.infer<typeof ReviewSchema>;

/** Preserve official chart labels such as ASICS `4H` instead of rewriting them. */
const ChartSizeValueSchema = z.string().trim().min(1, "chart size value must not be empty");

export const SizeChartRowSchema = z
  .object({
    US_M: ChartSizeValueSchema.optional(),
    US_W: ChartSizeValueSchema.optional(),
    UK: ChartSizeValueSchema.optional(),
    EU: ChartSizeValueSchema.optional(),
    CM: ChartSizeValueSchema.optional(),
  })
  .strict()
  .refine(
    (row) => Object.values(row).some((value) => value !== undefined),
    "a size-chart row must contain at least one size value",
  );
export type SizeChartRow = z.infer<typeof SizeChartRowSchema>;

export const SizeChartSchema = z
  .object({
    id: IdSchema,
    brand: NameSchema,
    audience: z.enum(["men", "women", "unisex"]),
    sourceUrl: UrlSchema,
    rows: z.array(SizeChartRowSchema).min(1, "a size chart needs at least one row"),
  })
  .strict()
  .superRefine((chart, context) => {
    for (const system of SizeSystemSchema.options) {
      const values = chart.rows
        .map((row) => row[system])
        .filter((value): value is string => value !== undefined);
      const seen = new Set<string>();

      for (const [index, value] of chart.rows.entries()) {
        const rowValue = value[system];
        if (rowValue === undefined) {
          continue;
        }
        if (seen.has(rowValue)) {
          context.addIssue({
            code: "custom",
            path: ["rows", index, system],
            message: `duplicate ${system} value ${rowValue} makes the ordered chart ambiguous`,
          });
        }
        seen.add(rowValue);
      }

      // A chart may omit a system for some rows, but it must have at least one
      // value in the system before that system can be used for navigation.
      if (values.length === 0) {
        continue;
      }
    }
  });
export type SizeChart = z.infer<typeof SizeChartSchema>;

export const CatalogSchema = z
  .object({
    shoes: z.array(ShoeSchema),
    reviewers: z.array(ReviewerSchema),
    reviews: z.array(ReviewSchema),
    sizeCharts: z.array(SizeChartSchema),
  })
  .strict()
  .superRefine((catalog, context) => {
    const shoeIds = new Set<string>();
    for (const [index, shoe] of catalog.shoes.entries()) {
      if (shoeIds.has(shoe.id)) {
        context.addIssue({
          code: "custom",
          path: ["shoes", index, "id"],
          message: `duplicate shoe id: ${shoe.id}`,
        });
      }
      shoeIds.add(shoe.id);
    }

    const reviewerIds = new Set<string>();
    for (const [index, reviewer] of catalog.reviewers.entries()) {
      if (reviewerIds.has(reviewer.id)) {
        context.addIssue({
          code: "custom",
          path: ["reviewers", index, "id"],
          message: `duplicate reviewer id: ${reviewer.id}`,
        });
      }
      reviewerIds.add(reviewer.id);
    }

    const chartIds = new Set<string>();
    for (const [index, chart] of catalog.sizeCharts.entries()) {
      if (chartIds.has(chart.id)) {
        context.addIssue({
          code: "custom",
          path: ["sizeCharts", index, "id"],
          message: `duplicate size-chart id: ${chart.id}`,
        });
      }
      chartIds.add(chart.id);
    }

    const reviewIds = new Set<string>();
    const reviewerShoePairs = new Set<string>();
    const directBridgePairs = new Set<string>();

    for (const [index, review] of catalog.reviews.entries()) {
      if (reviewIds.has(review.id)) {
        context.addIssue({
          code: "custom",
          path: ["reviews", index, "id"],
          message: `duplicate review id: ${review.id}`,
        });
      }
      reviewIds.add(review.id);

      if (!reviewerIds.has(review.reviewerId)) {
        context.addIssue({
          code: "custom",
          path: ["reviews", index, "reviewerId"],
          message: `review references unknown reviewer: ${review.reviewerId}`,
        });
      }
      if (!shoeIds.has(review.shoeId)) {
        context.addIssue({
          code: "custom",
          path: ["reviews", index, "shoeId"],
          message: `review references unknown shoe: ${review.shoeId}`,
        });
      }

      const reviewerShoeKey = `${review.reviewerId}\u0000${review.shoeId}`;
      if (reviewerShoePairs.has(reviewerShoeKey)) {
        context.addIssue({
          code: "custom",
          path: ["reviews", index],
          message: `reviewer ${review.reviewerId} may contribute only one review for shoe ${review.shoeId}; combine sources in one review`,
        });
      }
      reviewerShoePairs.add(reviewerShoeKey);

      const comparisonsSeen = new Set<string>();
      for (const [comparisonIndex, comparison] of (review.comparisons ?? []).entries()) {
        if (!shoeIds.has(comparison.shoeId)) {
          context.addIssue({
            code: "custom",
            path: ["reviews", index, "comparisons", comparisonIndex, "shoeId"],
            message: `comparison references unknown shoe: ${comparison.shoeId}`,
          });
        }
        if (comparison.shoeId === review.shoeId) {
          context.addIssue({
            code: "custom",
            path: ["reviews", index, "comparisons", comparisonIndex, "shoeId"],
            message: "a shoe cannot be compared with itself",
          });
        }
        if (comparisonsSeen.has(comparison.shoeId)) {
          context.addIssue({
            code: "custom",
            path: ["reviews", index, "comparisons", comparisonIndex, "shoeId"],
            message: `duplicate comparison for shoe ${comparison.shoeId}`,
          });
        }
        comparisonsSeen.add(comparison.shoeId);

        const shoePair = [review.shoeId, comparison.shoeId].sort().join("\u0000");
        const directKey = `${review.reviewerId}\u0000${shoePair}`;
        if (directBridgePairs.has(directKey)) {
          context.addIssue({
            code: "custom",
            path: ["reviews", index, "comparisons", comparisonIndex],
            message: `reviewer ${review.reviewerId} may contribute only one direct comparison for shoe pair ${shoePair.replaceAll("\u0000", " / ")}`,
          });
        }
        directBridgePairs.add(directKey);
      }
    }
  });
export type Catalog = z.infer<typeof CatalogSchema>;

/** Parse and validate a build-time catalog with relationship-aware errors. */
export function validateCatalog(input: unknown): Catalog {
  const result = CatalogSchema.safeParse(input);
  if (result.success) {
    return result.data;
  }

  const details = result.error.issues
    .map((issue) => {
      const path = issue.path.length > 0 ? issue.path.join(".") : "catalog";
      return `${path}: ${issue.message}`;
    })
    .join("; ");
  throw new Error(`Catalog validation failed: ${details}`);
}
