export type ExtractionStatus = "succeeded" | "failed";

export interface UserProfile { id: string; email: string; displayName: string | null; }
export interface CurrentUserResponse { user: UserProfile | null; }

export interface Metric { value: number | null; unit: string; }
export interface Calories { value: number | null; unit: "kcal"; }
export interface HeartRateZone { zone: number; minBpm: number | null; maxBpm: number | null; durationSeconds: number | null; }
export interface Analysis {
  schemaVersion: "1.0.0"; source: { fileName: string }; activity: { type: string | null; subType: string | null; date: string | null };
  summary: { duration: Metric; movingTime: Metric; distance: Metric; calories: Calories };
  heartRate: { averageBpm: number | null; maximumBpm: number | null; zones: HeartRateZone[] };
  pace: { average: Metric; moving: Metric; best: Metric }; power: { averageWatts: number | null; maximumWatts: number | null };
  runningDynamics: { cadence: { averageStepsPerMinute: number | null; maximumStepsPerMinute: number | null }; strideLength: Metric; groundContactTime: Metric; verticalOscillation: Metric; verticalRatio: Metric };
  elevation: { ascent: Metric; descent: Metric }; temperature: { averageCelsius: number | null; minimumCelsius: number | null; maximumCelsius: number | null };
  laps: Array<{ index: number; startTime: string | null; distance: Metric; duration: Metric; movingTime: Metric; pace: Metric; heartRate: { averageBpm: number | null; maximumBpm: number | null }; power: { averageWatts: number | null; maximumWatts: number | null }; cadence: { averageStepsPerMinute: number | null; maximumStepsPerMinute: number | null } }>;
}
export interface RawFitField { name: string; value: unknown; units?: string; }
export interface RawFitRecord { kind: string; fields: RawFitField[]; }
export interface ApiErrorDetail { code: string; message: string; fileName?: string; }
export interface ExtractionSummary { id: string; fileName: string; fileSizeBytes: number; status: ExtractionStatus; activityType: string | null; activityDate: string | null; error?: ApiErrorDetail; createdAt: string; }
export interface BatchCreateResponse { items: ExtractionSummary[]; }
export interface ExtractionPage { items: ExtractionSummary[]; total: number; limit: number; offset: number; }
export interface ExtractionDetail extends ExtractionSummary { normalized: Analysis | null; raw: RawFitRecord[] | null; }
