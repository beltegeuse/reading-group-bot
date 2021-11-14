use std::collections::HashSet;

use crypto::digest::Digest;
use crypto::sha3::Sha3;
use diesel::{prelude::*, result::QueryResult, QueryDsl, Queryable};
use rocket::serde::Serialize;

// For interacting with the database
use crate::schema::logins::dsl::logins as all_logins;
use crate::schema::papers::dsl::papers as all_papers;
use crate::schema::votes::dsl::votes as all_votes;
use crate::schema::*;

use crate::DbConn;
use crate::PaperForm;
use crate::RegisterForm;

////////////// Users
#[derive(Queryable, Insertable, Debug, Serialize)]
#[serde(crate = "rocket::serde")]
pub struct Login {
    pub id: Option<i32>,
    pub name: String,
    pub email: String,
    pub password_hash: String,
}
pub fn hash_password(password: String) -> String {
    let mut hasher = Sha3::sha3_256();
    hasher.input_str(&password);
    hasher.result_str()
}
impl Login {
    pub async fn all(conn: &DbConn) -> QueryResult<Vec<Login>> {
        conn.run(|c| all_logins.order(logins::id.desc()).load::<Login>(c))
            .await
    }

    pub async fn insert(register: RegisterForm, conn: &DbConn) -> QueryResult<usize> {
        conn.run(|c| {
            let p = Login {
                id: None,
                name: register.name,
                email: register.email,
                password_hash: hash_password(register.password),
            };
            diesel::insert_into(logins::table).values(&p).execute(c)
        })
        .await
    }
}

////////////// Papers
#[derive(Queryable, Insertable, Debug, Serialize, Clone)]
#[serde(crate = "rocket::serde")]
pub struct Paper {
    pub id: Option<i32>,
    pub title: String,
    pub url: String,
    pub venue: Option<String>,
    pub user_id: i32,
    pub vote_count: i32,
    pub readed: i32, // 0 = false, 1 = true
}

impl Paper {
    pub async fn get(conn: &DbConn, paper_id: i32) -> QueryResult<Paper> {
        let res = conn
            .run(move |c| all_papers.filter(papers::id.eq(paper_id)).load::<Paper>(c))
            .await;
        res.and_then(|r| Ok(r[0].clone()))
    }

    pub async fn all_unread(conn: &DbConn, user_id: Option<i32>) -> Option<Vec<(Paper, bool)>> {
        // Retrive all the papers
        let papers = conn
            .run(|c| {
                all_papers
                    .order(papers::vote_count.desc())
                    .filter(papers::readed.eq(0))
                    .load::<Paper>(c)
            })
            .await;
        if papers.is_err() {
            return None; // Database problem
        }
        let papers = papers.unwrap();

        // Get all the votes by the user
        match user_id {
            None => Some(papers.into_iter().map(|p| (p, false)).collect()),
            Some(user_id) => {
                let votes = conn
                    .run(move |c| all_votes.filter(votes::user_id.eq(user_id)).load::<Vote>(c))
                    .await;
                if votes.is_err() {
                    Some(papers.into_iter().map(|p| (p, false)).collect())
                } else {
                    let votes: HashSet<i32> =
                        votes.unwrap().into_iter().map(|v| v.paper_id).collect();
                    Some(
                        papers
                            .into_iter()
                            .map(|p| {
                                let id = p.id.clone().unwrap();
                                (p, votes.contains(&id))
                            })
                            .collect(),
                    )
                }
            }
        }
    }

    pub async fn all(conn: &DbConn) -> QueryResult<Vec<Paper>> {
        conn.run(|c| all_papers.order(papers::vote_count.desc()).load::<Paper>(c))
            .await
    }

    pub async fn insert(conn: &DbConn, paper: PaperForm, user_id: i32) -> QueryResult<usize> {
        conn.run(move |c| {
            let p = Paper {
                id: None,
                title: paper.title,
                url: paper.url,
                venue: if paper.venue == "" {
                    None
                } else {
                    Some(paper.venue)
                },
                user_id,
                vote_count: 0,
                readed: 0, // false
            };
            diesel::insert_into(papers::table).values(&p).execute(c)
        })
        .await
    }

    pub async fn remove(conn: &DbConn, paper_id: i32) -> QueryResult<usize> {
        conn.run(move |c| {
            diesel::delete(all_papers)
                .filter(papers::id.eq(paper_id))
                .execute(c)
        })
        .await
    }
}

#[derive(Queryable, Insertable, Debug, Serialize)]
#[serde(crate = "rocket::serde")]
pub struct Vote {
    pub id: Option<i32>,
    pub paper_id: i32,
    pub user_id: i32,
}
impl Vote {
    pub async fn down(conn: &DbConn, user_id: i32, paper_id: i32) -> bool {
        let votes = conn
            .run(move |c| {
                all_votes
                    .filter(votes::paper_id.eq(paper_id))
                    .filter(votes::user_id.eq(user_id))
                    .load::<Vote>(c)
            })
            .await;
        match votes {
            Ok(votes) => {
                for v in votes {
                    let _ = conn
                        .run(move |c| {
                            diesel::delete(all_votes)
                                .filter(votes::id.eq(v.id.unwrap()))
                                .execute(c)
                        })
                        .await;

                    let _ = conn
                        .run(move |c| {
                            diesel::update(all_papers)
                                .filter(papers::id.eq(paper_id))
                                .set(papers::vote_count.eq(papers::vote_count - 1))
                                .execute(c)
                        })
                        .await;
                }
            }
            Err(e) => {
                error_!("insert() count error: {}", e);
                return false;
            }
        };

        true
    }
    // Add the vote
    pub async fn up(conn: &DbConn, user_id: i32, paper_id: i32) -> bool {
        let count = conn
            .run(move |c| {
                all_votes
                    .filter(votes::paper_id.eq(paper_id))
                    .filter(votes::user_id.eq(user_id))
                    .load::<Vote>(c)
            })
            .await;
        let count = match count {
            Ok(count) => count,
            Err(e) => {
                error_!("insert() count error: {}", e);
                return false;
            }
        };
        println!("Vote count: {:?}", count);
        if count.len() == 0 {
            // Can proceed and add the vote
            let res = conn
                .run(move |c| {
                    let p = Vote {
                        id: None,
                        paper_id,
                        user_id,
                    };
                    diesel::insert_into(votes::table).values(&p).execute(c)
                })
                .await;
            if res.is_err() {
                return false;
            }
            // And increase the number to the vote count
            let res = conn
                .run(move |c| {
                    diesel::update(all_papers)
                        .filter(papers::id.eq(paper_id))
                        .set(papers::vote_count.eq(papers::vote_count + 1))
                        .execute(c)
                })
                .await;
            if res.is_err() {
                return false;
            }

            true
        } else {
            if count.len() > 1 {
                error_!("Count is superior to 1: {:?}", count);
            }
            false
        }
    }
}
