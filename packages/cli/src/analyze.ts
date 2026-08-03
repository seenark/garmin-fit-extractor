import { basename } from "node:path";
import { decodeFitFile } from "./fit.js";
import { normalizeFitMessages } from "./normalize.js";
import { type Analysis, analysisSchema } from "./schema.js";

export async function analyzeFitFile(filePath: string): Promise<Analysis> {
  const decoded = await decodeFitFile(filePath);
  const normalized = normalizeFitMessages(decoded, basename(filePath));
  return analysisSchema.parse(normalized);
}
