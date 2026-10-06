-- Indexed source-unavailable projection. Original legacy payloads remain unchanged.
CREATE TABLE runs_legacy_summaries (
    id TEXT PRIMARY KEY REFERENCES extractions(id) ON DELETE CASCADE,
    owner_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    start_time TEXT,
    end_time TEXT,
    start_order TIMESTAMPTZ,
    summary JSONB NOT NULL,
    processing_status TEXT NOT NULL CHECK (processing_status IN ('ready','failed')),
    error_code TEXT,
    UNIQUE (id, owner_id)
);
CREATE INDEX runs_legacy_owner_start_idx ON runs_legacy_summaries(owner_id,start_order DESC,id DESC);

CREATE TABLE runs_import_reports (
    id TEXT PRIMARY KEY,
    owner_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    payload JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX runs_import_reports_owner_created_idx ON runs_import_reports(owner_id,created_at DESC);
CREATE INDEX runs_import_reports_activity_idx ON runs_import_reports USING gin(payload jsonb_path_ops);
