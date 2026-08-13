ALTER TABLE oauth_states ADD COLUMN continue_path TEXT;

CREATE TABLE activities (
  id TEXT PRIMARY KEY REFERENCES extractions(id) ON DELETE CASCADE,
  owner_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  sport TEXT,
  started_at TEXT NOT NULL,
  activity_data TEXT NOT NULL,
  created_at TEXT NOT NULL
);
CREATE INDEX activities_owner_started_idx
  ON activities(owner_id, started_at DESC, id DESC);

CREATE TABLE oauth_login_requests (
  request_hash TEXT PRIMARY KEY,
  client_id TEXT NOT NULL,
  redirect_uri TEXT NOT NULL,
  state TEXT NOT NULL,
  scope TEXT NOT NULL,
  created_at TEXT NOT NULL,
  expires_at TEXT NOT NULL
);
CREATE INDEX oauth_login_requests_expiry_idx
  ON oauth_login_requests(expires_at);

CREATE TABLE oauth_authorization_codes (
  code_hash TEXT PRIMARY KEY,
  client_id TEXT NOT NULL,
  redirect_uri TEXT NOT NULL,
  user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  scope TEXT NOT NULL,
  created_at TEXT NOT NULL,
  expires_at TEXT NOT NULL
);
CREATE INDEX oauth_authorization_codes_expiry_idx
  ON oauth_authorization_codes(expires_at);

CREATE TABLE oauth_access_tokens (
  token_hash TEXT PRIMARY KEY,
  client_id TEXT NOT NULL,
  user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  scope TEXT NOT NULL,
  created_at TEXT NOT NULL,
  expires_at TEXT NOT NULL
);
CREATE INDEX oauth_access_tokens_user_expiry_idx
  ON oauth_access_tokens(user_id, expires_at);
CREATE INDEX oauth_access_tokens_expiry_idx
  ON oauth_access_tokens(expires_at);

CREATE TABLE oauth_refresh_tokens (
  token_hash TEXT PRIMARY KEY,
  client_id TEXT NOT NULL,
  user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  scope TEXT NOT NULL,
  created_at TEXT NOT NULL,
  expires_at TEXT NOT NULL,
  revoked_at TEXT
);
CREATE INDEX oauth_refresh_tokens_user_expiry_idx
  ON oauth_refresh_tokens(user_id, expires_at);
CREATE INDEX oauth_refresh_tokens_expiry_idx
  ON oauth_refresh_tokens(expires_at);
