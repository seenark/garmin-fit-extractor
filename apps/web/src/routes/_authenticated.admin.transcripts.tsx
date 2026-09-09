import { createFileRoute } from "@tanstack/react-router";

import { TranscriptEntriesPage } from "../features/admin/TranscriptEntriesPage";

export const Route = createFileRoute("/_authenticated/admin/transcripts")({
  component: TranscriptEntriesPage,
});
