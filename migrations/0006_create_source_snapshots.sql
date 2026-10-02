CREATE TABLE source_snapshots (
  source_id TEXT PRIMARY KEY,
  body BLOB,
  etag TEXT,
  last_modified TEXT,
  fetched_at INTEGER,
  refresh_lease_until INTEGER,
  last_error TEXT,
  FOREIGN KEY (source_id) REFERENCES sources(id) ON DELETE CASCADE
);
