use std::{env, net::SocketAddr, path::PathBuf, str::FromStr};

use sqlx::sqlite::SqliteConnectOptions;
use thiserror::Error;

pub const DEFAULT_BIND: &str = "0.0.0.0:3000";
pub const DEFAULT_DATABASE_URL: &str = "sqlite://data/garmin-fit-extractor.sqlite3";
pub const DEFAULT_STATIC_DIR: &str = "apps/web/dist";

#[derive(Debug, Clone)]
pub struct Config {
    pub bind: SocketAddr,
    pub database_url: String,
    pub static_dir: PathBuf,
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("GARMIN_FIT_BIND must be a socket address: {0}")]
    InvalidBind(String),
    #[error("GARMIN_FIT_DATABASE_URL must be a valid SQLite URL: {0}")]
    InvalidDatabaseUrl(String),
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

        Ok(Self {
            bind,
            database_url,
            static_dir,
        })
    }
}
