ALTER TABLE credentials ADD COLUMN credential_id TEXT;
ALTER TABLE credentials ADD COLUMN public_key TEXT;
ALTER TABLE credentials ADD COLUMN sign_count INTEGER;
ALTER TABLE credentials ADD COLUMN transports TEXT;
ALTER TABLE credentials ADD COLUMN label TEXT;

CREATE UNIQUE INDEX idx_credentials_passkey_id
  ON credentials(credential_id) WHERE credential_id IS NOT NULL;

CREATE TABLE webauthn_challenges (
  challenge TEXT PRIMARY KEY,
  user_id TEXT,
  kind TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  expires_at INTEGER NOT NULL
);

CREATE INDEX idx_webauthn_challenges_expires_at ON webauthn_challenges(expires_at);
