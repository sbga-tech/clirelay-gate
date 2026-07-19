CREATE TABLE users_new (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  github_id INTEGER NOT NULL UNIQUE,
  github_login TEXT NOT NULL,
  github_name TEXT NOT NULL DEFAULT '',
  github_email TEXT NOT NULL DEFAULT '',
  avatar_url TEXT NOT NULL DEFAULT '',
  api_key_ciphertext BLOB,
  api_key_nonce BLOB,
  clirelay_api_key_id TEXT UNIQUE,
  api_key_version INTEGER NOT NULL DEFAULT 0,
  created_at INTEGER NOT NULL,
  last_login_at INTEGER NOT NULL,
  CHECK (
    (api_key_ciphertext IS NULL AND api_key_nonce IS NULL)
    OR (api_key_ciphertext IS NOT NULL AND api_key_nonce IS NOT NULL)
  ),
  CHECK (clirelay_api_key_id IS NULL OR api_key_ciphertext IS NOT NULL)
);

INSERT INTO users_new (
  id,
  github_id,
  github_login,
  github_name,
  github_email,
  avatar_url,
  api_key_ciphertext,
  api_key_nonce,
  clirelay_api_key_id,
  api_key_version,
  created_at,
  last_login_at
)
SELECT
  id,
  github_id,
  github_login,
  github_name,
  github_email,
  avatar_url,
  api_key_ciphertext,
  api_key_nonce,
  NULL,
  0,
  created_at,
  last_login_at
FROM users;

DROP TABLE users;
ALTER TABLE users_new RENAME TO users;
