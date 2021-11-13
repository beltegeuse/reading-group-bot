use crypto::sha3::Sha3;
use crypto::digest::Digest;
use diesel::{QueryDsl, result::QueryResult, Queryable, prelude::*};
use rocket::serde::Serialize;

// For interacting with the database
use crate::schema::papers::dsl::papers as all_papers;
use crate::schema::logins::dsl::logins as all_logins;
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
  pub password_hash: String
}
pub fn hash_password(password: String) -> String {
    let mut hasher = Sha3::sha3_256();
    hasher.input_str(&password);
    hasher.result_str()
}
impl Login {
    pub async fn all(conn: &DbConn) -> QueryResult<Vec<Login>> {
        conn.run(|c| {
            all_logins.order(logins::id.desc()).load::<Login>(c)
        }).await
    }

    pub async fn insert(register: RegisterForm, conn: &DbConn) -> QueryResult<usize> {
        conn.run(|c| {
            let p = Login { 
                id: None, 
                name: register.name, 
                email: register.email, 
                password_hash: hash_password(register.password)
            };
            diesel::insert_into(logins::table).values(&p).execute(c)
        }).await
    }
}

////////////// Papers
#[derive(Queryable, Insertable, Debug, Serialize)]
#[serde(crate = "rocket::serde")]
pub struct Paper {
    pub id: Option<i32>,
    pub title: String,
    pub url: String,
    pub venue: Option<String>,
}

impl Paper {
    pub async fn all(conn: &DbConn) -> QueryResult<Vec<Paper>> {
        conn.run(|c| {
            all_papers.order(papers::id.desc()).load::<Paper>(c)
        }).await
    }

    pub async fn insert(paper: PaperForm, conn: &DbConn) -> QueryResult<usize> {
        conn.run(|c| {
            let p = Paper { 
                id: None, 
                title: paper.title, 
                url: paper.url, 
                venue: if paper.venue == "" {
                    None
                } else {
                    Some(paper.venue)
                } 
            };
            diesel::insert_into(papers::table).values(&p).execute(c)
        }).await
    }
}
