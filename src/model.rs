use std::collections::HashMap;

use chrono::{DateTime, Utc};
use crypto::digest::Digest;
use crypto::sha3::Sha3;
use diesel::{prelude::*, result::QueryResult};
use rocket::serde::Serialize;

// For interacting with the database
use crate::schema::logins::dsl::logins as all_logins;
use crate::schema::papers::dsl::papers as all_papers;
use crate::schema::votes::dsl::votes as all_votes;
use crate::schema::*;

use crate::DbConn;
use crate::RegisterForm;

////////////// Users
#[derive(Queryable, Insertable, Debug, Serialize, Clone)]
#[serde(crate = "rocket::serde")]
pub struct Login {
    pub id: Option<i32>,
    pub name: String,
    pub email: String,
    pub password_hash: String,
    pub is_admin: i32,
    pub is_approved: i32,
    pub is_disabled: i32,
    pub role: String,
    pub last_connected: Option<String>,
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
                is_admin: 0,
                is_approved: 0,
                is_disabled: 0,
                role: register.role,
                last_connected: None,
            };
            diesel::insert_into(logins::table).values(&p).execute(c)
        })
        .await
    }

    pub async fn get(conn: &DbConn, user_id: i32) -> QueryResult<Login> {
        conn.run(move |c| all_logins.filter(logins::id.eq(user_id)).first::<Login>(c))
            .await
    }

    pub async fn promote_to_admin(conn: &DbConn, user_id: i32) -> QueryResult<usize> {
        conn.run(move |c| {
            diesel::update(all_logins)
                .filter(logins::id.eq(user_id))
                .set((
                    logins::is_admin.eq(1),
                    logins::is_approved.eq(1),
                    logins::is_disabled.eq(0),
                    logins::role.eq("prof"),
                ))
                .execute(c)
        })
        .await
    }

    pub async fn approve(conn: &DbConn, user_id: i32) -> QueryResult<usize> {
        conn.run(move |c| {
            diesel::update(all_logins)
                .filter(logins::id.eq(user_id))
                .set(logins::is_approved.eq(1))
                .execute(c)
        })
        .await
    }

    pub async fn set_disabled(conn: &DbConn, user_id: i32, is_disabled: i32) -> QueryResult<usize> {
        conn.run(move |c| {
            diesel::update(all_logins)
                .filter(logins::id.eq(user_id))
                .set(logins::is_disabled.eq(is_disabled))
                .execute(c)
        })
        .await
    }

    pub async fn set_role(conn: &DbConn, user_id: i32, role: String) -> QueryResult<usize> {
        conn.run(move |c| {
            diesel::update(all_logins)
                .filter(logins::id.eq(user_id))
                .set(logins::role.eq(role))
                .execute(c)
        })
        .await
    }

    pub async fn update_last_connected(
        conn: &DbConn,
        user_id: i32,
        timestamp: String,
    ) -> QueryResult<usize> {
        conn.run(move |c| {
            diesel::update(all_logins)
                .filter(logins::id.eq(user_id))
                .set(logins::last_connected.eq(Some(timestamp)))
                .execute(c)
        })
        .await
    }
}

////////////// Papers
#[derive(Queryable, Debug, Serialize, Clone)]
#[serde(crate = "rocket::serde")]
pub struct Paper {
    pub id: Option<i32>,
    pub title: String,
    pub url: String,
    pub venue: Option<String>,
    pub publication_year: Option<i32>,
    pub user_id: i32,
    pub vote_count: i32,
    pub readed: i32,               // 0 = false, 1 = true
    pub pdf_file: Option<String>,  // Filename of uploaded PDF
    pub thumbnail: Option<String>, // Filename of thumbnail image
    pub added_at: String,
    pub discussed_at: Option<String>,
    pub presenter_id: Option<i32>,
}

impl Paper {
    pub async fn get(conn: &DbConn, paper_id: i32) -> QueryResult<Paper> {
        conn.run(move |c| all_papers.filter(papers::id.eq(paper_id)).first::<Paper>(c))
            .await
    }

