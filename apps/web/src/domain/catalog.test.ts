import { describe, expect, test } from "bun:test";

import {
  filterShoesByQuery,
  filterShoesByReleaseDate,
  formatReleaseDate,
  formatReleaseMonth,
  getAvailableReleaseYears,
  groupShoesByBrand,
  sortShoes,
  validateCatalog,
  type Catalog,
} from "./catalog";

function validCatalog(): Catalog {
  return {
    shoes: [
      {
        id: "target",
        brand: "PUMA",
        model: "Target",
        releaseDate: { month: 6, year: 2026, sourceUrl: "https://example.com/target-release" },
      },
      {
        id: "reference",
        brand: "PUMA",
        model: "Reference",
        releaseDate: { month: 5, year: 2026, sourceUrl: "https://example.com/reference-release" },
      },
    ],
    reviewers: [{ id: "reviewer", name: "Reviewer" }],
    reviews: [
      {
        id: "review",
        reviewerId: "reviewer",
        shoeId: "target",
        recommendation: { stepDelta: 0, basis: "usual_size" },
        summary: "The reviewer used their usual size.",
        sources: ["https://youtu.be/example"],
      },
    ],
    sizeCharts: [
      {
        id: "puma-men",
        brand: "PUMA",
        audience: "men",
        sourceUrl: "https://puma.example/size-chart",
        rows: [{ US_M: "9" }, { US_M: "9.5" }],
      },
    ],
  };
}

function expectInvalid(mutate: (catalog: Record<string, unknown>) => void, message: string) {
  const catalog = structuredClone(validCatalog()) as unknown as Record<string, unknown>;
  mutate(catalog);
  expect(() => validateCatalog(catalog)).toThrow(message);
}

