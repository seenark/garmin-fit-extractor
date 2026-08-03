CREATE TABLE extractions (
  id TEXT PRIMARY KEY,
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
  CHECK (
    (status = 'succeeded' AND normalized_json IS NOT NULL AND raw_json IS NOT NULL
     AND error_code IS NULL AND error_message IS NULL)
    OR
    (status = 'failed' AND normalized_json IS NULL AND raw_json IS NULL
     AND error_code IS NOT NULL AND error_message IS NOT NULL)
  )
);

CREATE INDEX extractions_created_at_idx
  ON extractions(created_at DESC, id DESC);
