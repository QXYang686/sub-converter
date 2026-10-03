CREATE TABLE publication_snapshots (
  publication_id TEXT NOT NULL,
  format TEXT NOT NULL,
  content TEXT NOT NULL,
  userinfo_upload INTEGER,
  userinfo_download INTEGER,
  userinfo_total INTEGER,
  userinfo_expire INTEGER,
  generated_at INTEGER NOT NULL,
  PRIMARY KEY (publication_id, format),
  FOREIGN KEY (publication_id) REFERENCES publications(id) ON DELETE CASCADE
);
