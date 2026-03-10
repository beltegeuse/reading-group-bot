#[macro_use]
extern crate rocket;
#[macro_use]
extern crate rocket_sync_db_pools;
#[macro_use]
extern crate diesel;

use std::fs;
use uuid::Uuid;

use diesel_migrations::{embed_migrations, MigrationHarness};
use diesel_migrations::EmbeddedMigrations;
// Use all modelss
use model::*;
use user::seed_default_user;

// Rocket tools
use rocket::fairing::AdHoc;
use rocket::form::{Contextual, Form};
use rocket::fs::{relative, FileServer, TempFile};
use rocket::http::{Cookie, CookieJar, Status};
use rocket::request::FlashMessage;
use rocket::response::{Flash, Redirect};
use rocket::serde::Serialize;
use rocket::{Build, Rocket};
use rocket_dyn_templates::Template;

pub mod model;
pub mod schema;
pub mod user;
pub mod pdf_utils;

#[database("sqlite_database")]
pub struct DbConn(diesel::SqliteConnection);

// Form
// #[allow(dead_code)]
// #[derive(FromForm, Debug)]
// pub struct SlackRequest {
//     pub token: String,
//     pub text: String,
//     pub channel_id: String,
//     pub team_id: String,
//     pub team_domain: String,
//     pub channel_name: String,
//     pub user_id: String,
//     pub user_name: String,
//     pub command: String,
//     pub response_url: String,
// }

// #[derive(Serialize, Debug)]
// pub struct SlackResponse {
//     pub response_type: String,
//     pub text: String,
// }

// Extract from cookie
#[derive(Debug, Serialize)]
#[serde(crate = "rocket::serde")]
pub struct CookieInfo {
    id: i32,
    name: String,
    is_admin: bool,
}
fn read_cookie(jar: &CookieJar<'_>) -> Option<CookieInfo> {
    let username = jar.get_private("name");
    let id = jar.get_private("user_id");
    let is_admin = jar
        .get_private("is_admin")
        .and_then(|cookie| cookie.value().parse::<i32>().ok())
        .map(|value| value == 1)
        .unwrap_or(false);

    match (username, id) {
        (Some(username), Some(id)) => Some(CookieInfo {
            name: username.value().to_string(),
            id: id.value().parse().unwrap(),
            is_admin,
        }),
        _ => None,
    }
}

// Context for the template
#[derive(Debug, Serialize)]
#[serde(crate = "rocket::serde")]
struct Context {
    flash: Option<(String, String)>,
    papers: Vec<(Paper, i32)>,
    cookie_info: Option<CookieInfo>,
    search_query: String,
    only_not_voted: bool,
}

#[derive(Debug, Serialize)]
#[serde(crate = "rocket::serde")]
struct PaperListContext {
    flash: Option<(String, String)>,
    papers: Vec<Paper>,
    cookie_info: Option<CookieInfo>,
}

#[derive(Debug, Serialize)]
#[serde(crate = "rocket::serde")]
struct ContextNull {}

#[derive(Debug, Serialize)]
#[serde(crate = "rocket::serde")]
struct PaperAddContext {
    flash: Option<(String, String)>,
    cookie_info: Option<CookieInfo>,
}

fn paper_matches_search(paper: &Paper, query: &str) -> bool {
    let query = query.to_lowercase();
    paper.title.to_lowercase().contains(&query)
        || paper.url.to_lowercase().contains(&query)
        || paper
            .venue
            .as_ref()
            .map(|venue| venue.to_lowercase().contains(&query))
            .unwrap_or(false)
        || paper
            .publication_year
            .map(|year| year.to_string().contains(&query))
            .unwrap_or(false)
}

const ALLOWED_VENUES: &[&str] = &[
    "SIGGRAPH / SIGGRAPH Asia",
    "Eurographics (EG)",
    "Pacific Graphics (PG)",
    "Symposium on Geometry Processing (SGP)",
    "Symposium on Rendering (EGSR)",
    "High Performance Graphics (HPG)",
    "IEEE VIS (Visualization)",
    "IEEE VR",
    "CVPR, ICCV, ECCV (computer vision / graphics overlap)",
    "3DV",
    "ACM Transactions on Graphics (TOG)",
    "IEEE Transactions on Visualization and Computer Graphics (TVCG)",
    "Computer Graphics Forum (CGF)",
    "Computers & Graphics (C&G)",
    "The Visual Computer (TVC)",
    "IEEE Computer Graphics and Applications (CG&A)",
    "arXiv (cs.GR for graphics, cs.CV for vision)",
];

