-- Your SQL goes here
create table votes (
	id INTEGER PRIMARY KEY AUTOINCREMENT,
	paper_id integer not null,
	user_id integer not null,

	foreign key (paper_id) references papers(id),
	foreign key (user_id) references logins(id)
)