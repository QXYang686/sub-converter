CREATE TABLE rule_sets (
  id             TEXT PRIMARY KEY,
  user_id        TEXT NOT NULL,
  name           TEXT NOT NULL,
  source_kind    TEXT NOT NULL,
  url            TEXT,
  path           TEXT,
  category       TEXT,
  content_format TEXT NOT NULL,
  interval       INTEGER,
  enabled        INTEGER NOT NULL DEFAULT 1,
  created_at     INTEGER NOT NULL,
  updated_at     INTEGER NOT NULL,
  FOREIGN KEY (user_id) REFERENCES users(id)
);

CREATE UNIQUE INDEX idx_rule_sets_user_name ON rule_sets(user_id, name);

CREATE INDEX idx_rule_sets_user_id ON rule_sets(user_id);

CREATE TABLE rule_set_contents (
  rule_set_id   TEXT PRIMARY KEY,
  body          BLOB,
  etag          TEXT,
  last_modified TEXT,
  updated_at    INTEGER NOT NULL,
  body_hash     TEXT,
  rule_count    INTEGER,
  last_error    TEXT,
  pinned        INTEGER NOT NULL DEFAULT 0,
  FOREIGN KEY (rule_set_id) REFERENCES rule_sets(id) ON DELETE CASCADE
);
