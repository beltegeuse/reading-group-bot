-- Your SQL goes here
CREATE TABLE papers (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    title TEXT NOT NULL,
    url TEXT NOT NULL,
    venue TEXT,
    user_id integer not null,

    foreign key (user_id) references logins(id)
);