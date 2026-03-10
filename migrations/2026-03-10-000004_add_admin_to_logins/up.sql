ALTER TABLE logins ADD COLUMN is_admin INTEGER NOT NULL DEFAULT 0;

UPDATE logins
SET is_admin = 1
WHERE id = (
    SELECT id
    FROM logins
    ORDER BY id ASC
    LIMIT 1
);