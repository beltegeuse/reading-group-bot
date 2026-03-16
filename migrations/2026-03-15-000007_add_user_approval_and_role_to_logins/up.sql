ALTER TABLE logins ADD COLUMN is_approved INTEGER NOT NULL DEFAULT 0;
ALTER TABLE logins ADD COLUMN is_disabled INTEGER NOT NULL DEFAULT 0;
ALTER TABLE logins ADD COLUMN role TEXT NOT NULL DEFAULT 'other';

-- Keep existing users working after migration.
UPDATE logins
SET is_approved = 1;

-- Admin users are professors by default.
UPDATE logins
SET role = 'prof'
WHERE is_admin = 1;