describe("catalog validation", () => {
  test("accepts a site-relative shoe image path", () => {
    const catalog = validCatalog();
    catalog.shoes[0]!.imageUrl = "/images/shoes/target.png";
    expect(validateCatalog(catalog).shoes[0]!.imageUrl).toBe("/images/shoes/target.png");
  });

  test("requires release month and year for every shoe", () =>
    expectInvalid((catalog) => {
      delete (catalog.shoes as Array<Record<string, unknown>>)[0]!.releaseDate;
    }, "releaseDate"));

  test("rejects invalid release months", () =>
    expectInvalid((catalog) => {
      (catalog.shoes as Array<Record<string, unknown>>)[0]!.releaseDate = {
        month: 13,
        year: 2026,
        sourceUrl: "https://example.com/target-release",
      };
    }, "release month must be between 1 and 12"));

  test("formats Thai release month and date labels", () => {
    expect(formatReleaseMonth(6)).toBe("มิ.ย.");
    expect(formatReleaseDate(validCatalog().shoes[0]!.releaseDate)).toBe("มิ.ย. 2026");
  });

  test("lists available release years from newest to oldest without duplicates", () => {
    const years = getAvailableReleaseYears(validCatalog().shoes);
    expect(years).toEqual([2026]);
  });

  test("filters shoes by query, year, and month", () => {
    const catalog = validCatalog();
    catalog.shoes.push({
      id: "older",
      brand: "Brooks",
      model: "Older",
      releaseDate: { month: 2, year: 2025, sourceUrl: "https://example.com/older-release" },
    });

    expect(
      filterShoesByReleaseDate(catalog.shoes, { query: "brooks" }).map((shoe) => shoe.id),
    ).toEqual(["older"]);
    expect(filterShoesByReleaseDate(catalog.shoes, { year: 2026 }).map((shoe) => shoe.id)).toEqual([
      "target",
      "reference",
    ]);
    expect(filterShoesByReleaseDate(catalog.shoes, { month: 5 }).map((shoe) => shoe.id)).toEqual([
      "reference",
    ]);
  });

  test("filters picker queries across brand and model terms", () => {
    const catalog = validCatalog();
    catalog.shoes.push(
      {
        id: "asics-novablast",
        brand: "ASICS",
        model: "Novablast 5",
        releaseDate: { month: 4, year: 2026, sourceUrl: "https://example.com/asics-release" },
      },
      {
        id: "asics-magic-speed",
        brand: "ASICS",
        model: "Magic Speed 4",
        releaseDate: { month: 3, year: 2026, sourceUrl: "https://example.com/magic-release" },
      },
    );

    expect(filterShoesByQuery(catalog.shoes, "asics nova").map((shoe) => shoe.id)).toEqual([
      "asics-novablast",
    ]);
    expect(filterShoesByQuery(catalog.shoes, "  PUMA   target ").map((shoe) => shoe.id)).toEqual([
      "target",
    ]);
    expect(filterShoesByQuery(catalog.shoes, "   ")).toHaveLength(catalog.shoes.length);
  });

  test("groups case-insensitive brand names and sorts each group", () => {
    const catalog = validCatalog();
    catalog.shoes.push(
      {
        id: "other-puma",
        brand: "puma",
        model: "Alpha",
        releaseDate: { month: 1, year: 2026, sourceUrl: "https://example.com/other-puma-release" },
      },
      {
        id: "other-brooks",
        brand: "Brooks",
        model: "Ghost",
        releaseDate: {
          month: 1,
          year: 2026,
          sourceUrl: "https://example.com/other-brooks-release",
        },
      },
      {
        id: "another-puma",
        brand: "puma",
        model: "Velocity",
        releaseDate: {
          month: 1,
          year: 2026,
          sourceUrl: "https://example.com/another-puma-release",
        },
      },
    );

    expect(groupShoesByBrand(catalog.shoes)).toEqual([
      {
        brand: "Brooks",
        shoes: [catalog.shoes[3]],
      },
      {
        brand: "PUMA",
        shoes: [catalog.shoes[2], catalog.shoes[1], catalog.shoes[0], catalog.shoes[4]],
      },
    ]);
  });

  test("sorts shoes by latest release by default, and supports year/month/name sorting", () => {
    const catalog = validCatalog();
    catalog.shoes.push(
      {
        id: "older",
        brand: "Brooks",
        model: "Older",
        releaseDate: { month: 2, year: 2025, sourceUrl: "https://example.com/older-release" },
      },
      {
        id: "same-year-earlier-month",
        brand: "ASICS",
        model: "Earlier Month",
        releaseDate: {
          month: 1,
          year: 2026,
          sourceUrl: "https://example.com/earlier-month-release",
        },
      },
    );

    expect(sortShoes(catalog.shoes, "latest").map((shoe) => shoe.id)).toEqual([
      "target",
      "reference",
      "same-year-earlier-month",
      "older",
    ]);
    expect(sortShoes(catalog.shoes, "year-asc").map((shoe) => shoe.id)).toEqual([
      "older",
      "same-year-earlier-month",
      "reference",
      "target",
    ]);
    expect(sortShoes(catalog.shoes, "month-asc").map((shoe) => shoe.id)).toEqual([
      "same-year-earlier-month",
      "older",
      "reference",
      "target",
    ]);
    expect(sortShoes(catalog.shoes, "name-asc").map((shoe) => shoe.id)).toEqual([
      "same-year-earlier-month",
      "older",
      "reference",
      "target",
    ]);
  });

  test("accepts official chart labels that are not decimal numbers", () => {
    const catalog = validCatalog();
    catalog.sizeCharts[0]!.rows = [{ US_M: "4" }, { US_M: "4H" }, { US_M: "5" }];
    expect(validateCatalog(catalog).sizeCharts[0]!.rows[1]?.US_M).toBe("4H");
  });

  test("rejects protocol-relative shoe image paths", () => {
    const catalog = validCatalog();
    catalog.shoes[0]!.imageUrl = "//cdn.example/target.png";
    expect(() => validateCatalog(catalog)).toThrow("site-relative path");
  });

  test("accepts a review without triedSize", () => {
    expect(validateCatalog(validCatalog()).reviews[0]?.triedSize).toBeUndefined();
  });

  test("rejects duplicate shoe IDs", () =>
    expectInvalid((catalog) => {
      (catalog.shoes as unknown[]).push({
        id: "target",
        brand: "Other",
        model: "Duplicate",
        releaseDate: { month: 1, year: 2026, sourceUrl: "https://example.com/duplicate-release" },
      });
    }, "duplicate shoe id"));

  test("rejects duplicate reviewer IDs", () =>
    expectInvalid((catalog) => {
      (catalog.reviewers as unknown[]).push({ id: "reviewer", name: "Duplicate" });
    }, "duplicate reviewer id"));

  test("rejects duplicate review IDs", () =>
    expectInvalid((catalog) => {
      (catalog.reviews as unknown[]).push({
        id: "review",
        reviewerId: "reviewer",
        shoeId: "reference",
        summary: "Another review",
        sources: ["https://youtu.be/another"],
      });
    }, "duplicate review id"));

  test("rejects missing reviewer references", () =>
    expectInvalid((catalog) => {
      (catalog.reviews as Array<{ reviewerId: string }>)[0]!.reviewerId = "missing";
    }, "unknown reviewer"));

  test("rejects missing shoe references", () =>
    expectInvalid((catalog) => {
      (catalog.reviews as Array<{ shoeId: string }>)[0]!.shoeId = "missing";
    }, "unknown shoe"));

  test("rejects invalid comparison references", () =>
    expectInvalid((catalog) => {
      (catalog.reviews as Array<Record<string, unknown>>)[0]!.comparisons = [
        { shoeId: "missing", stepDelta: 1 },
      ];
    }, "comparison references unknown shoe"));

  test("rejects duplicate reviewer and shoe pairs", () =>
    expectInvalid((catalog) => {
      (catalog.reviews as unknown[]).push({
        id: "second-review",
        reviewerId: "reviewer",
        shoeId: "target",
        summary: "Duplicate opinion",
        sources: ["https://youtu.be/second"],
      });
    }, "may contribute only one review"));

  test("rejects invalid size systems", () =>
    expectInvalid((catalog) => {
      (catalog.reviews as Array<Record<string, unknown>>)[0]!.triedSize = {
        system: "US_UNSPECIFIED",
        value: "10",
      };
    }, "triedSize.system"));

  test("rejects invalid step deltas", () =>
    expectInvalid((catalog) => {
      (catalog.reviews as Array<Record<string, unknown>>)[0]!.recommendation = {
        stepDelta: 3,
        basis: "explicit_delta",
      };
    }, "stepDelta must be between -2 and 2"));

  test("rejects nonzero steps for usual-size recommendations", () =>
    expectInvalid((catalog) => {
      (catalog.reviews as Array<Record<string, unknown>>)[0]!.recommendation = {
        stepDelta: 1,
        basis: "usual_size",
      };
    }, "usual_size recommendation must use stepDelta 0"));

  test("rejects reviews without a source", () =>
    expectInvalid((catalog) => {
      (catalog.reviews as Array<Record<string, unknown>>)[0]!.sources = [];
    }, "at least one source"));

  test("rejects invalid chart shapes", () =>
    expectInvalid((catalog) => {
      (catalog.sizeCharts as Array<Record<string, unknown>>)[0]!.rows = [{}];
    }, "size-chart row"));
});
