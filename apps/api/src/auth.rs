use axum::{
    extract::FromRequestParts,
    http::{header, request::Parts},
};
use axum_extra::extract::CookieJar;
use openidconnect::{
    ClientId, ClientSecret, EndpointMaybeSet, EndpointNotSet, EndpointSet, IssuerUrl, RedirectUrl,
    core::{CoreClient, CoreProviderMetadata},
};
use reqwest::Client;
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use std::{collections::HashSet, time::Duration as StdDuration};
use tokio::sync::OnceCell;
use uuid::Uuid;

use crate::{
    app::AppState,
    config::{CoachOAuthConfig, GoogleConfig},
    db,
    error::ApiError,
    model::UserProfile,
};

type GoogleClient = CoreClient<
    EndpointSet,
    EndpointNotSet,
    EndpointNotSet,
    EndpointNotSet,
    EndpointMaybeSet,
    EndpointMaybeSet,
>;

pub const SESSION_COOKIE: &str = "garmin_fit_session";
pub const SESSION_TTL_SECONDS: i64 = 30 * 24 * 60 * 60;

pub struct AuthState {
    pub google: Option<GoogleConfig>,
    pub coach_oauth: Option<CoachOAuthConfig>,
    pub client: Client,
    pub oidc_client: OnceCell<GoogleClient>,
    pub admin_emails: HashSet<String>,
}

impl AuthState {
    pub fn new(google: Option<GoogleConfig>, coach_oauth: Option<CoachOAuthConfig>) -> Self {
        let client = Client::builder()
            .connect_timeout(StdDuration::from_secs(5))
            .timeout(StdDuration::from_secs(15))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .expect("bounded redirect-disabled HTTP client should build");
        Self {
            google,
            coach_oauth,
            client,
            oidc_client: OnceCell::new(),
            admin_emails: HashSet::new(),
        }
    }

    pub fn with_admin_emails<I>(mut self, emails: I) -> Self
    where
        I: IntoIterator<Item = String>,
    {
        self.admin_emails = emails
            .into_iter()
            .map(|email| email.trim().to_ascii_lowercase())
            .filter(|email| !email.is_empty())
            .collect();
        self
    }

    pub fn is_admin_email(&self, email: &str) -> bool {
        self.admin_emails
            .contains(&email.trim().to_ascii_lowercase())
    }

    pub async fn oidc_client(&self) -> Result<&GoogleClient, ApiError> {
        let Some(google) = self.google.clone() else {
            return Err(ApiError::auth_not_configured());
        };
        self.oidc_client
            .get_or_try_init(|| async move {
                discover_client(google, self.client.clone())
                    .await
                    .map_err(|error| error.to_owned())
            })
            .await
            .map_err(|_| ApiError::auth_provider_unavailable())
    }
}

async fn discover_client(
    google: GoogleConfig,
    client: Client,
) -> Result<GoogleClient, &'static str> {
    let issuer = IssuerUrl::new("https://accounts.google.com".to_owned())
        .map_err(|_| "invalid Google issuer URL")?;
    let metadata = CoreProviderMetadata::discover_async(issuer, &client)
        .await
        .map_err(|_| "Google provider discovery failed")?;
    let redirect_uri =
        RedirectUrl::new(google.redirect_uri).map_err(|_| "invalid Google redirect URI")?;
    Ok(CoreClient::from_provider_metadata(
        metadata,
        ClientId::new(google.client_id),
        Some(ClientSecret::new(google.client_secret)),
    )
    .set_redirect_uri(redirect_uri))
}

#[derive(Clone, Debug)]
pub struct AuthenticatedUser {
    pub user_id: Uuid,
    pub email: String,
    pub display_name: Option<String>,
}

impl FromRequestParts<AppState> for AuthenticatedUser {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let jar = CookieJar::from_headers(&parts.headers);
        let Some(token) = jar
            .get(SESSION_COOKIE)
            .map(|cookie| cookie.value().to_owned())
        else {
            return Err(ApiError::auth_required());
        };

        let Some(user) = session_user(&state.db, &token).await? else {
            return Err(ApiError::auth_required());
        };
        Ok(Self {
            user_id: user.id,
            email: user.email,
            display_name: user.display_name,
        })
    }
}
pub const COACH_ACTIVITY_SCOPE: &str = "activities:read";

#[derive(Clone, Debug)]
pub struct CoachAuthenticatedUser {
    pub user_id: Uuid,
}

impl FromRequestParts<AppState> for CoachAuthenticatedUser {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let Some(coach) = state.auth.coach_oauth.as_ref() else {
            return Err(ApiError::coach_not_configured());
        };
        let Some(value) = parts.headers.get(header::AUTHORIZATION) else {
            return Err(ApiError::coach_authentication_failed());
        };
        let value = value
            .to_str()
            .map_err(|_| ApiError::coach_authentication_failed())?;
        let mut pieces = value.split_ascii_whitespace();
        let (Some(scheme), Some(token), None) = (pieces.next(), pieces.next(), pieces.next())
        else {
            return Err(ApiError::coach_authentication_failed());
        };
        if scheme != "Bearer" || token.is_empty() {
            return Err(ApiError::coach_authentication_failed());
        }
        let stored = db::find_access_token(&state.db, token)
            .await
            .map_err(|_| ApiError::coach_processing_error())?
            .ok_or_else(ApiError::coach_authentication_failed)?;
        if stored.client_id != coach.client_id {
            return Err(ApiError::coach_authentication_failed());
        }
        if !stored
            .scope
            .split_ascii_whitespace()
            .any(|scope| scope == COACH_ACTIVITY_SCOPE)
        {
            return Err(ApiError::coach_insufficient_scope());
        }
        Ok(Self {
            user_id: stored.user_id,
        })
    }
}

pub async fn session_user(pool: &PgPool, token: &str) -> Result<Option<UserProfile>, ApiError> {
    let now = db::timestamp_now();
    sqlx::query(
        "DELETE FROM sessions
         WHERE expires_at <= $1",
    )
    .bind(&now)
    .execute(pool)
    .await
    .map_err(|_| ApiError::service_unavailable())?;

    let row = sqlx::query_as::<_, (String, String, Option<String>)>(
        "SELECT u.id, u.email, u.display_name
         FROM sessions AS s
         JOIN users AS u ON u.id = s.user_id
         WHERE s.token_hash = $1
           AND s.expires_at > $2",
    )
    .bind(hash_token(token))
    .bind(&now)
    .fetch_optional(pool)
    .await
    .map_err(|_| ApiError::service_unavailable())?;

    row.map(|(id, email, display_name)| {
        Uuid::parse_str(&id)
            .map(|id| UserProfile {
                id,
                email,
                display_name,
            })
            .map_err(|_| ApiError::service_unavailable())
    })
    .transpose()
}

pub fn hash_token(value: &str) -> String {
    Sha256::digest(value.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::AuthState;

    #[test]
    fn admin_allowlist_matches_trimmed_ascii_lowercase_emails() {
        let auth = AuthState::new(None, None).with_admin_emails([" ADMIN@Example.COM ".to_owned()]);
        assert!(auth.is_admin_email("admin@example.com"));
        assert!(auth.is_admin_email(" ADMIN@EXAMPLE.COM "));
        assert!(!auth.is_admin_email("other@example.com"));
        assert!(!AuthState::new(None, None).is_admin_email("admin@example.com"));
    }
}
