CREATE TABLE users (
    id TEXT PRIMARY KEY,
    google_subject TEXT NOT NULL UNIQUE,
    email TEXT NOT NULL,
    display_name TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

ALTER TABLE extractions
    ADD COLUMN user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE;

CREATE TABLE sessions (
    token_hash TEXT PRIMARY KEY,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_at TEXT NOT NULL,
    expires_at TEXT NOT NULL
);

CREATE INDEX sessions_user_expiry_idx
    ON sessions (user_id, expires_at);

CREATE TABLE oauth_states (
    state_hash TEXT PRIMARY KEY,
    nonce TEXT NOT NULL,
    pkce_verifier TEXT NOT NULL,
    created_at TEXT NOT NULL,
    expires_at TEXT NOT NULL
);

CREATE INDEX oauth_states_expiry_idx
    ON oauth_states (expires_at);

CREATE INDEX extractions_user_activity_idx
    ON extractions (user_id, activity_date, created_at, id);
