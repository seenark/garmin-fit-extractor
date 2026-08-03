declare module "@garmin/fitsdk" {
  export class Stream {
    static fromBuffer(buffer: Buffer | Uint8Array): Stream;
  }

  export interface DecodeResult {
    messages: Record<string, unknown>;
    errors: Error[];
  }

  export class Decoder {
    constructor(stream: Stream);
    isFIT(): boolean;
    checkIntegrity(): boolean;
    read(options?: {
      expandComponents?: boolean;
      expandSubFields?: boolean;
      mergeHeartRates?: boolean;
    }): DecodeResult;
  }
}
