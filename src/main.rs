#[macro_use]
extern crate rocket;
#[macro_use]
extern crate rocket_sync_db_pools;
#[macro_use]
extern crate diesel;

use std::fs;
use uuid::Uuid;

use diesel_migrations::EmbeddedMigrations;
use diesel_migrations::{embed_migrations, MigrationHarness};
// Use all modelss
use model::*;
use user::seed_default_user;

// Rocket tools
use chrono::Utc;
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
pub mod pdf_utils;
pub mod schema;
pub mod user;

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
struct PaperWithUsername {
    paper: Paper,
    vote_state: i32,
    username: String,
}

#[derive(Debug, Serialize)]
#[serde(crate = "rocket::serde")]
struct Context {
    flash: Option<(String, String)>,
    papers: Vec<PaperWithUsername>,
    cookie_info: Option<CookieInfo>,
    search_query: String,
    only_not_voted: bool,
}

#[derive(Debug, Serialize)]
#[serde(crate = "rocket::serde")]
struct PaperWithUserInfo {
    paper: Paper,
    username: String,
}

#[derive(Debug, Serialize)]
#[serde(crate = "rocket::serde")]
struct PaperListContext {
    flash: Option<(String, String)>,
    papers: Vec<PaperWithUserInfo>,
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
    can_submit: bool,
    access_message: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(crate = "rocket::serde")]
struct PaperEditContext {
    flash: Option<(String, String)>,
    cookie_info: Option<CookieInfo>,
    paper: Paper,
    selected_venue: String,
    venue_other: String,
}

#[derive(Debug, Serialize)]
#[serde(crate = "rocket::serde")]
struct RoleAssignmentInfo {
    role_name: String,
    role_label: String,
    assigned_user_id: Option<i32>,
    suggested_user_id: Option<i32>,
}

#[derive(Debug, Serialize)]
#[serde(crate = "rocket::serde")]
struct SelectedPaperInfo {
    paper: Paper,
    proposer_name: String,
    roles: Vec<RoleAssignmentInfo>,
}

#[derive(Debug, Serialize)]
#[serde(crate = "rocket::serde")]
struct RoleDisplay {
    role_label: String,
    user_name: String,
}

#[derive(Debug, Serialize)]
#[serde(crate = "rocket::serde")]
struct DiscussedPaperWithRoles {
    paper: Paper,
    proposer_name: String,
    roles: Vec<RoleDisplay>,
}

#[derive(Debug, Serialize)]
#[serde(crate = "rocket::serde")]
struct AdminContext {
    flash: Option<(String, String)>,
    papers: Vec<PaperWithUserInfo>,
    selected_papers: Vec<SelectedPaperInfo>,
    users: Vec<UserWithStats>,
    cookie_info: Option<CookieInfo>,
}

#[derive(Debug, Serialize)]
#[serde(crate = "rocket::serde")]
struct ScheduleContext {
    flash: Option<(String, String)>,
    selected_papers: Vec<DiscussedPaperWithRoles>,
    discussed_papers: Vec<DiscussedPaperWithRoles>,
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

fn normalize_role(role: &str) -> Option<String> {
    match role.trim().to_lowercase().as_str() {
        "master_student" | "master student" | "master students" => {
            Some("master_student".to_string())
        }
        "phd_student" | "phd student" | "phd students" => Some("phd_student".to_string()),
        "prof" | "profs" | "professor" | "professors" => Some("prof".to_string()),
        "other" => Some("other".to_string()),
        _ => None,
    }
}

fn role_label(role: &str) -> &'static str {
    match role {
        "master_student" => "Master student",
        "phd_student" => "PhD student",
        "prof" => "Prof",
        _ => "Other",
    }
}

fn can_manage_paper(login: &Login, paper: &Paper, current_user_id: i32) -> bool {
    login.is_admin == 1 || paper.user_id == current_user_id
}

fn session_role_label(role: &str) -> &str {
    match role {
        "reviewer_friendly" => "Friendly Reviewer",
        "reviewer_adversarial" => "Adversarial Reviewer",
        "archaeologist" => "Archaeologist",
        "futurist" => "Futurist",
        _ => role,
    }
}