// Forward flash message if we have one
#[get("/?<q>&<only_not_voted>")]
async fn index(
    jar: &CookieJar<'_>,
    flash: Option<FlashMessage<'_>>,
    q: Option<String>,
    only_not_voted: Option<i32>,
    conn: DbConn,
) -> Template {
    // Get context informations
    let cookie_info = read_cookie(jar);
    let flash = flash.map(FlashMessage::into_inner);
    let search_query = q.unwrap_or_default().trim().to_string();
    let only_not_voted = only_not_voted.unwrap_or(0) == 1;
    let user_id = match &cookie_info {
        None => None,
        Some(i) => Some(i.id),
    };

    // Build context on paper unread
    let context = match Paper::all_active_with_vote_status(&conn, user_id).await {
        Some(mut papers) => {
            if !search_query.is_empty() {
                papers.retain(|(paper, _)| paper_matches_search(paper, &search_query));
            }
            if only_not_voted {
                papers.retain(|(_, vote_state)| *vote_state == 0);
            }

            Context {
                flash,
                papers,
                cookie_info,
                search_query,
                only_not_voted,
            }
        }
        None => Context {
            flash,
            papers: vec![],
            cookie_info,
            search_query,
            only_not_voted,
        },
    };
    Template::render("index", &context)
}

#[get("/ranking")]
async fn paper_ranking(
    jar: &CookieJar<'_>,
    flash: Option<FlashMessage<'_>>,
    conn: DbConn,
) -> Template {
    let cookie_info = read_cookie(jar);
    let flash = flash.map(FlashMessage::into_inner);

    let papers = match Paper::all_active(&conn).await {
        Ok(papers) => papers,
        Err(_) => vec![],
    };

    let context = PaperListContext {
        flash,
        papers,
        cookie_info,
    };

    Template::render("ranking", &context)
}

#[get("/discussed")]
async fn paper_discussed(
    jar: &CookieJar<'_>,
    flash: Option<FlashMessage<'_>>,
    conn: DbConn,
) -> Template {
    let cookie_info = read_cookie(jar);
    let flash = flash.map(FlashMessage::into_inner);

    let papers = match Paper::all_discussed(&conn).await {
        Ok(papers) => papers,
        Err(_) => vec![],
    };

    let context = PaperListContext {
        flash,
        papers,
        cookie_info,
    };

    Template::render("discussed", &context)
}

#[get("/admin")]
async fn paper_admin_discussed(
    jar: &CookieJar<'_>,
    flash: Option<FlashMessage<'_>>,
    conn: DbConn,
) -> Result<Template, Flash<Redirect>> {
    let cookie_info = read_cookie(jar);
    if cookie_info.as_ref().map(|user| user.is_admin).unwrap_or(false) == false {
        return Err(Flash::error(
            Redirect::to("/"),
            "Only admin users can access the discussed management page.",
        ));
    }

    let flash = flash.map(FlashMessage::into_inner);
    let papers = match Paper::all_active(&conn).await {
        Ok(papers) => papers,
        Err(_) => vec![],
    };
    let context = PaperListContext {
        flash,
        papers,
        cookie_info,
    };

    Ok(Template::render("admin_discussed", &context))
}

#[get("/add")]
async fn paper_add(jar: &CookieJar<'_>, flash: Option<FlashMessage<'_>>) -> Template {
    let cookie_info = read_cookie(jar);
    let flash = flash.map(FlashMessage::into_inner);
    let context = PaperAddContext { flash, cookie_info };
    Template::render("paper", &context)
}

#[derive(Debug, FromForm)]
pub struct DiscussForm {
    pub discussed_date: String,
}

