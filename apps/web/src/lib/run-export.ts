import { createRunExport, downloadRunExport, readRunExport } from "./runs-api";
import type { RunExportMode, RunExportSnapshot } from "./runs-types";

export interface PrivacyOmission {
  category: "location" | "deviceIdentifiers" | "unclassified";
  pathPattern: string;
  count: number;
  reason: string;
}
export interface RunExportPreview { omissions: PrivacyOmission[]; }
export interface RunExportSession {
  prepare(mode: RunExportMode): Promise<RunExportSnapshot>;
  preview(mode: RunExportMode): Promise<RunExportPreview>;
  copy(mode: RunExportMode, writeClipboard: (text: string) => Promise<void>): Promise<void>;
  download(mode: RunExportMode): Promise<Blob>;
  invalidate(): void;
}
export class ExportChangedError extends Error {
  constructor() { super("Selection or privacy changed. Prepare a new snapshot."); }
}
export class ExportExpiredError extends Error {
  constructor() { super("Snapshot expired. Prepare a new snapshot."); }
}
export class ClipboardExportError extends Error {
  constructor() { super("Clipboard access failed. Download the same snapshot instead."); }
}

function parsePreview(text: string): RunExportPreview {
  const document: unknown = JSON.parse(text);
  if (!document || typeof document !== "object" || !("privacyOmissions" in document) || !Array.isArray(document.privacyOmissions)) {
    throw new Error("Export privacy preview is unavailable.");
  }
  const omissions: PrivacyOmission[] = [];
  for (const value of document.privacyOmissions) {
    if (!value || typeof value !== "object" ||
      !["location", "deviceIdentifiers", "unclassified"].includes(value.category) ||
      typeof value.pathPattern !== "string" || typeof value.reason !== "string" ||
      !Number.isSafeInteger(value.count) || value.count < 0) {
      throw new Error("Export privacy preview is invalid.");
    }
    omissions.push({ category: value.category, pathPattern: value.pathPattern, count: value.count, reason: value.reason });
  }
  return { omissions };
}

/** One selection and privacy policy, with separate pinned snapshots for each mode. */
export function createRunExportSession(activityIds: string[], includeLocation = false, includeDeviceIdentifiers = false): RunExportSession {
  const selection = [...activityIds];
  let active = true;
  const slots: Partial<Record<RunExportMode, {
    preparing?: Promise<RunExportSnapshot>;
    snapshot?: RunExportSnapshot;
    reading?: Promise<RunExportPreview>;
    preview?: RunExportPreview;
  }>> = {};
  function check(snapshot?: RunExportSnapshot) {
    if (!active) throw new ExportChangedError();
    if (snapshot && Date.parse(snapshot.expiresAt) <= Date.now()) throw new ExportExpiredError();
  }
  async function prepare(mode: RunExportMode): Promise<RunExportSnapshot> {
    check();
    const slot = slots[mode] ??= {};
    if (slot.snapshot) { check(slot.snapshot); return slot.snapshot; }
    slot.preparing ??= createRunExport(selection, mode, includeLocation, includeDeviceIdentifiers)
      .then((snapshot) => { check(snapshot); slot.snapshot = snapshot; return snapshot; })
      .finally(() => { slot.preparing = undefined; });
    return slot.preparing;
  }
  async function preview(mode: RunExportMode): Promise<RunExportPreview> {
    const snapshot = await prepare(mode);
    const slot = slots[mode]!;
    if (slot.preview) return slot.preview;
    slot.reading ??= readRunExport(snapshot).then((text) => {
      check(snapshot);
      slot.preview = parsePreview(text);
      return slot.preview;
    }).finally(() => { slot.reading = undefined; });
    return slot.reading;
  }
  return {
    prepare,
    preview,
    invalidate() { active = false; },
    async copy(mode: RunExportMode, writeClipboard: (text: string) => Promise<void>): Promise<void> {
      const snapshot = await prepare(mode);
      const text = await readRunExport(snapshot);
      check(snapshot);
      try { await writeClipboard(text); } catch { check(snapshot); throw new ClipboardExportError(); }
      check(snapshot);
    },
    async download(mode: RunExportMode): Promise<Blob> {
      const snapshot = await prepare(mode);
      const blob = await downloadRunExport(snapshot);
      check(snapshot);
      return blob;
    },
  };
}