async fn build_role_display_for_paper(
    conn: &DbConn,
    paper: &Paper,
    proposer_name: &str,
) -> Vec<RoleDisplay> {
    let mut roles = vec![RoleDisplay {
        role_label: "Investigator".to_string(),
        user_name: proposer_name.to_string(),
    }];

    let role_rows = PaperRole::for_paper(conn, paper.id.unwrap_or(0))
        .await
        .unwrap_or_default();
    let role_map: std::collections::HashMap<String, i32> = role_rows
        .into_iter()
        .map(|role| (role.role_name, role.user_id))
        .collect();

    for &role_name in SESSION_ROLES {
        if let Some(user_id) = role_map.get(role_name) {
            let user_name = Login::get(conn, *user_id)
                .await
                .map(|login| login.name)
                .unwrap_or_else(|_| "Unknown".to_string());
            roles.push(RoleDisplay {
                role_label: session_role_label(role_name).to_string(),
                user_name,
            });
        }
    }

    roles
}

async fn build_discussed_with_roles(conn: &DbConn, paper: Paper) -> DiscussedPaperWithRoles {
    let proposer_name = Login::get(conn, paper.user_id)
        .await
        .map(|login| login.name)
        .unwrap_or_else(|_| "Unknown".to_string());
    let roles = build_role_display_for_paper(conn, &paper, &proposer_name).await;

    DiscussedPaperWithRoles {
        paper,
        proposer_name,
        roles,
    }
}

const ADMIN_CONTACT_EMAIL: &str = "adrien.gruson@etsmtl.ca";

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

            let mut papers_with_username = Vec::new();
            for (paper, vote_state) in papers {
                let username = match Login::get(&conn, paper.user_id).await {
                    Ok(login) => login.name,
                    Err(_) => "Unknown".to_string(),
                };
                papers_with_username.push(PaperWithUsername {
                    paper,
                    vote_state,
                    username,
                });
            }

            Context {
                flash,
                papers: papers_with_username,
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
        Ok(papers) => {
            let mut papers_with_username = Vec::new();
            for paper in papers {
                let username = match Login::get(&conn, paper.user_id).await {
                    Ok(login) => login.name,
                    Err(_) => "Unknown".to_string(),
                };
                papers_with_username.push(PaperWithUserInfo { paper, username });
            }
            papers_with_username
        }
        Err(_) => vec![],
    };

    let context = PaperListContext {
        flash,
        papers,
        cookie_info,
    };

    Template::render("ranking", &context)
}

#[get("/admin")]
async fn paper_admin_discussed(
    jar: &CookieJar<'_>,
    flash: Option<FlashMessage<'_>>,
    conn: DbConn,
) -> Result<Template, Flash<Redirect>> {
    let cookie_info = read_cookie(jar);
    let admin_login = match &cookie_info {
        Some(user) => Login::get(&conn, user.id).await.ok(),
        None => None,
    };
    if admin_login
        .as_ref()
        .map(|user| user.is_admin == 1 && user.is_disabled == 0)
        .unwrap_or(false)
        == false
    {
        return Err(Flash::error(
            Redirect::to("/"),
            "Only admin users can access the session planning page.",
        ));
    }

    let flash = flash.map(FlashMessage::into_inner);

    // Active (not yet selected) papers stay in the lower section.
    let papers = match Paper::all_active_not_selected(&conn).await {
        Ok(papers) => {
            let mut papers_with_username = Vec::new();
            for paper in papers {
                let username = match Login::get(&conn, paper.user_id).await {
                    Ok(login) => login.name,
                    Err(_) => "Unknown".to_string(),
                };
                papers_with_username.push(PaperWithUserInfo { paper, username });
            }
            papers_with_username
        }
        Err(_) => vec![],
    };

    let all_logins = Login::all(&conn).await.unwrap_or_default();
    let all_papers_for_stats = Paper::all(&conn).await.unwrap_or_default();
    let users = build_users_with_stats(all_logins.clone(), &all_papers_for_stats);

    // Build selected papers with current role assignments and auto-suggestions.
    let role_counts = PaperRole::role_counts_for_discussed(&conn).await;
    let selected_papers = match Paper::all_selected(&conn).await {
        Ok(sel_papers) => {
            let mut result = Vec::new();
            for paper in sel_papers {
                let proposer_name = match Login::get(&conn, paper.user_id).await {
                    Ok(l) => l.name,
                    Err(_) => "Unknown".to_string(),
                };
                let current_roles = PaperRole::for_paper(&conn, paper.id.unwrap_or(0))
                    .await
                    .unwrap_or_default();
                let current_map: std::collections::HashMap<String, i32> = current_roles
                    .iter()
                    .map(|r| (r.role_name.clone(), r.user_id))
                    .collect();
                let suggestions = auto_suggest_roles(&role_counts, &all_logins, paper.user_id);
                let roles = SESSION_ROLES
                    .iter()
                    .map(|&r| RoleAssignmentInfo {
                        role_name: r.to_string(),
                        role_label: session_role_label(r).to_string(),
                        assigned_user_id: current_map.get(r).copied(),
                        suggested_user_id: suggestions.get(r).copied(),
                    })
                    .collect();
                result.push(SelectedPaperInfo {
                    paper,
                    proposer_name,
                    roles,
                });
            }
            result
        }
        Err(_) => vec![],
    };

    let context = AdminContext {
        flash,
        papers,
        selected_papers,
        users,
        cookie_info,
    };

    Ok(Template::render("admin_discussed", &context))
}