#[derive(Debug, FromForm)]
pub struct PaperForm<'r> {
    pub title: String,
    pub link: Option<String>,
    pub venue: String,
    pub venue_other: Option<String>,
    pub year: i32,
    pub pdf: Option<TempFile<'r>>,
}
#[post("/add", data = "<paper_form>")]
async fn paper_add_post(
    jar: &CookieJar<'_>,
    paper_form: Form<Contextual<'_, PaperForm<'_>>>,
    conn: DbConn,
) -> Flash<Redirect> {
    // Check login first
    let cookie_info = read_cookie(jar);
    if cookie_info.is_none() {
        return Flash::error(
            Redirect::to("/"),
            "Impossible to add paper without login first",
        );
    }
    let cookie_info = cookie_info.unwrap();

    let paper_form = paper_form.into_inner();
    let upload_too_large = paper_form.context.status() == Status::PayloadTooLarge
        || paper_form
            .context
            .field_errors("pdf")
            .any(|error| error.status() == Status::PayloadTooLarge);

    if upload_too_large {
        return Flash::error(
            Redirect::to("/paper/add"),
            "Uploaded PDF is too large. Maximum allowed size is 200 MiB.",
        );
    }

    let mut paper = match paper_form.value {
        Some(paper) => paper,
        None => {
            return Flash::error(
                Redirect::to("/paper/add"),
                "Invalid paper submission. Please check the form and try again.",
            )
        }
    };

    // Check if the form is correct
    let title = paper.title.trim().to_string();
    if title.is_empty() {
        return Flash::error(Redirect::to("/paper/add"), "Title cannot be empty.");
    }

    let link = paper
        .link
        .take()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());

    let selected_venue = paper.venue.trim().to_string();
    if selected_venue.is_empty() {
        return Flash::error(Redirect::to("/paper/add"), "Venue must be selected.");
    }
    let venue = if selected_venue == "Other" {
        let venue_other = paper
            .venue_other
            .take()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());

        if venue_other.is_none() {
            return Flash::error(
                Redirect::to("/paper/add"),
                "Please specify the venue when selecting Other.",
            );
        }

        venue_other
    } else if ALLOWED_VENUES
        .iter()
        .any(|allowed_venue| *allowed_venue == selected_venue.as_str())
    {
        Some(selected_venue)
    } else {
        return Flash::error(Redirect::to("/paper/add"), "Invalid venue selection.");
    };

    if !(1900..=2100).contains(&paper.year) {
        return Flash::error(
            Redirect::to("/paper/add"),
            "Publication year must be between 1900 and 2100.",
        );
    }
    let publication_year = Some(paper.year);

    let has_pdf = paper
        .pdf
        .as_ref()
        .map(|pdf| pdf.len() > 0)
        .unwrap_or(false);

    if !has_pdf {
        return Flash::error(Redirect::to("/paper/add"), "A PDF file is required.");
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
                if p.title.eq_ignore_ascii_case(&title) {
                    error_!("new() paper already added");
                    return Flash::error(Redirect::to("/"), "Paper have been already proposed!");
                }
            }
        }
    }

    let pdf = paper.pdf.as_mut().unwrap();
    let is_pdf = pdf
        .content_type()
        .and_then(|content_type| content_type.extension())
        .map(|ext| ext.as_str().eq_ignore_ascii_case("pdf"))
        .unwrap_or(false)
        || pdf
            .name()
            .map(|name| name.to_ascii_lowercase().ends_with(".pdf"))
            .unwrap_or(false);

    if !is_pdf {
        return Flash::error(Redirect::to("/paper/add"), "Uploaded file must be a PDF.");
    }

    if let Err(e) = fs::create_dir_all("static/pdfs") {
        error_!("create pdf directory error: {}", e);
        return Flash::error(
            Redirect::to("/paper/add"),
            "Could not prepare PDF storage.",
        );
    }

    if let Err(e) = fs::create_dir_all("static/thumbnails") {
        error_!("create thumbnail directory error: {}", e);
        return Flash::error(
            Redirect::to("/paper/add"),
            "Could not prepare thumbnail storage.",
        );
    }

    let filename = format!("{}.pdf", Uuid::new_v4());
    let filepath = format!("static/pdfs/{}", filename);
    if let Err(e) = pdf.move_copy_to(&filepath).await {
        error_!("persist pdf error: {}", e);
        return Flash::error(
            Redirect::to("/paper/add"),
            "Could not save the uploaded PDF.",
        );
    }
    let pdf_file = Some(filename);

    // Generate thumbnail
    let thumbnail = if pdf_file.is_some() {
        let thumbnail_filename = format!("{}.png", Uuid::new_v4());
        let thumbnail_filepath = format!("static/thumbnails/{}", thumbnail_filename);
        
        match pdf_utils::generate_thumbnail(&filepath, &thumbnail_filepath).await {
            Ok(_) => {
                info_!("Thumbnail generated: {}", thumbnail_filename);
                Some(thumbnail_filename)
            }
            Err(e) => {
                warn_!("Failed to generate thumbnail: {}", e);
                None
            }
        }
    } else {
        None
    };

    if let Err(e) = Paper::insert(
        &conn,
        title,
        link,
        venue,
        publication_year,
        pdf_file.clone(),
        thumbnail,
        cookie_info.id,
    )
    .await
    {
        error_!("DB insertion error: {}", e);
        if let Some(pdf_file) = pdf_file {
            let _ = fs::remove_file(format!("static/pdfs/{}", pdf_file));
        }
        Flash::error(
            Redirect::to("/"),
            "Paper could not be inserted due an internal error.",
        )
    } else {
        Flash::success(Redirect::to("/"), "Paper successfully added.")
    }
}

