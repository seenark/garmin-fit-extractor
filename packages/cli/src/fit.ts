import { readFile } from "node:fs/promises";
import { Decoder, Stream } from "@garmin/fitsdk";

export type FitMessage = Record<string, unknown>;
export type FitMessages = Record<string, unknown>;

export async function decodeFitFile(filePath: string): Promise<FitMessages> {
  const buffer = await readFile(filePath);
  const stream = Stream.fromBuffer(buffer);
  const decoder = new Decoder(stream);

  if (!decoder.isFIT()) {
    throw new Error("Input is not a valid FIT file.");
  }

  if (!decoder.checkIntegrity()) {
    throw new Error("FIT file failed its integrity check.");
  }

  const { messages, errors } = decoder.read({
    expandComponents: true,
    expandSubFields: true,
    mergeHeartRates: true,
  });

  if (errors.length > 0) {
    const details = errors.map((error) => error.message).join("; ");
    throw new Error(`FIT decoding failed: ${details}`);
  }

  return messages;
}