#[get("/schedule")]
async fn paper_schedule(
    jar: &CookieJar<'_>,
    flash: Option<FlashMessage<'_>>,
    conn: DbConn,
) -> Template {
    let cookie_info = read_cookie(jar);
    let flash = flash.map(FlashMessage::into_inner);

    let selected_papers = match Paper::all_selected(&conn).await {
        Ok(papers) => {
            let mut with_roles = Vec::new();
            for paper in papers {
                with_roles.push(build_discussed_with_roles(&conn, paper).await);
            }
            with_roles
        }
        Err(_) => vec![],
    };

    let discussed_papers = match Paper::all_discussed(&conn).await {
        Ok(papers) => {
            let mut with_roles = Vec::new();
            for paper in papers {
                with_roles.push(build_discussed_with_roles(&conn, paper).await);
            }
            with_roles
        }
        Err(_) => vec![],
    };

    let context = ScheduleContext {
        flash,
        selected_papers,
        discussed_papers,
        cookie_info,
    };

    Template::render("schedule", &context)
}

#[get("/add")]
async fn paper_add(jar: &CookieJar<'_>, flash: Option<FlashMessage<'_>>, conn: DbConn) -> Template {
    let cookie_info = read_cookie(jar);
    let flash = flash.map(FlashMessage::into_inner);
    let mut can_submit = false;
    let mut access_message = None;

    match &cookie_info {
        None => {
            access_message = Some("You need to login before proposing new papers.".to_string());
        }
        Some(user) => match Login::get(&conn, user.id).await {
            Ok(login) => {
                if login.is_disabled == 1 {
                    access_message = Some(format!(
                        "Your account is disabled. Please contact {}.",
                        ADMIN_CONTACT_EMAIL
                    ));
                } else if login.is_approved == 0 {
                    access_message = Some(format!(
                        "Your account is pending approval for paper uploads. Please contact {}.",
                        ADMIN_CONTACT_EMAIL
                    ));
                } else {
                    can_submit = true;
                }
            }
            Err(_) => {
                access_message = Some(
                    "Could not validate your account. Please log out and log in again.".to_string(),
                );
            }
        },
    }
    let context = PaperAddContext {
        flash,
        cookie_info,
        can_submit,
        access_message,
    };
    Template::render("paper", &context)
}

#[derive(Debug, FromForm)]
pub struct DiscussForm {
    pub discussed_date: String,
    pub presenter_id: Option<i32>,
}

#[derive(Debug, FromForm)]
pub struct RoleAssignForm {
    pub reviewer_friendly: i32,
    pub reviewer_adversarial: i32,
    pub archaeologist: i32,
    pub futurist: i32,
}

#[derive(Debug, FromForm)]
pub struct UserDisableForm {
    pub disabled: i32,
}

