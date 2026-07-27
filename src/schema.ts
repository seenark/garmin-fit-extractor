import { z } from "zod";

const nullableNumber = z.number().finite().nullable();
const nullableInteger = z.number().int().nullable();

export const metricSchema = z.object({
  value: nullableNumber,
  unit: z.string(),
});

export const heartRateZoneSchema = z.object({
  zone: z.number().int().positive(),
  minBpm: nullableInteger,
  maxBpm: nullableInteger,
  durationSeconds: nullableNumber,
});

export const lapSchema = z.object({
  index: z.number().int().nonnegative(),
  startTime: z.string().datetime().nullable(),
  distance: metricSchema,
  duration: metricSchema,
  movingTime: metricSchema,
  pace: metricSchema,
  heartRate: z.object({
    averageBpm: nullableInteger,
    maximumBpm: nullableInteger,
  }),
  power: z.object({
    averageWatts: nullableInteger,
    maximumWatts: nullableInteger,
  }),
  cadence: z.object({
    averageStepsPerMinute: nullableNumber,
    maximumStepsPerMinute: nullableNumber,
  }),
});

export const analysisSchema = z.object({
  schemaVersion: z.literal("1.0.0"),
  source: z.object({
    fileName: z.string().min(1),
  }),
  activity: z.object({
    type: z.string().nullable(),
    subType: z.string().nullable(),
    date: z.string().datetime().nullable(),
  }),
  summary: z.object({
    duration: metricSchema,
    movingTime: metricSchema,
    distance: metricSchema,
    calories: z.object({
      value: nullableInteger,
      unit: z.literal("kcal"),
    }),
  }),
  heartRate: z.object({
    averageBpm: nullableInteger,
    maximumBpm: nullableInteger,
    zones: z.array(heartRateZoneSchema),
  }),
  pace: z.object({
    average: metricSchema,
    moving: metricSchema,
    best: metricSchema,
  }),
  power: z.object({
    averageWatts: nullableInteger,
    maximumWatts: nullableInteger,
  }),
  runningDynamics: z.object({
    cadence: z.object({
      averageStepsPerMinute: nullableNumber,
      maximumStepsPerMinute: nullableNumber,
    }),
    strideLength: metricSchema,
    groundContactTime: metricSchema,
    verticalOscillation: metricSchema,
    verticalRatio: metricSchema,
  }),
  elevation: z.object({
    ascent: metricSchema,
    descent: metricSchema,
  }),
  temperature: z.object({
    averageCelsius: nullableNumber,
    minimumCelsius: nullableNumber,
    maximumCelsius: nullableNumber,
  }),
  laps: z.array(lapSchema),
});

export type Analysis = z.infer<typeof analysisSchema>;
