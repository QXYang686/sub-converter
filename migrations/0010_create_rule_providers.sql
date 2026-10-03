CREATE TABLE source_rule_providers (
  source_id TEXT NOT NULL,
  ordinal INTEGER NOT NULL,
  name TEXT NOT NULL,
  provider_type TEXT,
  behavior TEXT,
  url TEXT,
  path TEXT,
  interval INTEGER,
  options TEXT NOT NULL,
  PRIMARY KEY (source_id, ordinal),
  FOREIGN KEY (source_id) REFERENCES sources(id) ON DELETE CASCADE
);

CREATE INDEX idx_source_rule_providers_name ON source_rule_providers(source_id, name);

CREATE TABLE rule_provider_snapshots (
  source_id TEXT NOT NULL,
  name TEXT NOT NULL,
  body BLOB,
  etag TEXT,
  last_modified TEXT,
  fetched_at INTEGER,
  body_hash TEXT,
  rule_count INTEGER,
  last_error TEXT,
  PRIMARY KEY (source_id, name),
  FOREIGN KEY (source_id) REFERENCES sources(id) ON DELETE CASCADE
);
