ALTER TABLE source_snapshots ADD COLUMN body_hash TEXT;
ALTER TABLE source_snapshots ADD COLUMN proxy_count INTEGER;
ALTER TABLE source_snapshots ADD COLUMN group_count INTEGER;
ALTER TABLE source_snapshots ADD COLUMN rule_count INTEGER;
ALTER TABLE source_snapshots ADD COLUMN protocol_counts TEXT;
ALTER TABLE source_snapshots ADD COLUMN userinfo_upload INTEGER;
ALTER TABLE source_snapshots ADD COLUMN userinfo_download INTEGER;
ALTER TABLE source_snapshots ADD COLUMN userinfo_total INTEGER;
ALTER TABLE source_snapshots ADD COLUMN userinfo_expire INTEGER;
ALTER TABLE source_snapshots ADD COLUMN update_interval INTEGER;
ALTER TABLE source_snapshots ADD COLUMN provider_name TEXT;
ALTER TABLE source_snapshots ADD COLUMN provider_url TEXT;

CREATE TABLE source_proxies (
  source_id TEXT NOT NULL,
  ordinal INTEGER NOT NULL,
  protocol TEXT,
  name TEXT,
  server TEXT,
  port INTEGER,
  options TEXT NOT NULL,
  PRIMARY KEY (source_id, ordinal),
  FOREIGN KEY (source_id) REFERENCES sources(id) ON DELETE CASCADE
);

CREATE INDEX idx_source_proxies_source_protocol ON source_proxies(source_id, protocol);

CREATE TABLE source_proxy_groups (
  source_id TEXT NOT NULL,
  ordinal INTEGER NOT NULL,
  name TEXT,
  group_type TEXT,
  proxies TEXT NOT NULL,
  options TEXT NOT NULL,
  PRIMARY KEY (source_id, ordinal),
  FOREIGN KEY (source_id) REFERENCES sources(id) ON DELETE CASCADE
);

CREATE TABLE source_config (
  source_id TEXT PRIMARY KEY,
  rules TEXT NOT NULL,
  settings TEXT NOT NULL,
  FOREIGN KEY (source_id) REFERENCES sources(id) ON DELETE CASCADE
);