#[put("/discuss/<id>", data = "<discuss_form>")]
async fn paper_mark_discussed(
    jar: &CookieJar<'_>,
    conn: DbConn,
    id: i32,
    discuss_form: Form<DiscussForm>,
) -> Flash<Redirect> {
    let cookie_info = read_cookie(jar);
    if cookie_info.as_ref().map(|user| user.is_admin).unwrap_or(false) == false {
        return Flash::error(
            Redirect::to("/"),
            "Only admin users can mark a paper as discussed.",
        );
    }

    let discussed_date = discuss_form.into_inner().discussed_date.trim().to_string();
    if discussed_date.is_empty() {
        return Flash::error(
            Redirect::to("/paper/admin"),
            "Please provide the date when the paper was discussed.",
        );
    }

    match Paper::mark_discussed(&conn, id, discussed_date).await {
        Ok(updated_rows) if updated_rows > 0 => {
            Flash::success(Redirect::to("/paper/admin"), "Paper marked as discussed.")
        }
        _ => Flash::error(
            Redirect::to("/paper/admin"),
            "Could not mark paper as discussed.",
        ),
    }
}

#[put("/remove/<id>")]
async fn paper_remove(jar: &CookieJar<'_>, conn: DbConn, id: i32) -> Flash<Redirect> {
    let cookie_info = read_cookie(jar);
    if cookie_info.is_none() {
        return Flash::error(
            Redirect::to("/"),
            "Impossible to remove paper without login",
        );
    }
    let cookie_info = cookie_info.unwrap();

    match Paper::get(&conn, id).await {
        Err(e) => {
            error_!("remove paper: {}", e);
            Flash::error(Redirect::to("/"), "Impossible to retrive paper")
        }
        Ok(paper) => {
            if paper.user_id == cookie_info.id {
                let pdf_file = paper.pdf_file.clone();
                let thumbnail = paper.thumbnail.clone();
                // We allow to remove it
                let _ = Vote::remove_for_paper(&conn, id).await;
                let _ = Paper::remove(&conn, id).await;
                if let Some(pdf_file) = pdf_file {
                    let _ = fs::remove_file(format!("static/pdfs/{}", pdf_file));
                }
                if let Some(thumbnail) = thumbnail {
                    let _ = fs::remove_file(format!("static/thumbnails/{}", thumbnail));
                }
            }
            Flash::success(Redirect::to("/"), format!("Paper removed: {}", id))
        }
    }
}

#[put("/up/<id>")]
async fn paper_vote_up(jar: &CookieJar<'_>, conn: DbConn, id: i32) -> Flash<Redirect> {
    let cookie_info = read_cookie(jar);
    if cookie_info.is_none() {
        return Flash::error(Redirect::to("/"), "Impossible to vote without login");
    }
    let cookie_info = cookie_info.unwrap();

    let res = Vote::up(&conn, cookie_info.id, id).await;
    if res {
        Flash::success(Redirect::to("/"), "Marked as interested.")
    } else {
        Flash::error(Redirect::to("/"), "You already marked this paper as interested.")
    }
}
#[put("/down/<id>")]
async fn paper_vote_down(jar: &CookieJar<'_>, conn: DbConn, id: i32) -> Flash<Redirect> {
    let cookie_info = read_cookie(jar);
    if cookie_info.is_none() {
        return Flash::error(Redirect::to("/"), "Impossible to vote without login");
    }
    let cookie_info = cookie_info.unwrap();

    let res = Vote::down(&conn, cookie_info.id, id).await;
    if res {
        Flash::success(Redirect::to("/"), "Marked as ignored.")
    } else {
        Flash::error(Redirect::to("/"), "You already marked this paper as ignored.")
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
                        jar.add_private(Cookie::new("name", l.name.clone()));
                        jar.add_private(Cookie::new("is_admin", l.is_admin.to_string()));
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
    jar.remove_private(Cookie::from("user_id"));
    jar.remove_private(Cookie::from("name"));
    jar.remove_private(Cookie::from("is_admin"));
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

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("./migrations");

async fn run_migrations(rocket: Rocket<Build>) -> Rocket<Build> {
    let conn = DbConn::get_one(&rocket).await.expect("database connection");
    conn.run(|c| c.run_pending_migrations(MIGRATIONS).map(|_| ()))
        .await
        .unwrap();
    seed_default_user(&conn).await;
    if let Err(e) = fs::create_dir_all("static/pdfs") {
        error_!("cannot create pdf directory: {}", e);
    }
    if let Err(e) = fs::create_dir_all("static/thumbnails") {
        error_!("cannot create thumbnails directory: {}", e);
    }

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
        .mount(
            "/paper",
            routes![
                paper_add,
                paper_add_post,
                paper_remove,
                paper_vote_up,
                paper_vote_down,
                paper_ranking,
                paper_discussed,
                paper_admin_discussed,
                paper_mark_discussed
            ],
        ) // Managing papers
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
