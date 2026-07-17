-- Role-based authorization. Every user is a plain 'user' by default; admins are
-- promoted explicitly. This lets administrative access derive from a normal
-- authenticated session instead of depending solely on a single shared secret.
ALTER TABLE users
    ADD COLUMN role TEXT NOT NULL DEFAULT 'user'
        CHECK (role IN ('user', 'admin'));