#[derive(Debug, FromForm)]
pub struct UserRoleForm {
    pub role: String,
}

#[derive(Debug, FromForm)]
pub struct PaperEditForm {
    pub title: String,
    pub link: Option<String>,
    pub venue: String,
    pub venue_other: Option<String>,
    pub year: i32,
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

    let login = match Login::get(&conn, cookie_info.id).await {
        Ok(login) => login,
        Err(_) => {
            return Flash::error(
                Redirect::to("/paper/add"),
                "Could not validate your account. Please log in again.",
            )
        }
    };

    if login.is_disabled == 1 {
        return Flash::error(
            Redirect::to("/paper/add"),
            format!(
                "Your account is disabled. Please contact {}.",
                ADMIN_CONTACT_EMAIL
            ),
        );
    }

    if login.is_approved == 0 {
        return Flash::error(
            Redirect::to("/paper/add"),
            format!(
                "Your account is pending approval for paper uploads. Please contact {}.",
                ADMIN_CONTACT_EMAIL
            ),
        );
    }

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

    let has_pdf = paper.pdf.as_ref().map(|pdf| pdf.len() > 0).unwrap_or(false);

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
        return Flash::error(Redirect::to("/paper/add"), "Could not prepare PDF storage.");
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

#[get("/edit/<id>")]
async fn paper_edit(
    jar: &CookieJar<'_>,
    flash: Option<FlashMessage<'_>>,
    conn: DbConn,
    id: i32,
) -> Result<Template, Flash<Redirect>> {
    let cookie_info = read_cookie(jar);
    let flash = flash.map(FlashMessage::into_inner);

    let cookie_info = match cookie_info {
        Some(cookie_info) => cookie_info,
        None => {
            return Err(Flash::error(
                Redirect::to("/"),
                "Please log in before editing papers.",
            ))
        }
    };

    let login = match Login::get(&conn, cookie_info.id).await {
        Ok(login) => login,
        Err(_) => {
            return Err(Flash::error(
                Redirect::to("/"),
                "Could not validate your account. Please log in again.",
            ))
        }
    };

    if login.is_disabled == 1 {
        return Err(Flash::error(
            Redirect::to("/"),
            format!(
                "Your account is disabled. Please contact {}.",
                ADMIN_CONTACT_EMAIL
            ),
        ));
    }

    let paper = match Paper::get(&conn, id).await {
        Ok(paper) => paper,
        Err(_) => return Err(Flash::error(Redirect::to("/"), "Paper not found.")),
    };

    if !can_manage_paper(&login, &paper, cookie_info.id) {
        return Err(Flash::error(
            Redirect::to("/"),
            "Only the proposer or an admin can edit this paper.",
        ));
    }

    let venue_value = paper.venue.clone().unwrap_or_default();
    let (selected_venue, venue_other) = if ALLOWED_VENUES
        .iter()
        .any(|allowed_venue| *allowed_venue == venue_value.as_str())
    {
        (venue_value, String::new())
    } else if venue_value.is_empty() {
        ("Other".to_string(), String::new())
    } else {
        ("Other".to_string(), venue_value)
    };

    let context = PaperEditContext {
        flash,
        cookie_info: Some(cookie_info),
        paper,
        selected_venue,
        venue_other,
    };

    Ok(Template::render("paper_edit", &context))
}

#[post("/edit/<id>", data = "<paper_form>")]
async fn paper_edit_post(
    jar: &CookieJar<'_>,
    conn: DbConn,
    id: i32,
    paper_form: Form<PaperEditForm>,
) -> Flash<Redirect> {
    let cookie_info = read_cookie(jar);
    if cookie_info.is_none() {
        return Flash::error(Redirect::to("/"), "Please log in before editing papers.");
    }
    let cookie_info = cookie_info.unwrap();

    let login = match Login::get(&conn, cookie_info.id).await {
        Ok(login) => login,
        Err(_) => {
            return Flash::error(
                Redirect::to("/"),
                "Could not validate your account. Please log in again.",
            )
        }
    };

    if login.is_disabled == 1 {
        return Flash::error(
            Redirect::to("/"),
            format!(
                "Your account is disabled. Please contact {}.",
                ADMIN_CONTACT_EMAIL
            ),
        );
    }

    let paper = match Paper::get(&conn, id).await {
        Ok(paper) => paper,
        Err(_) => return Flash::error(Redirect::to("/"), "Paper not found."),
    };

    if !can_manage_paper(&login, &paper, cookie_info.id) {
        return Flash::error(
            Redirect::to("/"),
            "Only the proposer or an admin can edit this paper.",
        );
    }

    let mut paper_form = paper_form.into_inner();

    let title = paper_form.title.trim().to_string();
    if title.is_empty() {
        return Flash::error(
            Redirect::to(format!("/paper/edit/{}", id)),
            "Title cannot be empty.",
        );
    }

    let link = paper_form
        .link
        .take()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());

