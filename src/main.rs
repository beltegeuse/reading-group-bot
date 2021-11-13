#[macro_use]
extern crate rocket;
#[macro_use]
extern crate rocket_sync_db_pools;
#[macro_use]
extern crate diesel;
#[macro_use]
extern crate diesel_migrations;

// Use all modelss
use model::*;

// Rocket tools
use rocket::fairing::AdHoc;
use rocket::form::Form;
use rocket::fs::{relative, FileServer};
use rocket::http::{Cookie, CookieJar};
use rocket::request::FlashMessage;
use rocket::response::{Flash, Redirect};
use rocket::serde::Serialize;
use rocket::{Build, Rocket};
use rocket_dyn_templates::Template;

pub mod model;
pub mod schema;

#[database("sqlite_database")]
pub struct DbConn(diesel::SqliteConnection);

// Form
#[allow(dead_code)]
#[derive(FromForm, Debug)]
pub struct SlackRequest {
    pub token: String,
    pub text: String,
    pub channel_id: String,
    pub team_id: String,
    pub team_domain: String,
    pub channel_name: String,
    pub user_id: String,
    pub user_name: String,
    pub command: String,
    pub response_url: String,
}

#[derive(Serialize, Debug)]
pub struct SlackResponse {
    pub response_type: String,
    pub text: String,
}

// Extract from cookie
pub struct CookieInfo {
    id: i32,
    name: String,
}
fn read_cookie(jar: &CookieJar<'_>) -> Option<CookieInfo> {
    let username = jar.get_private("name");
    let id = jar.get_private("user_id");
    match (username, id) {
        (Some(username), Some(id)) => Some(CookieInfo {
            name: username.value().to_string(),
            id: id.value().parse().unwrap(),
        }),
        _ => None,
    }
}

// Context for the template
#[derive(Debug, Serialize)]
#[serde(crate = "rocket::serde")]
struct Context {
    flash: Option<(String, String)>,
    papers: Vec<Paper>,
    username: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(crate = "rocket::serde")]
struct ContextNull {}

// Forward flash message if we have one
#[get("/")]
async fn index(jar: &CookieJar<'_>, flash: Option<FlashMessage<'_>>, conn: DbConn) -> Template {
    let cookie_info = read_cookie(jar);
    let flash = flash.map(FlashMessage::into_inner);
    let context = match Paper::all(&conn).await {
        Ok(papers) => Context {
            flash,
            papers,
            username: cookie_info.and_then(|c| Some(c.name)),
        },
        Err(e) => {
            error_!("index() error: {}", e);
            Context {
                flash: Some(("error".into(), "Fail to access database.".into())),
                papers: vec![],
                username: cookie_info.and_then(|c| Some(c.name)),
            }
        }
    };
    Template::render("index", &context)
}

#[derive(Debug, FromForm)]
pub struct PaperForm {
    pub title: String,
    pub url: String,
    pub venue: String,
}
#[post("/", data = "<paper_form>")]
async fn new(jar: &CookieJar<'_>, paper_form: Form<PaperForm>, conn: DbConn) -> Flash<Redirect> {
    // Check login first
    let cookie_info = read_cookie(jar);
    if cookie_info.is_none() {
        return Flash::error(Redirect::to("/"), "Impossible to add paper without login first");
    }
    let cookie_info = cookie_info.unwrap();
    
    // Check if the form is correct
    let paper = paper_form.into_inner();
    if paper.title.is_empty() {
        return Flash::error(Redirect::to("/"), "Title cannot be empty.");
    }
    if paper.url.is_empty() {
        return Flash::error(Redirect::to("/"), "URL cannot be empty.");
    }

    // Check if the paper was already proposed
    match Paper::all(&conn).await {
        Err(e) => {
            error_!("new() error: {}", e);
            return Flash::error(
                Redirect::to("/"),
                "Paper could not be inserted due an internal error.",
            );
        }
        Ok(papers) => {
            for p in papers {
                // TODO: Do it more robustly
                if p.title == paper.title {
                    error_!("new() paper already added");
                    return Flash::error(Redirect::to("/"), "Paper have been already proposed!");
                }
            }
        }
    }

    if let Err(e) = Paper::insert(paper, &conn, cookie_info.id).await {
        error_!("DB insertion error: {}", e);
        Flash::error(
            Redirect::to("/"),
            "Paper could not be inserted due an internal error.",
        )
    } else {
        Flash::success(Redirect::to("/"), "Paper successfully added.")
    }
}

