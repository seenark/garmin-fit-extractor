use std::{collections::HashSet, env, net::SocketAddr, path::PathBuf, str::FromStr};

use sqlx::postgres::PgConnectOptions;
use thiserror::Error;
use url::Url;

pub const DEFAULT_BIND: &str = "0.0.0.0:3000";
pub const DEFAULT_DATABASE_URL: &str = "postgresql://localhost/garmin_fit_extractor";
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
    pub app_origin: Option<String>,
    pub google: Option<GoogleConfig>,
    pub coach_oauth: Option<CoachOAuthConfig>,
    pub admin_emails: HashSet<String>,
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("GARMIN_FIT_BIND must be a socket address: {0}")]
    InvalidBind(String),
    #[error("GARMIN_FIT_APP_ORIGIN must be an HTTP(S) origin: {0}")]
    InvalidAppOrigin(String),
    #[error("DATABASE_URL must be a valid PostgreSQL URL: {0}")]
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
        let database_url = nonempty_env("DATABASE_URL")
            .or_else(|| nonempty_env("GARMIN_FIT_DATABASE_URL"))
            .unwrap_or_else(|| DEFAULT_DATABASE_URL.to_owned());
        let static_dir = env::var("GARMIN_FIT_STATIC_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from(DEFAULT_STATIC_DIR));
        let app_origin = nonempty_env("GARMIN_FIT_APP_ORIGIN")
            .map(|value| {
                parse_app_origin(&value).map_err(|_| ConfigError::InvalidAppOrigin(value.clone()))
            })
            .transpose()?;
        let bind = bind
            .parse()
            .map_err(|_| ConfigError::InvalidBind(bind.clone()))?;
        validate_database_url(&database_url)
            .map_err(|_| ConfigError::InvalidDatabaseUrl(database_url.clone()))?;
        let admin_value = env::var("ADMIN_EMAILS").ok();
        let admin_emails = parse_admin_emails(admin_value.as_deref());

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
            admin_emails,
            app_origin,
        })
    }
}

fn nonempty_env(name: &str) -> Option<String> {
    env::var(name).ok().filter(|value| !value.trim().is_empty())
}

fn parse_app_origin(value: &str) -> Result<String, ()> {
    let value = value.trim();
    let parsed = Url::parse(value).map_err(|_| ())?;
    if !matches!(parsed.scheme(), "http" | "https")
        || parsed.host_str().is_none()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || !matches!(parsed.path(), "" | "/")
        || parsed.query().is_some()
        || parsed.fragment().is_some()
        || !parsed.origin().is_tuple()
    {
        return Err(());
    }
    Ok(parsed.origin().ascii_serialization())
}

fn validate_database_url(value: &str) -> Result<(), ()> {
    let parsed = url::Url::parse(value).map_err(|_| ())?;
    if !matches!(parsed.scheme(), "postgres" | "postgresql") {
        return Err(());
    }
    PgConnectOptions::from_str(value).map_err(|_| ())?;
    Ok(())
}

fn parse_admin_emails(value: Option<&str>) -> HashSet<String> {
    value
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|email| !email.is_empty())
        .map(str::to_ascii_lowercase)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{parse_admin_emails, parse_app_origin, validate_database_url};

    #[test]
    fn accepts_postgres_url_schemes_only() {
        assert!(validate_database_url("postgres://user:pass@localhost/db").is_ok());
        assert!(validate_database_url("postgresql://user:pass@localhost/db").is_ok());
        assert!(validate_database_url("sqlite://data/test.sqlite").is_err());
        assert!(validate_database_url("mysql://user:pass@localhost/db").is_err());
        assert!(validate_database_url("not a database url").is_err());
    }

    #[test]
    fn parses_admin_emails_as_trimmed_ascii_lowercase_allowlist() {
        assert_eq!(
            parse_admin_emails(Some(" Admin@Example.COM,second@example.com ")),
            ["admin@example.com", "second@example.com"]
                .into_iter()
                .map(str::to_owned)
                .collect()
        );
        assert!(parse_admin_emails(None).is_empty());
        assert!(parse_admin_emails(Some(" ,  ")).is_empty());
    }

    #[test]
    fn validates_app_origin_and_normalizes_url_origin() {
        assert_eq!(
            parse_app_origin(" https://fit.example.test/ ").unwrap(),
            "https://fit.example.test"
        );
        assert_eq!(
            parse_app_origin("https://fit.example.test:443").unwrap(),
            "https://fit.example.test"
        );
        assert_eq!(
            parse_app_origin("http://127.0.0.1:5173").unwrap(),
            "http://127.0.0.1:5173"
        );
        for value in [
            "fit.example.test",
            "ftp://fit.example.test",
            "https://fit.example.test/path",
            "https://fit.example.test?query",
            "https://user:password@fit.example.test",
        ] {
            assert!(
                parse_app_origin(value).is_err(),
                "expected invalid origin: {value}"
            );
        }
    }
}
