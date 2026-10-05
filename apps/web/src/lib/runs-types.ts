export type RunOrder = "asc" | "desc";
export interface RunMetrics {
  distanceMeters: number | null;
  timerTimeSeconds: number | null;
  elapsedTimeSeconds: number | null;
  movingTimeSeconds: number | null;
  averageSpeedMps: number | null;
  averagePaceSecondsPerKm: number | null;
  averageHeartRateBpm: number | null;
  averagePowerWatts: number | null;
  averageCadenceStepsPerMinute: number | null;
}
export interface RunProcessing { status: "queued" | "processing" | "ready" | "failed"; stale: boolean; updateFailed: boolean; errorCode: string | null; historyStatus?: "pending" | "ready" | "failed"; }
export interface RunSummary { id: string; startTime: string | null; endTime: string | null; summary: RunMetrics; processing: RunProcessing; sourceUnavailable: boolean; revisionId: string | null; possibleDuplicate?: boolean; }
export interface RunPage { items: RunSummary[]; total: number; limit: number; offset: number; }
export interface RunSample { index: number; timestamp: string | null; elapsedSeconds: number | null; speedMps: number | null; paceSecondsPerKm: number | null; heartRateBpm: number | null; powerWatts: number | null; cadenceStepsPerMinute: number | null; altitudeMeters: number | null; distanceMeters: number | null; timerRunning?: boolean | null; sourceReferences: unknown; }
export interface RunLap { index: number; startTime: string | null; endTime: string | null; startElapsedSeconds: number | null; endElapsedSeconds: number | null; summary: RunMetrics; sourceReferences: unknown; }
export interface RunEvent { index: number; timestamp: string | null; elapsedSeconds: number | null; event: string | null; eventType: string | null; sourceReferences: unknown; }
export interface NormalizedRun { schemaVersion: "2.0.0"; session: unknown; startTime: string | null; endTime: string | null; sport: string; subtype: string | null; summary: RunMetrics; samples: RunSample[]; laps: RunLap[]; timerEvents: RunEvent[]; rr: { intervals: unknown[]; alignmentEligible: boolean; reasons: string[] }; sensors: unknown[]; zones: unknown; deviceReportedThresholds: unknown; extensions: unknown; warnings: unknown[]; }
export interface RunSegment { index: number; startElapsedSeconds: number; endElapsedSeconds: number; kind: string; durationSeconds: number; timeBasis: "elapsed"; version: string; features: Record<string, unknown>; eligibility: { lt1: { accepted: boolean; reasons: string[] }; lt2: { accepted: boolean; reasons: string[] } }; }
export interface RunThresholdTarget { status: "estimated" | "low_confidence" | "insufficient_data"; engineStatus: string; method: unknown; targetDefinition: unknown; value: unknown; uncertainty: unknown; reasons: unknown[]; evidence: unknown; trace: unknown; suggestions: unknown[]; [key: string]: unknown; }
export interface RunHistoricalThresholds { evidenceCutoff: string | null; computedAt: string | null; lt1: RunThresholdTarget; lt2: RunThresholdTarget; [key: string]: unknown; }
export interface RunAnalysis { schemaVersion: string; quality: unknown; segments: RunSegment[]; thresholds: unknown; transformations: unknown[]; [key: string]: unknown; }
export interface RunDetail extends RunSummary { normalized: NormalizedRun | null; analysis: RunAnalysis | null; historicalThresholds: RunHistoricalThresholds | null; fidelityWarnings?: string[]; }
export interface RunTargetWithDates extends RunThresholdTarget { activityId: string | null; evidenceCutoff: string; computedAt: string; stale: boolean; }
export interface RunLatestThresholds { evidenceCutoff: string | null; latestAttempt: RunHistoricalThresholds | null; lastAvailable: { lt1: RunTargetWithDates | null; lt2: RunTargetWithDates | null }; stale: boolean; engineStatus: string; }
export interface RunTrend { items: Array<RunHistoricalThresholds & { activityId: string }>; }
export interface RunImportResult { batchId: string; items: Array<{ index: number; name: string; status: "imported" | "duplicate" | "unsupported" | "failed"; reason: string | null; activityId: string | null; warnings: string[] }>; counts: { imported: number; duplicate: number; unsupported: number; failed: number }; }
export type RunExportMode = "coach" | "full";
export interface RunExportSnapshot { token: string; generatedAt: string; expiresAt: string; byteLength: number; downloadUrl: string; }
