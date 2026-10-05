import { createRunExport, readRunExport, runExportUrl, validateRunExport } from "./runs-api";
import type { RunExportMode, RunExportSnapshot, RunPrivacyOmission } from "./runs-types";

export interface RunExportPreview { omissions: RunPrivacyOmission[]; }
export interface RunExportSession {
  prepare(mode: RunExportMode): Promise<RunExportSnapshot>;
  preview(mode: RunExportMode): Promise<RunExportPreview>;
  copy(mode: RunExportMode, writeClipboard: (text: string) => Promise<void>): Promise<void>;
  download(mode: RunExportMode): Promise<string>;
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

/** One selection and privacy policy, with separate pinned snapshots for each mode. */
export function createRunExportSession(activityIds: string[], includeLocation = false, includeDeviceIdentifiers = false): RunExportSession {
  const selection = [...activityIds];
  let active = true;
  const slots: Partial<Record<RunExportMode, {
    preparing?: Promise<RunExportSnapshot>;
    snapshot?: RunExportSnapshot;
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
    await validateRunExport(snapshot);
    check(snapshot);
    return { omissions: snapshot.privacyOmissions };
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
    async download(mode: RunExportMode): Promise<string> {
      const snapshot = await prepare(mode);
      await validateRunExport(snapshot);
      check(snapshot);
      return runExportUrl(snapshot);
    },
  };
}
