-- Server-side session registry backing the refresh-token flow.
--
-- The short-lived access token (JWT) stays stateless, but the long-lived
-- refresh token is an opaque random value whose SHA-256 hash lives here. That
-- gives us what a pure JWT cannot: real revocation (logout kills the row) and
-- rotation (every refresh burns the old token and issues a new one, so a stolen
-- token stops working the moment the legitimate user refreshes).
--
-- Only the hash is stored: a database leak does not leak usable tokens.
CREATE TABLE IF NOT EXISTS sessions (
    id BIGSERIAL PRIMARY KEY NOT NULL,
    user_id BIGINT NOT NULL REFERENCES users (id),
    token_hash BYTEA NOT NULL UNIQUE,
    expires_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    -- NULL while the session is alive; set on logout or rotation.
    revoked_at TIMESTAMPTZ
);

-- Listing/revoking a user's sessions filters by user.
CREATE INDEX IF NOT EXISTS idx_sessions_user ON sessions (user_id);
