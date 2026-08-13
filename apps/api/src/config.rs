use std::{env, net::SocketAddr, path::PathBuf, str::FromStr};

use sqlx::sqlite::SqliteConnectOptions;
use thiserror::Error;

pub const DEFAULT_BIND: &str = "0.0.0.0:3000";
pub const DEFAULT_DATABASE_URL: &str = "sqlite://data/garmin-fit-extractor.sqlite3";
pub const DEFAULT_STATIC_DIR: &str = "apps/web/dist";

#[derive(Debug, Clone)]
pub struct GoogleConfig {
    pub client_id: String,
    pub client_secret: String,
    pub redirect_uri: String,
}

#[derive(Debug, Clone)]
pub struct CoachOAuthConfig {
    pub client_id: String,
    pub client_secret: String,
    pub redirect_uri: String,
}

pub const COACH_CLIENT_ID: &str = "FIT_COACH_CHATGPT";

#[derive(Debug, Clone)]
pub struct Config {
    pub bind: SocketAddr,
    pub database_url: String,
    pub static_dir: PathBuf,
    pub google: Option<GoogleConfig>,
    pub coach_oauth: Option<CoachOAuthConfig>,
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("GARMIN_FIT_BIND must be a socket address: {0}")]
    InvalidBind(String),
    #[error("GARMIN_FIT_DATABASE_URL must be a valid SQLite URL: {0}")]
    InvalidDatabaseUrl(String),
    #[error("Google OAuth configuration must set all three variables")]
    InvalidGoogleConfig,
    #[error(
        "ChatGPT OAuth configuration must set all three variables and use client ID FIT_COACH_CHATGPT"
    )]
    InvalidCoachOAuthConfig,
}

impl Config {
    pub fn from_env() -> Result<Self, ConfigError> {
        let bind = env::var("GARMIN_FIT_BIND").unwrap_or_else(|_| DEFAULT_BIND.to_owned());
        let database_url =
            env::var("GARMIN_FIT_DATABASE_URL").unwrap_or_else(|_| DEFAULT_DATABASE_URL.to_owned());
        let static_dir = env::var("GARMIN_FIT_STATIC_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from(DEFAULT_STATIC_DIR));

        let bind = bind
            .parse()
            .map_err(|_| ConfigError::InvalidBind(bind.clone()))?;
        SqliteConnectOptions::from_str(&database_url)
            .map_err(|_| ConfigError::InvalidDatabaseUrl(database_url.clone()))?;

        let google_values = [
            nonempty_env("GARMIN_FIT_GOOGLE_CLIENT_ID"),
            nonempty_env("GARMIN_FIT_GOOGLE_CLIENT_SECRET"),
            nonempty_env("GARMIN_FIT_GOOGLE_REDIRECT_URI"),
        ];
        let google = match google_values {
            [None, None, None] => None,
            [Some(client_id), Some(client_secret), Some(redirect_uri)] => Some(GoogleConfig {
                client_id,
                client_secret,
                redirect_uri,
            }),
            _ => return Err(ConfigError::InvalidGoogleConfig),
        };
        let coach_values = [
            nonempty_env("GARMIN_FIT_CHATGPT_CLIENT_ID"),
            nonempty_env("GARMIN_FIT_CHATGPT_CLIENT_SECRET"),
            nonempty_env("GARMIN_FIT_CHATGPT_REDIRECT_URI"),
        ];
        let coach_oauth = match coach_values {
            [None, None, None] => None,
            [Some(client_id), Some(client_secret), Some(redirect_uri)]
                if client_id == COACH_CLIENT_ID =>
            {
                Some(CoachOAuthConfig {
                    client_id,
                    client_secret,
                    redirect_uri,
                })
            }
            _ => return Err(ConfigError::InvalidCoachOAuthConfig),
        };

        Ok(Self {
            bind,
            database_url,
            static_dir,
            google,
            coach_oauth,
        })
    }
}

fn nonempty_env(name: &str) -> Option<String> {
    env::var(name).ok().filter(|value| !value.trim().is_empty())
}
