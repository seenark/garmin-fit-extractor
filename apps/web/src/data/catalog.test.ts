import { describe, expect, test } from "bun:test";

import { catalog } from "./catalog";
import { validateCatalog } from "../domain/catalog";

describe("production shoe catalog", () => {
  test("is validated and has unique URL-safe IDs", () => {
    expect(() => validateCatalog(catalog)).not.toThrow();

    const ids = catalog.shoes.map((shoe) => shoe.id);
    expect(new Set(ids).size).toBe(ids.length);
    expect(ids.every((id) => /^[a-z0-9]+(?:-[a-z0-9]+)*$/.test(id))).toBe(true);
  });

  test("keeps every image reference relative to the shoe asset directory", async () => {
    const references = catalog.shoes
      .map((shoe) => shoe.imageUrl)
      .filter((imageUrl): imageUrl is string => imageUrl !== undefined);

    expect(references.length).toBeGreaterThan(0);
    expect(references.every((imageUrl) => imageUrl.startsWith("/images/shoes/"))).toBe(true);

    const missing = [] as string[];
    for (const imageUrl of references) {
      const imagePath = new URL(`../../public${imageUrl}`, import.meta.url);
      if (!(await Bun.file(imagePath).exists())) {
        missing.push(imageUrl);
      }
    }
    expect(missing).toEqual([]);
  });

  test("preserves ordered size-chart rows and references", () => {
    expect(catalog.sizeCharts.length).toBeGreaterThan(0);

    for (const chart of catalog.sizeCharts) {
      expect(chart.rows.length).toBeGreaterThan(0);
      for (const system of ["US_M", "US_W", "UK", "EU", "CM"] as const) {
        const values = chart.rows
          .map((row) => row[system])
          .filter((value): value is string => value !== undefined);
        expect(new Set(values).size).toBe(values.length);
      }
    }

    const shoeIds = new Set(catalog.shoes.map((shoe) => shoe.id));
    const reviewerIds = new Set(catalog.reviewers.map((reviewer) => reviewer.id));
    for (const review of catalog.reviews) {
      expect(shoeIds.has(review.shoeId)).toBe(true);
      expect(reviewerIds.has(review.reviewerId)).toBe(true);
      for (const comparison of review.comparisons ?? []) {
        expect(shoeIds.has(comparison.shoeId)).toBe(true);
        expect(comparison.shoeId).not.toBe(review.shoeId);
      }
    }
  });
});
