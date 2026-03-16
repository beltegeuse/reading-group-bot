use std::fs;
use std::path::Path;

use rocket::serde::Deserialize;

use crate::model::Login;
use crate::{DbConn, RegisterForm};

const DEFAULT_USER_CONFIG_PATH: &str = "local/default_user.json";

#[derive(Debug, Deserialize)]
#[serde(crate = "rocket::serde")]
struct DefaultUserConfig {
    name: String,
    email: String,
    password: String,
    #[serde(default = "default_is_admin")]
    is_admin: bool,
    #[serde(default = "default_role")]
    role: String,
}

fn default_is_admin() -> bool {
    true
}

fn default_role() -> String {
    "other".to_string()
}

fn normalize_role(role: &str) -> String {
    match role.trim().to_lowercase().as_str() {
        "master_student" | "master student" => "master_student".to_string(),
        "phd_student" | "phd student" | "phd students" => "phd_student".to_string(),
        "prof" | "profs" | "professor" | "professors" => "prof".to_string(),
        _ => "other".to_string(),
    }
}

fn read_default_user_config() -> Option<DefaultUserConfig> {
    let path = Path::new(DEFAULT_USER_CONFIG_PATH);
    if !path.exists() {
        warn_!(
            "Default user config {} not found. Skipping default user seed.",
            DEFAULT_USER_CONFIG_PATH
        );
        return None;
    }

    let content = match fs::read_to_string(path) {
        Ok(content) => content,
        Err(e) => {
            error_!(
                "Failed to read default user config {}: {}",
                DEFAULT_USER_CONFIG_PATH,
                e
            );
            return None;
        }
    };

    match serde_json::from_str::<DefaultUserConfig>(&content) {
        Ok(config) => Some(config),
        Err(e) => {
            error_!(
                "Failed to parse default user config {}: {}",
                DEFAULT_USER_CONFIG_PATH,
                e
            );
            None
        }
    }
}

pub async fn seed_default_user(conn: &DbConn) {
    let default_user = match read_default_user_config() {
        Some(default_user) => default_user,
        None => return,
    };

    if default_user.name.trim().is_empty()
        || default_user.email.trim().is_empty()
        || default_user.password.is_empty()
    {
        error_!(
            "Default user config {} must define non-empty name, email and password",
            DEFAULT_USER_CONFIG_PATH
        );
        return;
    }

    match Login::all(conn).await {
        Err(e) => {
            error_!("Failed to load users before seeding default user: {}", e);
        }
        Ok(logins) => {
            if !logins.is_empty() {
                info_!(
                    "Users already exist. Skipping default user seed from {}",
                    DEFAULT_USER_CONFIG_PATH
                );
                return;
            }

            let default_name = default_user.name.clone();
            let default_email = default_user.email.clone();

            let register = RegisterForm {
                name: default_user.name,
                email: default_user.email,
                password: default_user.password,
                role: if default_user.is_admin {
                    "prof".to_string()
                } else {
                    normalize_role(&default_user.role)
                },
            };
            if let Err(e) = Login::insert(register, conn).await {
                error_!(
                    "Failed to insert default user from {}: {}",
                    DEFAULT_USER_CONFIG_PATH,
                    e
                );
            } else {
                info_!("Inserted default user from {}", DEFAULT_USER_CONFIG_PATH);
                if default_user.is_admin {
                    match Login::all(conn).await {
                        Ok(logins) => {
                            let inserted = logins.into_iter().find(|login| {
                                login.name == default_name.as_str()
                                    || login.email == default_email.as_str()
                            });
                            if let Some(user) = inserted {
                                if let Some(user_id) = user.id {
                                    let _ = Login::promote_to_admin(conn, user_id).await;
                                }
                            }
                        }
                        Err(e) => {
                            error_!("Failed to reload users after default user insert: {}", e);
                        }
                    }
                }
            }
        }
    }
}
