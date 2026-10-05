-- Preserve the owner foreign key and legacy extraction projection, but new Runs
-- do not manufacture successful extraction records merely to satisfy this FK.
ALTER TABLE activities DROP CONSTRAINT activities_id_fkey;

CREATE TABLE runs_sources (
    id TEXT PRIMARY KEY,
    owner_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    sha256 BYTEA NOT NULL CHECK (octet_length(sha256)=32),
    size_bytes BIGINT NOT NULL CHECK (size_bytes>0 AND size_bytes<=20971520),
    bytes BYTEA NOT NULL CHECK (octet_length(bytes)=size_bytes),
    integrity TEXT NOT NULL CHECK (integrity='verified'),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(owner_id,sha256), UNIQUE(id,owner_id)
);
CREATE TABLE runs_activities (
    id TEXT PRIMARY KEY,
    owner_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    source_id TEXT NOT NULL,
    session_index INTEGER NOT NULL CHECK(session_index=0),
    start_time TEXT NOT NULL, end_time TEXT NOT NULL,
    start_order TIMESTAMPTZ NOT NULL, end_order TIMESTAMPTZ NOT NULL,
    subtype TEXT,
    summary JSONB NOT NULL,
    observation_group_id TEXT NOT NULL,
    observation_fingerprint BYTEA NOT NULL,
    duplicate_evidence JSONB NOT NULL DEFAULT '[]',
    desired_generation BIGINT NOT NULL DEFAULT 1 CHECK(desired_generation>0),
    history_generation BIGINT NOT NULL DEFAULT 1 CHECK(history_generation>0),
    desired_versions JSONB NOT NULL,
    current_manifest_id TEXT,
    processing_status TEXT NOT NULL DEFAULT 'queued' CHECK(processing_status IN ('queued','processing','ready','failed')),
    error_code TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    FOREIGN KEY(source_id,owner_id) REFERENCES runs_sources(id,owner_id),
    UNIQUE(owner_id,source_id,session_index), UNIQUE(id,owner_id)
);
CREATE INDEX runs_activities_owner_event_idx ON runs_activities(owner_id,start_order DESC,id DESC);
CREATE INDEX runs_activities_owner_end_idx ON runs_activities(owner_id,end_order,id);
CREATE INDEX runs_activities_observation_idx ON runs_activities(owner_id,observation_fingerprint);
CREATE TABLE runs_revisions (
    id TEXT PRIMARY KEY,
    owner_id TEXT NOT NULL,
    activity_id TEXT NOT NULL,
    stage TEXT NOT NULL CHECK(stage IN ('decoded','normalized','analysis')),
    generation BIGINT NOT NULL CHECK(generation>0),
    versions JSONB NOT NULL,
    input_revision_ids JSONB NOT NULL,
    payload JSONB NOT NULL,
    legacy_projection JSONB,
    validation JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    FOREIGN KEY(activity_id,owner_id) REFERENCES runs_activities(id,owner_id) ON DELETE CASCADE,
    UNIQUE(id,activity_id,owner_id), UNIQUE(id,owner_id), UNIQUE(activity_id,stage,generation),
    CHECK((stage='normalized' AND legacy_projection IS NOT NULL) OR (stage<>'normalized' AND legacy_projection IS NULL))
);
CREATE TABLE runs_revision_chunks (
    revision_id TEXT NOT NULL, owner_id TEXT NOT NULL,
    document TEXT NOT NULL CHECK(document IN ('archive','analysisInput')),
    position BIGINT NOT NULL CHECK(position>=0),
    payload BYTEA NOT NULL CHECK(octet_length(payload)>0 AND octet_length(payload)<=65536),
    FOREIGN KEY(revision_id,owner_id) REFERENCES runs_revisions(id,owner_id) ON DELETE CASCADE,
    PRIMARY KEY(revision_id,document,position)
);
CREATE TABLE runs_manifests (
    id TEXT PRIMARY KEY, owner_id TEXT NOT NULL, activity_id TEXT NOT NULL,
    decoded_revision_id TEXT NOT NULL, normalized_revision_id TEXT NOT NULL, analysis_revision_id TEXT NOT NULL,
    generation BIGINT NOT NULL, versions JSONB NOT NULL,
    published_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    FOREIGN KEY(activity_id,owner_id) REFERENCES runs_activities(id,owner_id) ON DELETE CASCADE,
    FOREIGN KEY(decoded_revision_id,activity_id,owner_id) REFERENCES runs_revisions(id,activity_id,owner_id) ON DELETE CASCADE,
    FOREIGN KEY(normalized_revision_id,activity_id,owner_id) REFERENCES runs_revisions(id,activity_id,owner_id) ON DELETE CASCADE,
    FOREIGN KEY(analysis_revision_id,activity_id,owner_id) REFERENCES runs_revisions(id,activity_id,owner_id) ON DELETE CASCADE,
    UNIQUE(id,activity_id,owner_id), UNIQUE(activity_id,generation)
);
ALTER TABLE runs_activities ADD CONSTRAINT runs_current_manifest_fk FOREIGN KEY(current_manifest_id,id,owner_id) REFERENCES runs_manifests(id,activity_id,owner_id) DEFERRABLE INITIALLY DEFERRED;
CREATE TABLE runs_jobs (
    id TEXT PRIMARY KEY, owner_id TEXT NOT NULL, activity_id TEXT NOT NULL,
    stage TEXT NOT NULL CHECK(stage IN ('process','analysis','history')),
    desired_generation BIGINT NOT NULL,
    status TEXT NOT NULL DEFAULT 'queued' CHECK(status IN ('queued','processing','ready','failed','cancelled')),
    attempts INTEGER NOT NULL DEFAULT 0 CHECK(attempts BETWEEN 0 AND 3),
    lease_owner TEXT, lease_until TIMESTAMPTZ, heartbeat_at TIMESTAMPTZ,
    error_code TEXT, next_attempt_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    input_revision_ids JSONB NOT NULL, versions JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    FOREIGN KEY(activity_id,owner_id) REFERENCES runs_activities(id,owner_id) ON DELETE CASCADE
);
CREATE UNIQUE INDEX runs_jobs_active_idx ON runs_jobs(activity_id,stage,desired_generation) WHERE status IN ('queued','processing');
CREATE INDEX runs_jobs_claim_idx ON runs_jobs(status,next_attempt_at,created_at);
CREATE INDEX runs_jobs_owner_activity_stage_idx ON runs_jobs(owner_id,activity_id,stage,desired_generation,status);
CREATE TABLE runs_slots (
    stage TEXT NOT NULL, slot INTEGER NOT NULL, lease_owner TEXT, lease_until TIMESTAMPTZ,
    PRIMARY KEY(stage,slot)
);
INSERT INTO runs_slots(stage,slot) VALUES ('decode',1),('worker',1),('import',1);
CREATE TABLE runs_tombstones (
    activity_id TEXT PRIMARY KEY, owner_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    deleted_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE TABLE runs_estimates (
    id TEXT PRIMARY KEY, owner_id TEXT NOT NULL, activity_id TEXT NOT NULL,
    evidence_cutoff TEXT NOT NULL, computed_at TEXT NOT NULL,
    generation BIGINT NOT NULL, manifest_id TEXT NOT NULL, payload JSONB NOT NULL,
    FOREIGN KEY(activity_id,owner_id) REFERENCES runs_activities(id,owner_id) ON DELETE CASCADE,
    FOREIGN KEY(manifest_id,activity_id,owner_id) REFERENCES runs_manifests(id,activity_id,owner_id) ON DELETE CASCADE,
    UNIQUE(activity_id,generation), UNIQUE(id,owner_id)
);
CREATE INDEX runs_estimates_owner_cutoff_idx ON runs_estimates(owner_id,evidence_cutoff DESC);
CREATE TABLE runs_estimate_dependencies (
    estimate_id TEXT NOT NULL, owner_id TEXT NOT NULL,
    activity_id TEXT NOT NULL, manifest_id TEXT NOT NULL,
    FOREIGN KEY(estimate_id,owner_id) REFERENCES runs_estimates(id,owner_id) ON DELETE CASCADE,
    FOREIGN KEY(activity_id,owner_id) REFERENCES runs_activities(id,owner_id) ON DELETE CASCADE,
    FOREIGN KEY(manifest_id,activity_id,owner_id) REFERENCES runs_manifests(id,activity_id,owner_id) ON DELETE CASCADE,
    PRIMARY KEY(estimate_id,activity_id)
);
CREATE TABLE runs_exports (
    token TEXT PRIMARY KEY, owner_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    activity_ids TEXT[] NOT NULL, manifest_ids TEXT[] NOT NULL,
    byte_length BIGINT NOT NULL CHECK(byte_length>=0),
    mode TEXT NOT NULL CHECK(mode IN ('coach','full')),
    meta JSONB NOT NULL,
    generated_at TEXT NOT NULL, expires_at TIMESTAMPTZ NOT NULL,
    revoked BOOLEAN NOT NULL DEFAULT false,
    CHECK(cardinality(activity_ids)>0 AND cardinality(activity_ids)=cardinality(manifest_ids))
);
CREATE INDEX runs_exports_owner_expiry_idx ON runs_exports(owner_id,expires_at);
CREATE TABLE runs_export_chunks (
    token TEXT NOT NULL REFERENCES runs_exports(token) ON DELETE CASCADE,
    position BIGINT NOT NULL CHECK(position>=0),
    payload BYTEA NOT NULL CHECK(octet_length(payload)>0 AND octet_length(payload)<=65536),
    PRIMARY KEY(token,position)
);

CREATE FUNCTION runs_immutable() RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION 'Runs archived records are immutable' USING ERRCODE='23000';
END $$;
CREATE TRIGGER runs_sources_immutable BEFORE UPDATE ON runs_sources FOR EACH ROW EXECUTE FUNCTION runs_immutable();
CREATE TRIGGER runs_revisions_immutable BEFORE UPDATE ON runs_revisions FOR EACH ROW EXECUTE FUNCTION runs_immutable();
CREATE TRIGGER runs_manifests_immutable BEFORE UPDATE ON runs_manifests FOR EACH ROW EXECUTE FUNCTION runs_immutable();
CREATE TRIGGER runs_revision_chunks_immutable BEFORE UPDATE ON runs_revision_chunks FOR EACH ROW EXECUTE FUNCTION runs_immutable();
CREATE TRIGGER runs_export_chunks_immutable BEFORE UPDATE ON runs_export_chunks FOR EACH ROW EXECUTE FUNCTION runs_immutable();