    let selected_venue = paper_form.venue.trim().to_string();
    if selected_venue.is_empty() {
        return Flash::error(
            Redirect::to(format!("/paper/edit/{}", id)),
            "Venue must be selected.",
        );
    }
    let venue = if selected_venue == "Other" {
        let venue_other = paper_form
            .venue_other
            .take()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());

        if venue_other.is_none() {
            return Flash::error(
                Redirect::to(format!("/paper/edit/{}", id)),
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
        return Flash::error(
            Redirect::to(format!("/paper/edit/{}", id)),
            "Invalid venue selection.",
        );
    };

    if !(1900..=2100).contains(&paper_form.year) {
        return Flash::error(
            Redirect::to(format!("/paper/edit/{}", id)),
            "Publication year must be between 1900 and 2100.",
        );
    }
    let publication_year = Some(paper_form.year);

    if let Err(e) = Paper::update_fields(
        &conn,
        id,
        title,
        link.clone().unwrap_or_default(),
        venue,
        publication_year,
    )
    .await
    {
        error_!("paper update error: {}", e);
        Flash::error(
            Redirect::to(format!("/paper/edit/{}", id)),
            "Could not update paper due to an internal error.",
        )
    } else {
        let redirect_target = if login.is_admin == 1 {
            "/paper/admin"
        } else {
            "/"
        };
        Flash::success(Redirect::to(redirect_target), "Paper updated successfully.")
    }
}

#[put("/select/<id>")]
async fn paper_select(jar: &CookieJar<'_>, conn: DbConn, id: i32) -> Flash<Redirect> {
    let cookie_info = read_cookie(jar);
    let admin_login = match &cookie_info {
        Some(user) => Login::get(&conn, user.id).await.ok(),
        None => None,
    };
    if admin_login
        .as_ref()
        .map(|user| user.is_admin == 1 && user.is_disabled == 0)
        .unwrap_or(false)
        == false
    {
        return Flash::error(
            Redirect::to("/"),
            "Only admin users can select a paper for session roles.",
        );
    }

    let paper = match Paper::get(&conn, id).await {
        Ok(paper) => paper,
        Err(_) => return Flash::error(Redirect::to("/paper/admin"), "Paper not found."),
    };

    if paper.readed == 1 || paper.discussed_at.is_some() {
        return Flash::error(
            Redirect::to("/paper/admin"),
            "Cannot select a paper that has already been discussed.",
        );
    }

    match Paper::mark_selected(&conn, id).await {
        Ok(updated_rows) if updated_rows > 0 => Flash::success(
            Redirect::to("/paper/admin"),
            "Paper selected. Assign and review the roles before discussion.",
        ),
        _ => Flash::error(Redirect::to("/paper/admin"), "Could not select paper."),
    }
}

#[put("/roles/<id>", data = "<role_form>")]
async fn paper_assign_roles(
    jar: &CookieJar<'_>,
    conn: DbConn,
    id: i32,
    role_form: Form<RoleAssignForm>,
) -> Flash<Redirect> {
    let cookie_info = read_cookie(jar);
    let admin_login = match &cookie_info {
        Some(user) => Login::get(&conn, user.id).await.ok(),
        None => None,
    };
    if admin_login
        .as_ref()
        .map(|user| user.is_admin == 1 && user.is_disabled == 0)
        .unwrap_or(false)
        == false
    {
        return Flash::error(
            Redirect::to("/"),
            "Only admin users can assign session roles.",
        );
    }

    let paper = match Paper::get(&conn, id).await {
        Ok(paper) => paper,
        Err(_) => return Flash::error(Redirect::to("/paper/admin"), "Paper not found."),
    };

    if paper.is_selected == 0 {
        return Flash::error(
            Redirect::to("/paper/admin"),
            "Roles can only be assigned to selected papers.",
        );
    }

    let form = role_form.into_inner();
    let assignments = vec![
        ("reviewer_friendly".to_string(), form.reviewer_friendly),
        (
            "reviewer_adversarial".to_string(),
            form.reviewer_adversarial,
        ),
        ("archaeologist".to_string(), form.archaeologist),
        ("futurist".to_string(), form.futurist),
    ];

    let mut seen_users = std::collections::HashSet::new();
    for (_, user_id) in &assignments {
        if *user_id == paper.user_id {
            return Flash::error(
                Redirect::to("/paper/admin"),
                "The proposer already has the Investigator role and cannot be assigned another role.",
            );
        }
        if !seen_users.insert(*user_id) {
            return Flash::error(
                Redirect::to("/paper/admin"),
                "Each role must be assigned to a different user.",
            );
        }

        let login = match Login::get(&conn, *user_id).await {
            Ok(login) => login,
            Err(_) => {
                return Flash::error(
                    Redirect::to("/paper/admin"),
                    "One of the selected users does not exist.",
                )
            }
        };
        if login.is_approved == 0 || login.is_disabled == 1 {
            return Flash::error(
                Redirect::to("/paper/admin"),
                "Roles can only be assigned to approved and enabled users.",
            );
        }
    }

    match PaperRole::assign(&conn, id, assignments).await {
        Ok(_) => Flash::success(
            Redirect::to("/paper/admin"),
            "Role assignments saved for selected paper.",
        ),
        Err(_) => Flash::error(
            Redirect::to("/paper/admin"),
            "Could not save role assignments.",
        ),
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
    let admin_login = match &cookie_info {
        Some(user) => Login::get(&conn, user.id).await.ok(),
        None => None,
    };
    if admin_login
        .as_ref()
        .map(|user| user.is_admin == 1 && user.is_disabled == 0)
        .unwrap_or(false)
        == false
    {
        return Flash::error(
            Redirect::to("/"),
            "Only admin users can mark a paper as discussed.",
        );
    }

    let discuss_form = discuss_form.into_inner();
    let discussed_date = discuss_form.discussed_date.trim().to_string();

    let paper = match Paper::get(&conn, id).await {
        Ok(paper) => paper,
        Err(_) => return Flash::error(Redirect::to("/paper/admin"), "Paper not found."),
    };

    if paper.is_selected == 1 {
        let role_rows = PaperRole::for_paper(&conn, id).await.unwrap_or_default();
        if role_rows.len() < SESSION_ROLES.len() {
            return Flash::error(
                Redirect::to("/paper/admin"),
                "Please assign all selected-paper roles before marking discussed.",
            );
        }
    }

    let presenter_id = discuss_form.presenter_id.or(Some(paper.user_id));

    if discussed_date.is_empty() {
        return Flash::error(
            Redirect::to("/paper/admin"),
            "Please provide the date when the paper was discussed.",
        );
    }

    match Paper::mark_discussed(&conn, id, discussed_date, presenter_id).await {
        Ok(updated_rows) if updated_rows > 0 => {
            Flash::success(Redirect::to("/paper/admin"), "Paper marked as discussed.")
        }
        _ => Flash::error(
            Redirect::to("/paper/admin"),
            "Could not mark paper as discussed.",
        ),
    }
}

#[put("/approve/<id>")]
async fn user_approve(jar: &CookieJar<'_>, conn: DbConn, id: i32) -> Flash<Redirect> {
    let cookie_info = read_cookie(jar);
    let admin_login = match &cookie_info {
        Some(user) => Login::get(&conn, user.id).await.ok(),
        None => None,
    };
    if admin_login
        .as_ref()
        .map(|user| user.is_admin == 1 && user.is_disabled == 0)
        .unwrap_or(false)
        == false
    {
        return Flash::error(
            Redirect::to("/"),
            "Only admin users can approve user accounts.",
        );
    }

    match Login::approve(&conn, id).await {
        Ok(updated_rows) if updated_rows > 0 => {
            Flash::success(Redirect::to("/paper/admin"), "User approved successfully.")
        }
        _ => Flash::error(
            Redirect::to("/paper/admin"),
            "Could not approve user account.",
        ),
    }
}

#[put("/disable/<id>", data = "<disable_form>")]
async fn user_set_disabled(
    jar: &CookieJar<'_>,
    conn: DbConn,
    id: i32,
    disable_form: Form<UserDisableForm>,
) -> Flash<Redirect> {
    let cookie_info = read_cookie(jar);
    let admin_login = match &cookie_info {
        Some(user) => Login::get(&conn, user.id).await.ok(),
        None => None,
    };
    if admin_login
        .as_ref()
        .map(|user| user.is_admin == 1 && user.is_disabled == 0)
        .unwrap_or(false)
        == false
    {
        return Flash::error(
            Redirect::to("/"),
            "Only admin users can disable or enable user accounts.",
        );
    }

    let cookie_info = cookie_info.unwrap();
    let disabled = if disable_form.into_inner().disabled == 1 {
        1
    } else {
        0
    };

    if disabled == 1 && id == cookie_info.id {
        return Flash::error(
            Redirect::to("/paper/admin"),
            "You cannot disable your own account.",
        );
    }

    match Login::set_disabled(&conn, id, disabled).await {
        Ok(updated_rows) if updated_rows > 0 => {
            if disabled == 1 {
                Flash::success(Redirect::to("/paper/admin"), "User disabled successfully.")
            } else {
                Flash::success(Redirect::to("/paper/admin"), "User enabled successfully.")
            }
        }
        _ => Flash::error(
            Redirect::to("/paper/admin"),
            "Could not update user status.",
        ),
    }
}

#[put("/role/<id>", data = "<role_form>")]
async fn user_set_role(
    jar: &CookieJar<'_>,
    conn: DbConn,
    id: i32,
    role_form: Form<UserRoleForm>,
) -> Flash<Redirect> {
    let cookie_info = read_cookie(jar);
    let admin_login = match &cookie_info {
        Some(user) => Login::get(&conn, user.id).await.ok(),
        None => None,
    };
    if admin_login
        .as_ref()
        .map(|user| user.is_admin == 1 && user.is_disabled == 0)
        .unwrap_or(false)
        == false
    {
        return Flash::error(Redirect::to("/"), "Only admin users can edit user roles.");
    }

    let role = match normalize_role(&role_form.into_inner().role) {
        Some(role) => role,
        None => {
            return Flash::error(
                Redirect::to("/paper/admin"),
                "Invalid role value. Allowed roles: master student, PhD student, prof, other.",
            )
        }
    };

    match Login::set_role(&conn, id, role).await {
        Ok(updated_rows) if updated_rows > 0 => Flash::success(
            Redirect::to("/paper/admin"),
            "User role updated successfully.",
        ),
        _ => Flash::error(Redirect::to("/paper/admin"), "Could not update user role."),
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

    let login = match Login::get(&conn, cookie_info.id).await {
        Ok(login) => login,
        Err(_) => {
            return Flash::error(
                Redirect::to("/"),
                "Could not validate your account. Please log in again.",
            )
        }
    };
    if login.is_disabled == 1 {
        return Flash::error(
            Redirect::to("/"),
            format!(
                "Your account is disabled. Please contact {}.",
                ADMIN_CONTACT_EMAIL
            ),
        );
    }

    let redirect_target = if login.is_admin == 1 {
        "/paper/admin"
    } else {
        "/"
    };

    match Paper::get(&conn, id).await {
        Err(e) => {
            error_!("remove paper: {}", e);
            Flash::error(Redirect::to("/"), "Impossible to retrive paper")
        }
        Ok(paper) => {
            if !can_manage_paper(&login, &paper, cookie_info.id) {
                return Flash::error(
                    Redirect::to(redirect_target),
                    "Only the proposer or an admin can remove this paper.",
                );
            }

            let pdf_file = paper.pdf_file.clone();
            let thumbnail = paper.thumbnail.clone();
            let _ = Vote::remove_for_paper(&conn, id).await;
            let _ = PaperRole::remove_for_paper(&conn, id).await;
            if let Err(e) = Paper::remove(&conn, id).await {
                error_!("remove paper db error: {}", e);
                return Flash::error(Redirect::to(redirect_target), "Could not remove paper.");
            }
            if let Some(pdf_file) = pdf_file {
                let _ = fs::remove_file(format!("static/pdfs/{}", pdf_file));
            }
            if let Some(thumbnail) = thumbnail {
                let _ = fs::remove_file(format!("static/thumbnails/{}", thumbnail));
            }

            Flash::success(
                Redirect::to(redirect_target),
                format!("Paper removed: {}", id),
            )
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

    let login = match Login::get(&conn, cookie_info.id).await {
        Ok(login) => login,
        Err(_) => {
            return Flash::error(
                Redirect::to("/"),
                "Could not validate your account. Please log in again.",
            )
        }
    };
    if login.is_disabled == 1 {
        return Flash::error(
            Redirect::to("/"),
            format!(
                "Your account is disabled. Please contact {}.",
                ADMIN_CONTACT_EMAIL
            ),
        );
    }

    let res = Vote::up(&conn, cookie_info.id, id).await;
    if res {
        Flash::success(Redirect::to("/"), "Marked as interested.")
    } else {
        Flash::error(
            Redirect::to("/"),
            "You already marked this paper as interested.",
        )
    }
}
#[put("/down/<id>")]
async fn paper_vote_down(jar: &CookieJar<'_>, conn: DbConn, id: i32) -> Flash<Redirect> {
    let cookie_info = read_cookie(jar);
    if cookie_info.is_none() {
        return Flash::error(Redirect::to("/"), "Impossible to vote without login");
    }
    let cookie_info = cookie_info.unwrap();

    let login = match Login::get(&conn, cookie_info.id).await {
        Ok(login) => login,
        Err(_) => {
            return Flash::error(
                Redirect::to("/"),
                "Could not validate your account. Please log in again.",
            )
        }
    };
    if login.is_disabled == 1 {
        return Flash::error(
            Redirect::to("/"),
            format!(
                "Your account is disabled. Please contact {}.",
                ADMIN_CONTACT_EMAIL
            ),
        );
    }

    let res = Vote::down(&conn, cookie_info.id, id).await;
    if res {
        Flash::success(Redirect::to("/"), "Marked as ignored.")
    } else {
        Flash::error(
            Redirect::to("/"),
            "You already marked this paper as ignored.",
        )
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
                        if l.is_disabled == 1 {
                            return Flash::error(
                                Redirect::to("/"),
                                format!(
                                    "Your account is disabled. Please contact {}.",
                                    ADMIN_CONTACT_EMAIL
                                ),
                            );
                        }
                        let user_id = l.id.unwrap();
                        if let Err(e) =
                            Login::update_last_connected(&conn, user_id, Utc::now().to_rfc3339())
                                .await
                        {
                            error_!("update_last_connected error: {}", e);
                        }
                        jar.add_private(Cookie::new("user_id", user_id.to_string()));
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
    pub role: String,
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
    let name = register.name.trim().to_string();
    let email = register.email.trim().to_string();
    let role = match normalize_role(&register.role) {
        Some(role) => role,
        None => {
            return Flash::error(
                Redirect::to("/"),
                "Role must be one of master student, PhD student, prof, or other.",
            )
        }
    };

    let register = RegisterForm {
        name,
        email,
        password: register.password,
        role,
    };

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
        Flash::success(
            Redirect::to("/"),
            format!(
                "User successfully added. Please contact {} so an admin can approve your account for paper uploads.",
                ADMIN_CONTACT_EMAIL
            ),
        )
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
                paper_edit,
                paper_edit_post,
                paper_remove,
                paper_vote_up,
                paper_vote_down,
                paper_ranking,
                paper_schedule,
                paper_admin_discussed,
                paper_select,
                paper_assign_roles,
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
                user_approve,
                user_set_disabled,
                user_set_role,
                user_logout
            ],
        )
}