    pub async fn all_active_with_vote_status(
        conn: &DbConn,
        user_id: Option<i32>,
    ) -> Option<Vec<(Paper, i32)>> {
        let papers = conn
            .run(|c| {
                all_papers
                    .order(papers::vote_count.desc())
                    .filter(papers::readed.eq(0))
                    .filter(papers::discussed_at.is_null())
                    .load::<Paper>(c)
            })
            .await;
        if papers.is_err() {
            return None;
        }
        let papers = papers.unwrap();

        match user_id {
            None => Some(papers.into_iter().map(|p| (p, 0)).collect()),
            Some(user_id) => {
                let votes = conn
                    .run(move |c| all_votes.filter(votes::user_id.eq(user_id)).load::<Vote>(c))
                    .await;
                if votes.is_err() {
                    Some(papers.into_iter().map(|p| (p, 0)).collect())
                } else {
                    let votes: HashMap<i32, i32> = votes
                        .unwrap()
                        .into_iter()
                        .map(|v| (v.paper_id, v.value))
                        .collect();
                    Some(
                        papers
                            .into_iter()
                            .map(|p| {
                                let id = p.id.unwrap();
                                let state = votes.get(&id).copied().unwrap_or(0);
                                (p, state)
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

    pub async fn all_active(conn: &DbConn) -> QueryResult<Vec<Paper>> {
        conn.run(|c| {
            all_papers
                .order(papers::vote_count.desc())
                .filter(papers::readed.eq(0))
                .filter(papers::discussed_at.is_null())
                .load::<Paper>(c)
        })
        .await
    }

    pub async fn all_discussed(conn: &DbConn) -> QueryResult<Vec<Paper>> {
        conn.run(|c| {
            all_papers
                .order(papers::discussed_at.desc())
                .filter(papers::discussed_at.is_not_null())
                .load::<Paper>(c)
        })
        .await
    }

    pub async fn insert(
        conn: &DbConn,
        title: String,
        url: Option<String>,
        venue: Option<String>,
        publication_year: Option<i32>,
        pdf_file: Option<String>,
        thumbnail: Option<String>,
        user_id: i32,
    ) -> QueryResult<usize> {
        conn.run(move |c| {
            diesel::insert_into(papers::table)
                .values((
                    papers::title.eq(title),
                    papers::url.eq(url.unwrap_or_default()),
                    papers::venue.eq(venue),
                    papers::publication_year.eq(publication_year),
                    papers::user_id.eq(user_id),
                    papers::vote_count.eq(0),
                    papers::readed.eq(0),
                    papers::pdf_file.eq(pdf_file),
                    papers::thumbnail.eq(thumbnail),
                ))
                .execute(c)
        })
        .await
    }

    pub async fn mark_discussed(
        conn: &DbConn,
        paper_id: i32,
        discussed_date: String,
        presenter_id: Option<i32>,
    ) -> QueryResult<usize> {
        conn.run(move |c| {
            diesel::update(all_papers)
                .filter(papers::id.eq(paper_id))
                .set((
                    papers::readed.eq(1),
                    papers::discussed_at.eq(Some(discussed_date)),
                    papers::presenter_id.eq(presenter_id),
                ))
                .execute(c)
        })
        .await
    }

    pub async fn update_fields(
        conn: &DbConn,
        paper_id: i32,
        title: String,
        url: String,
        venue: Option<String>,
        publication_year: Option<i32>,
    ) -> QueryResult<usize> {
        conn.run(move |c| {
            diesel::update(all_papers)
                .filter(papers::id.eq(paper_id))
                .set((
                    papers::title.eq(title),
                    papers::url.eq(url),
                    papers::venue.eq(venue),
                    papers::publication_year.eq(publication_year),
                ))
                .execute(c)
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

////////////// User stats
#[derive(Debug, Serialize, Clone)]
#[serde(crate = "rocket::serde")]
pub struct UserWithStats {
    pub id: i32,
    pub name: String,
    pub email: String,
    pub role: String,
    pub role_label: String,
    pub is_admin: bool,
    pub is_approved: bool,
    pub is_disabled: bool,
    pub last_connected_display: String,
    pub papers_proposed: usize,
    pub papers_presented: usize,
}

fn relative_time(ts: &str) -> String {
    let parsed = DateTime::parse_from_rfc3339(ts)
        .map(|dt| dt.with_timezone(&Utc))
        .ok();
    match parsed {
        None => ts.to_string(),
        Some(dt) => {
            let now = Utc::now();
            let diff = now.signed_duration_since(dt);
            let seconds = diff.num_seconds();
            let minutes = diff.num_minutes();
            let hours = diff.num_hours();
            let days = diff.num_days();
            if seconds < 60 {
                "Just now".to_string()
            } else if minutes < 60 {
                if minutes == 1 {
                    "1 minute ago".to_string()
                } else {
                    format!("{} minutes ago", minutes)
                }
            } else if hours < 24 {
                if hours == 1 {
                    "1 hour ago".to_string()
                } else {
                    format!("{} hours ago", hours)
                }
            } else if days == 0 {
                "Today".to_string()
            } else if days == 1 {
                "Yesterday".to_string()
            } else if days < 30 {
                format!("{} days ago", days)
            } else if days < 365 {
                let months = days / 30;
                if months == 1 {
                    "1 month ago".to_string()
                } else {
                    format!("{} months ago", months)
                }
            } else {
                let years = days / 365;
                if years == 1 {
                    "1 year ago".to_string()
                } else {
                    format!("{} years ago", years)
                }
            }
        }
    }
}

pub fn build_users_with_stats(logins: Vec<Login>, papers: &[Paper]) -> Vec<UserWithStats> {
    logins
        .into_iter()
        .filter_map(|login| {
            let uid = login.id?;
            let last_connected_display = match &login.last_connected {
                Some(ts) => relative_time(ts),
                None => "Never".to_string(),
            };
            let papers_proposed = papers.iter().filter(|p| p.user_id == uid).count();
            let papers_presented = papers
                .iter()
                .filter(|p| p.presenter_id == Some(uid) && p.discussed_at.is_some())
                .count();
            let role = crate::normalize_role(&login.role).unwrap_or_else(|| "other".to_string());
            let role_label = crate::role_label(&role).to_string();
            Some(UserWithStats {
                id: uid,
                name: login.name,
                email: login.email,
                role,
                role_label,
                is_admin: login.is_admin == 1,
                is_approved: login.is_approved == 1,
                is_disabled: login.is_disabled == 1,
                last_connected_display,
                papers_proposed,
                papers_presented,
            })
        })
        .collect()
}

#[derive(Queryable, Insertable, Debug, Serialize)]
#[serde(crate = "rocket::serde")]
pub struct Vote {
    pub id: Option<i32>,
    pub paper_id: i32,
    pub user_id: i32,
    pub value: i32,
}
impl Vote {
    pub async fn remove_for_paper(conn: &DbConn, paper_id: i32) -> bool {
        conn.run(move |c| {
            diesel::delete(all_votes)
                .filter(votes::paper_id.eq(paper_id))
                .execute(c)
        })
        .await
        .is_ok()
    }

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
            Ok(existing_votes) => {
                if existing_votes.is_empty() {
                    let res = conn
                        .run(move |c| {
                            let p = Vote {
                                id: None,
                                paper_id,
                                user_id,
                                value: -1,
                            };
                            diesel::insert_into(votes::table).values(&p).execute(c)
                        })
                        .await;
                    return res.is_ok();
                }

                let mut changed = false;
                for v in existing_votes {
                    let vote_id = v.id.unwrap();
                    let previous_value = v.value;

                    if previous_value != -1 {
                        changed = true;
                    }

                    let update_ok = conn
                        .run(move |c| {
                            diesel::update(all_votes)
                                .filter(votes::id.eq(vote_id))
                                .set(votes::value.eq(-1))
                                .execute(c)
                        })
                        .await
                        .is_ok();
                    if !update_ok {
                        return false;
                    }

                    if previous_value == 1 {
                        let decrement_ok = conn
                            .run(move |c| {
                                diesel::update(all_papers)
                                    .filter(papers::id.eq(paper_id))
                                    .set(papers::vote_count.eq(papers::vote_count - 1))
                                    .execute(c)
                            })
                            .await
                            .is_ok();
                        if !decrement_ok {
                            return false;
                        }
                    }
                }

                changed
            }
            Err(e) => {
                error_!("insert() count error: {}", e);
                false
            }
        }
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
        if count.len() == 0 {
            // Can proceed and add the vote
            let res = conn
                .run(move |c| {
                    let p = Vote {
                        id: None,
                        paper_id,
                        user_id,
                        value: 1,
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
            let mut changed = false;
            for vote in count {
                let vote_id = vote.id.unwrap();
                let previous_value = vote.value;

                if previous_value != 1 {
                    changed = true;
                }

                let update_ok = conn
                    .run(move |c| {
                        diesel::update(all_votes)
                            .filter(votes::id.eq(vote_id))
                            .set(votes::value.eq(1))
                            .execute(c)
                    })
                    .await
                    .is_ok();
                if !update_ok {
                    return false;
                }

                if previous_value == -1 {
                    let increment_ok = conn
                        .run(move |c| {
                            diesel::update(all_papers)
                                .filter(papers::id.eq(paper_id))
                                .set(papers::vote_count.eq(papers::vote_count + 1))
                                .execute(c)
                        })
                        .await
                        .is_ok();
                    if !increment_ok {
                        return false;
                    }
                }
            }

            changed
        }
    }
}
