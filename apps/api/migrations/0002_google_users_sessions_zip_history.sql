DROP TABLE IF EXISTS extractions;

CREATE TABLE users (
  id TEXT PRIMARY KEY,
  google_subject TEXT NOT NULL UNIQUE,
  email TEXT NOT NULL,
  display_name TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE sessions (
  token_hash TEXT PRIMARY KEY,
  user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  created_at TEXT NOT NULL,
  expires_at TEXT NOT NULL
);
CREATE INDEX sessions_user_expiry_idx ON sessions(user_id, expires_at);

CREATE TABLE oauth_states (
  state_hash TEXT PRIMARY KEY,
  nonce TEXT NOT NULL,
  pkce_verifier TEXT NOT NULL,
  created_at TEXT NOT NULL,
  expires_at TEXT NOT NULL
);
CREATE INDEX oauth_states_expiry_idx ON oauth_states(expires_at);

CREATE TABLE extractions (
  id TEXT PRIMARY KEY,
  user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  file_name TEXT NOT NULL,
  file_size_bytes INTEGER NOT NULL CHECK (file_size_bytes >= 0),
  status TEXT NOT NULL CHECK (status IN ('succeeded', 'failed')),
  activity_type TEXT,
  activity_date TEXT,
  normalized_json TEXT,
  raw_json TEXT,
  error_code TEXT,
  error_message TEXT,
  created_at TEXT NOT NULL,
  CHECK ((status = 'succeeded' AND normalized_json IS NOT NULL AND raw_json IS NOT NULL AND error_code IS NULL AND error_message IS NULL) OR (status = 'failed' AND normalized_json IS NULL AND raw_json IS NULL AND error_code IS NOT NULL AND error_message IS NOT NULL))
);
CREATE INDEX extractions_user_activity_idx ON extractions(user_id, activity_date, created_at, id);
