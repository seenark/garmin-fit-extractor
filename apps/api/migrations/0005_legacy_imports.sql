CREATE TABLE legacy_imports (
    source_name TEXT PRIMARY KEY,
    source_sha256 TEXT NOT NULL,
    imported_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    table_manifest JSONB NOT NULL
);
