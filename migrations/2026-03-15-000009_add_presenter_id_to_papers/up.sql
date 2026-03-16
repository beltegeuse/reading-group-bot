ALTER TABLE papers ADD COLUMN presenter_id INTEGER REFERENCES logins(id);
