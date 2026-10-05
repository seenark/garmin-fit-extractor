import { expect, test } from "bun:test";
import { renderToStaticMarkup } from "react-dom/server";
import { LatestThresholds, ThresholdStatus } from "./threshold-status";
import type { RunHistoricalThresholds, RunLatestThresholds, RunThresholdTarget, RunTrend } from "../lib/runs-types";

test("missing analysis and failed processing never diagnose insufficient physiological data", () => {
  const html = renderToStaticMarkup(
    <ThresholdStatus
      result={null}
      processing={{ status: "failed", stale: false, updateFailed: true, errorCode: "ANALYSIS_FAILED" }}
    />,
  );
  expect(html).toContain("ยังไม่มีผลวิเคราะห์");
  expect(html).toContain("failed");
  expect(html).toContain("ANALYSIS_FAILED");
  expect(html).not.toContain("insufficient_data");
});

function target(status: RunThresholdTarget["status"], heartRateBpm: number | null): RunThresholdTarget {
  return {
    status,
    engineStatus: "experimental",
    researchBlocked: true,
    method: { id: "running-dfa", version: "1.0.0", configurationHash: "test-hash" },
    targetDefinition: "VT proxy",
    value: heartRateBpm === null ? null : { heartRateBpm },
    uncertainty: { interval: null, reason: "research_not_validated" },
    reasons: ["RESEARCH_NOT_VALIDATED"],
    evidence: { independentActivityCount: 1, startTime: "2026-10-01T08:00:00Z", endTime: "2026-10-01T09:00:00Z", ageDays: 4 },
    trace: { inputRevision: "revision-17", inputHash: "input-17", windows: [], counts: { candidate: 3, accepted: 1, rejected: 2, windows: 6, acceptedWindows: 4, rejectedWindows: 2 } },
    suggestions: [],
    contextUnverified: ["medication"],
    requiredContextUnprovable: true,
  };
}

test("experimental and research blocked stay distinct from each target physiological status", () => {
  const result: RunHistoricalThresholds = {
    evidenceCutoff: "2026-10-05T09:00:00Z",
    computedAt: "2026-10-05T09:01:00Z",
    lt1: target("estimated", 138),
    lt2: target("low_confidence", 171),
  };
  const html = renderToStaticMarkup(<ThresholdStatus result={result} recorded={[{ heartRateBpm: 160, source: "device" }]} />);
  expect(html).toContain('data-threshold-status="estimated"');
  expect(html).toContain('data-threshold-status="low_confidence"');
  expect(html).toMatch(/Research gate \(researchBlocked\)<\/dt><dd><code>true<\/code>/);
  expect(html).toContain("experimental");
  expect(html).toContain("input-17");
  expect(html).toContain('aria-label="threshold ที่อุปกรณ์บันทึก"');
});

test("latest abstention keeps independent old target values and event time order without relabeling old cutoffs", () => {
  const attempt: RunHistoricalThresholds = {
    evidenceCutoff: "2026-10-05T12:00:00Z",
    computedAt: "2026-10-05T12:01:00Z",
    lt1: target("insufficient_data", null),
    lt2: target("insufficient_data", null),
  };
  const latest: RunLatestThresholds = {
    evidenceCutoff: attempt.evidenceCutoff,
    latestAttempt: attempt,
    lastAvailable: {
      lt1: { ...target("low_confidence", 138), activityId: "older-lt1", evidenceCutoff: "2026-10-01T09:00:00Z", computedAt: "2026-10-05T11:00:00Z", stale: true },
      lt2: { ...target("estimated", 171), activityId: "newer-lt2", evidenceCutoff: "2026-10-04T09:00:00Z", computedAt: "2026-10-04T10:00:00Z", stale: true },
    },
    stale: true,
    engineStatus: "experimental",
  };
  const trend: RunTrend = { items: [
    { ...attempt, activityId: "event-newer", evidenceCutoff: "2026-10-04T09:00:00Z", computedAt: "2026-10-04T10:00:00Z" },
    { ...attempt, activityId: "event-older", evidenceCutoff: "2026-10-01T09:00:00Z", computedAt: "2026-10-05T11:00:00Z" },
  ] };
  const html = renderToStaticMarkup(<LatestThresholds latest={latest} trend={trend} />);
  const lt1 = html.slice(html.indexOf('aria-label="lastAvailable LT1"'), html.indexOf('aria-label="lastAvailable LT2"'));
  const lt2 = html.slice(html.indexOf('aria-label="lastAvailable LT2"'), html.indexOf('class="threshold-trend"'));
  expect(lt1).toContain("138");
  expect(lt1).toContain("older-lt1");
  expect(lt1).toContain("2026-10-01T09:00:00Z");
  expect(lt1).not.toContain("2026-10-05T12:00:00Z");
  expect(lt1).not.toContain("newer-lt2");
  expect(lt2).toContain("171");
  expect(lt2).toContain("newer-lt2");
  expect(lt2).toContain("2026-10-04T09:00:00Z");
  expect(html).toContain('data-threshold-status="insufficient_data"');
  const table = html.slice(html.indexOf('class="threshold-trend"'), html.indexOf("</table>"));
  expect(table.indexOf("event-older")).toBeLessThan(table.indexOf("event-newer"));
});
