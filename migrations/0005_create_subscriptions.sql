CREATE TABLE sources (
  id TEXT PRIMARY KEY,
  user_id TEXT NOT NULL,
  name TEXT NOT NULL,
  url TEXT NOT NULL,
  enabled INTEGER NOT NULL DEFAULT 1,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  FOREIGN KEY (user_id) REFERENCES users(id)
);

CREATE UNIQUE INDEX idx_sources_user_url ON sources(user_id, url);

CREATE INDEX idx_sources_user_id ON sources(user_id);

CREATE TABLE publications (
  id TEXT PRIMARY KEY,
  user_id TEXT NOT NULL,
  name TEXT NOT NULL,
  secret TEXT NOT NULL UNIQUE,
  enabled INTEGER NOT NULL DEFAULT 1,
  expires_at INTEGER,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  FOREIGN KEY (user_id) REFERENCES users(id)
);

CREATE INDEX idx_publications_user_id ON publications(user_id);

CREATE TABLE publication_sources (
  publication_id TEXT NOT NULL,
  source_id TEXT NOT NULL,
  position INTEGER NOT NULL,
  PRIMARY KEY (publication_id, source_id),
  FOREIGN KEY (publication_id) REFERENCES publications(id) ON DELETE CASCADE,
  FOREIGN KEY (source_id) REFERENCES sources(id) ON DELETE CASCADE
);

CREATE INDEX idx_publication_sources_source_id ON publication_sources(source_id);