#[derive(FromForm)]
struct LoginForm {
    name: String,
    password: String,
}
#[get("/login")]
fn user_login() -> Template {
    let context = ContextNull {};
    Template::render("login", &context)
}
#[post("/login", data = "<login_form>")]
async fn user_login_post(
    jar: &CookieJar<'_>,
    conn: DbConn,
    login_form: Form<LoginForm>,
) -> Flash<Redirect> {
    // Check the entry
    let login = login_form.into_inner();
    if login.name.is_empty() {
        return Flash::error(Redirect::to("/"), "Name cannot be empty.");
    }
    if login.password.is_empty() {
        return Flash::error(Redirect::to("/"), "Password cannot be empty.");
    }

    // Check matched password
    let pwd_hash = hash_password(login.password);
    match Login::all(&conn).await {
        Err(e) => {
            error_!("new() error: {}", e);
            return Flash::error(
                Redirect::to("/"),
                "Login could not be inserted due an internal error.",
            );
        }
        Ok(logins) => {
            for l in logins {
                if l.name == login.name {
                    if l.password_hash == pwd_hash {
                        jar.add_private(Cookie::new("user_id", l.id.unwrap().to_string()));
                        jar.add_private(Cookie::new("name", l.name));
                        return Flash::success(Redirect::to("/"), "Successfully logged.");
                    } else {
                        return Flash::error(Redirect::to("/"), "Wrong password.");
                    }
                }
            }
        }
    }

    Flash::error(Redirect::to("/"), "User not found!")
}

#[get("/logout")]
async fn user_logout(jar: &CookieJar<'_>) -> Flash<Redirect> {
    jar.remove(Cookie::named("user_id"));
    jar.remove(Cookie::named("name"));
    Flash::success(Redirect::to("/"), "User successfully logout.")
}

#[derive(FromForm)]
pub struct RegisterForm {
    pub name: String,
    pub email: String,
    pub password: String,
}
#[get("/register")]
fn user_register() -> Template {
    let context = ContextNull {};
    Template::render("register", &context)
}
#[post("/register", data = "<register_form>")]
async fn user_register_post(conn: DbConn, register_form: Form<RegisterForm>) -> Flash<Redirect> {
    // Check the entry
    let register = register_form.into_inner();
    if register.name.is_empty() {
        return Flash::error(Redirect::to("/"), "Name cannot be empty.");
    }
    if register.email.is_empty() {
        return Flash::error(Redirect::to("/"), "Email cannot be empty.");
    }
    if register.password.is_empty() {
        return Flash::error(Redirect::to("/"), "Password cannot be empty.");
    }

    // Check if we do not have multiple users
    match Login::all(&conn).await {
        Err(e) => {
            error_!("new() error: {}", e);
            return Flash::error(
                Redirect::to("/"),
                "Paper could not be inserted due an internal error.",
            );
        }
        Ok(logins) => {
            for l in logins {
                // TODO: Do it more robustly
                if l.name == register.name {
                    return Flash::error(Redirect::to("/"), "Name is already taken!");
                }
                if l.email == register.email {
                    return Flash::error(Redirect::to("/"), "Email is already taken!");
                }
            }
        }
    }

    if let Err(e) = Login::insert(register, &conn).await {
        error_!("DB insertion error: {}", e);
        Flash::error(
            Redirect::to("/"),
            "User could not be inserted due an internal error.",
        )
    } else {
        Flash::success(Redirect::to("/"), "User successfully added.")
    }
}

async fn run_migrations(rocket: Rocket<Build>) -> Rocket<Build> {
    // This macro from `diesel_migrations` defines an `embedded_migrations`
    // module containing a function named `run`. This allows the example to be
    // run and tested without any outside setup of the database.
    embed_migrations!();

    let conn = DbConn::get_one(&rocket).await.expect("database connection");
    conn.run(|c| embedded_migrations::run(c))
        .await
        .expect("can run migrations");

    rocket
}

#[launch]
fn rocket() -> _ {
    rocket::build()
        .attach(Template::fairing())
        .attach(DbConn::fairing())
        .attach(AdHoc::on_ignite("Run Migrations", run_migrations))
        .mount("/", FileServer::from(relative!("static")))
        .mount("/", routes![index]) // Main list of papers
        .mount("/paper", routes![new]) // Managing papers
        .mount(
            "/user",
            routes![
                user_login,
                user_login_post,
                user_register,
                user_register_post,
                user_logout
            ],
        )
}
