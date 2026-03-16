CREATE TABLE paper_roles (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    paper_id INTEGER NOT NULL REFERENCES papers(id),
    user_id INTEGER NOT NULL REFERENCES logins(id),
    role_name TEXT NOT NULL
);
