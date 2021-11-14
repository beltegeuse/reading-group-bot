-- Your SQL goes here
CREATE TABLE papers (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    title TEXT NOT NULL, -- Paper title
    url TEXT NOT NULL, -- URL to found the paper
    venue TEXT, -- Venue where the paper get published
    user_id integer not null, -- User who proposed the paper
    vote_count integer not null, -- The number of vote count (rapid to get processed)
    readed integer not null, -- If the paper got readed or not

    foreign key (user_id) references logins(id)
);