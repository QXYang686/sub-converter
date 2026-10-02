CREATE TABLE credentials (
  id TEXT PRIMARY KEY,
  user_id TEXT NOT NULL,
  kind TEXT NOT NULL,
  password_hash TEXT,
  created_at INTEGER NOT NULL,
  last_used_at INTEGER,
  FOREIGN KEY (user_id) REFERENCES users(id)
);

CREATE UNIQUE INDEX idx_credentials_password_per_user
  ON credentials(user_id) WHERE kind = 'password';

CREATE INDEX idx_credentials_user_id ON credentials(user_id);

INSERT INTO credentials (id, user_id, kind, password_hash, created_at, last_used_at)
SELECT id, id, 'password', password_hash, created_at, NULL FROM users;

ALTER TABLE users DROP COLUMN password_hash;
