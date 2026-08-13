use axum::{
    Router,
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Json, Redirect, Response},
    routing::{get, post},
};
use axum_extra::extract::{
    CookieJar,
    cookie::{Cookie, SameSite},
};
use openidconnect::{
    AuthorizationCode, CsrfToken, Nonce, PkceCodeChallenge, PkceCodeVerifier, Scope, TokenResponse,
    core::CoreAuthenticationFlow,
};
use serde::Deserialize;
use time::{Duration, OffsetDateTime, format_description::FormatItem, macros::format_description};
use uuid::Uuid;

use crate::{
    app::AppState,
    auth::{SESSION_COOKIE, SESSION_TTL_SECONDS, hash_token, session_user},
    db,
    error::ApiError,
    model::CurrentUserResponse,
};
use url::form_urlencoded;

const OAUTH_STATE_TTL_SECONDS: i64 = 10 * 60;
const TIMESTAMP_FORMAT: &[FormatItem<'static>] =
    format_description!("[year]-[month]-[day]T[hour]:[minute]:[second].[subsecond digits:3]Z");

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/auth/login", get(login))
        .route("/api/v1/auth/callback", get(callback))
        .route("/api/v1/auth/me", get(me))
        .route("/api/v1/auth/logout", post(logout))
        .route("/api/v1/auth/test-login", get(test_login))
}
#[derive(Debug, Deserialize)]
struct LoginQuery {
    #[serde(rename = "continue")]
    continue_token: Option<String>,
}

async fn login(
    State(state): State<AppState>,
    Query(query): Query<LoginQuery>,
) -> Result<Redirect, ApiError> {
    let client = state.auth.oidc_client().await?;
    let (challenge, verifier) = PkceCodeChallenge::new_random_sha256();
    let (authorization_url, csrf_state, nonce) = client
        .authorize_url(
            CoreAuthenticationFlow::AuthorizationCode,
            CsrfToken::new_random,
            Nonce::new_random,
        )
        .add_scope(Scope::new("openid".to_owned()))
        .add_scope(Scope::new("email".to_owned()))
        .add_scope(Scope::new("profile".to_owned()))
        .set_pkce_challenge(challenge)
        .url();

    let continue_path = query.continue_token.map(|token| {
        let query = form_urlencoded::Serializer::new(String::new())
            .append_pair("resume", &token)
            .finish();
        format!("/oauth/authorize?{query}")
    });
    sqlx::query(
        "INSERT INTO oauth_states
            (state_hash, nonce, pkce_verifier, continue_path, created_at, expires_at)
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(hash_token(csrf_state.secret()))
    .bind(nonce.secret())
    .bind(verifier.secret())
    .bind(continue_path)
    .bind(timestamp_now())
    .bind(timestamp_after(OAUTH_STATE_TTL_SECONDS))
    .execute(&state.db)
    .await
    .map_err(|_| ApiError::service_unavailable())?;

    Ok(Redirect::to(authorization_url.as_str()))
}

#[derive(Debug, Deserialize)]
struct CallbackQuery {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
}

#[derive(Debug)]
struct CallbackFailure {
    error: ApiError,
    resume_token: Option<String>,
    oauth_error: &'static str,
}

impl CallbackFailure {
    fn ordinary(error: ApiError) -> Self {
        Self {
            error,
            resume_token: None,
            oauth_error: "server_error",
        }
    }

    fn with_resume(
        error: ApiError,
        resume_token: Option<String>,
        oauth_error: &'static str,
    ) -> Self {
        Self {
            error,
            resume_token,
            oauth_error,
        }
    }
}

async fn callback(State(state): State<AppState>, Query(query): Query<CallbackQuery>) -> Response {
    match callback_inner(&state, query).await {
        Ok(response) => response.into_response(),
        Err(failure) => {
            tracing::warn!(code = failure.error.code(), "Google callback failed");
            if let Some(resume_token) = failure.resume_token
                && let Ok(Some(pending)) =
                    db::consume_oauth_login_request(&state.db, &resume_token).await
                && let Some(response) = oauth_failure_redirect(
                    &pending.redirect_uri,
                    failure.oauth_error,
                    &pending.state,
                )
            {
                return response.into_response();
            }
            Redirect::to("/?authError=AUTH_FAILED").into_response()
        }
    }
}

async fn callback_inner(
    state: &AppState,
    query: CallbackQuery,
) -> Result<(CookieJar, Redirect), CallbackFailure> {
    let state_value = query
        .state
        .filter(|value| !value.is_empty())
        .ok_or_else(|| CallbackFailure::ordinary(ApiError::auth_required()))?;
    let Some((nonce, pkce_verifier, continue_path)) = consume_oauth_state(&state.db, &state_value)
        .await
        .map_err(|_| CallbackFailure::ordinary(ApiError::service_unavailable()))?
    else {
        return Err(CallbackFailure::ordinary(ApiError::auth_required()));
    };
    let resume_token = continue_path.as_deref().and_then(parse_resume_token);
    let oauth_error = if query.error.is_some() {
        "access_denied"
    } else {
        "server_error"
    };
    if query.error.is_some() {
        return Err(CallbackFailure::with_resume(
            ApiError::auth_required(),
            resume_token,
            "access_denied",
        ));
    }
    let code = query
        .code
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            CallbackFailure::with_resume(
                ApiError::auth_required(),
                resume_token.clone(),
                oauth_error,
            )
        })?;

    let client =
        state.auth.oidc_client().await.map_err(|error| {
            CallbackFailure::with_resume(error, resume_token.clone(), oauth_error)
        })?;
    let token_response = client
        .exchange_code(AuthorizationCode::new(code))
        .map_err(|_| {
            CallbackFailure::with_resume(
                ApiError::auth_provider_unavailable(),
                resume_token.clone(),
                oauth_error,
            )
        })?
        .set_pkce_verifier(PkceCodeVerifier::new(pkce_verifier))
        .request_async(&state.auth.client)
        .await
        .map_err(|_| {
            CallbackFailure::with_resume(
                ApiError::auth_provider_unavailable(),
                resume_token.clone(),
                oauth_error,
            )
        })?;
    let id_token = token_response.id_token().ok_or_else(|| {
        CallbackFailure::with_resume(
            ApiError::auth_provider_unavailable(),
            resume_token.clone(),
            oauth_error,
        )
    })?;
    let claims = id_token
        .claims(&client.id_token_verifier(), &Nonce::new(nonce))
        .map_err(|_| {
            CallbackFailure::with_resume(
                ApiError::auth_required(),
                resume_token.clone(),
                oauth_error,
            )
        })?;
    let email = claims
        .email()
        .map(|value| value.as_str().trim().to_owned())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            CallbackFailure::with_resume(
                ApiError::auth_required(),
                resume_token.clone(),
                oauth_error,
            )
        })?;
    if claims.email_verified() != Some(true) {
        return Err(CallbackFailure::with_resume(
            ApiError::auth_required(),
            resume_token,
            oauth_error,
        ));
    }
    let display_name = claims
        .name()
        .and_then(|name| name.get(None))
        .map(|value| value.to_string().trim().to_owned())
        .filter(|value| !value.is_empty());
    let google_subject = claims.subject().as_str().to_owned();

    let now = timestamp_now();
    let proposed_id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO users
            (id, google_subject, email, display_name, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?)
         ON CONFLICT(google_subject) DO UPDATE SET
            email = excluded.email,
            display_name = COALESCE(excluded.display_name, users.display_name),
            updated_at = excluded.updated_at",
    )
    .bind(proposed_id.to_string())
    .bind(google_subject)
    .bind(email)
    .bind(display_name)
    .bind(&now)
    .bind(&now)
    .execute(&state.db)
    .await
    .map_err(|_| {
        CallbackFailure::with_resume(
            ApiError::service_unavailable(),
            resume_token.clone(),
            oauth_error,
        )
    })?;
    let user_id = sqlx::query_scalar::<_, String>("SELECT id FROM users WHERE google_subject = ?")
        .bind(claims.subject().as_str())
        .fetch_one(&state.db)
        .await
        .map_err(|_| {
            CallbackFailure::with_resume(
                ApiError::service_unavailable(),
                resume_token.clone(),
                oauth_error,
            )
        })?;

    let session_token = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO sessions (token_hash, user_id, created_at, expires_at)
         VALUES (?, ?, ?, ?)",
    )
    .bind(hash_token(&session_token))
    .bind(user_id)
    .bind(&now)
    .bind(timestamp_after(SESSION_TTL_SECONDS))
    .execute(&state.db)
    .await
    .map_err(|_| {
        CallbackFailure::with_resume(
            ApiError::service_unavailable(),
            resume_token.clone(),
            oauth_error,
        )
    })?;

    let mut cookie = session_cookie(session_token, state);
    cookie.set_max_age(Duration::seconds(SESSION_TTL_SECONDS));
    let redirect = continue_path.unwrap_or_else(|| "/".to_owned());
    Ok((CookieJar::new().add(cookie), Redirect::to(&redirect)))
}

async fn consume_oauth_state(
    pool: &sqlx::SqlitePool,
    state_value: &str,
) -> Result<Option<(String, String, Option<String>)>, sqlx::Error> {
    sqlx::query(
        "DELETE FROM oauth_states
         WHERE expires_at <= strftime('%Y-%m-%dT%H:%M:%fZ', 'now')",
    )
    .execute(pool)
    .await?;
    sqlx::query_as::<_, (String, String, Option<String>)>(
        "DELETE FROM oauth_states
         WHERE state_hash = ?
           AND expires_at > strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         RETURNING nonce, pkce_verifier, continue_path",
    )
    .bind(hash_token(state_value))
    .fetch_optional(pool)
    .await
}

fn parse_resume_token(path: &str) -> Option<String> {
    let (_, query) = path.split_once('?')?;
    query
        .split('&')
        .find_map(|pair| pair.strip_prefix("resume="))
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

fn oauth_failure_redirect(base: &str, error: &str, state: &str) -> Option<Redirect> {
    let mut url = url::Url::parse(base).ok()?;
    {
        let mut query = url.query_pairs_mut();
        query.append_pair("error", error);
        query.append_pair("state", state);
    }
    Some(Redirect::to(url.as_str()))
}

async fn me(
    State(state): State<AppState>,
    jar: CookieJar,
) -> Result<Json<CurrentUserResponse>, ApiError> {
    let token = jar
        .get(SESSION_COOKIE)
        .map(|cookie| cookie.value().to_owned());
    let user = match token {
        Some(token) => session_user(&state.db, &token).await?,
        None => {
            sqlx::query(
                "DELETE FROM sessions
                 WHERE expires_at <= strftime('%Y-%m-%dT%H:%M:%fZ', 'now')",
            )
            .execute(&state.db)
            .await
            .map_err(|_| ApiError::service_unavailable())?;
            None
        }
    };
    Ok(Json(CurrentUserResponse { user }))
}

async fn logout(
    State(state): State<AppState>,
    jar: CookieJar,
) -> Result<(CookieJar, StatusCode), ApiError> {
    let token = jar
        .get(SESSION_COOKIE)
        .map(|cookie| cookie.value().to_owned());
    if let Some(token) = token {
        sqlx::query("DELETE FROM sessions WHERE token_hash = ?")
            .bind(hash_token(&token))
            .execute(&state.db)
            .await
            .map_err(|_| ApiError::service_unavailable())?;
    }
    let mut expired = session_cookie(String::new(), &state);
    expired.set_max_age(Duration::seconds(0));
    Ok((jar.add(expired), StatusCode::NO_CONTENT))
}

#[derive(Debug, Deserialize)]
struct TestLoginQuery {
    user: Option<String>,
}

async fn test_login(
    State(state): State<AppState>,
    Query(query): Query<TestLoginQuery>,
) -> Result<(CookieJar, Redirect), ApiError> {
    if !cfg!(debug_assertions)
        || std::env::var("GARMIN_FIT_TEST_AUTH").ok().as_deref() != Some("true")
    {
        return Err(ApiError::api_route_not_found());
    }
    let slug = query.user.unwrap_or_else(|| "default".to_owned());
    let user_id = Uuid::new_v5(&Uuid::NAMESPACE_URL, slug.as_bytes());
    let now = timestamp_now();
    let google_subject = format!("test:{slug}");
    sqlx::query(
        "INSERT INTO users
            (id, google_subject, email, display_name, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?)
         ON CONFLICT(google_subject) DO UPDATE SET
            email = excluded.email,
            display_name = excluded.display_name,
            updated_at = excluded.updated_at",
    )
    .bind(user_id.to_string())
    .bind(google_subject)
    .bind(format!("{slug}@example.test"))
    .bind(&slug)
    .bind(&now)
    .bind(&now)
    .execute(&state.db)
    .await
    .map_err(|_| ApiError::service_unavailable())?;

    let token = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO sessions (token_hash, user_id, created_at, expires_at)
         VALUES (?, ?, ?, ?)",
    )
    .bind(hash_token(&token))
    .bind(user_id.to_string())
    .bind(&now)
    .bind(timestamp_after(SESSION_TTL_SECONDS))
    .execute(&state.db)
    .await
    .map_err(|_| ApiError::service_unavailable())?;
    let mut cookie = session_cookie(token, &state);
    cookie.set_max_age(Duration::seconds(SESSION_TTL_SECONDS));
    Ok((CookieJar::new().add(cookie), Redirect::to("/")))
}

fn session_cookie(value: String, state: &AppState) -> Cookie<'static> {
    let secure = state
        .auth
        .google
        .as_ref()
        .is_some_and(|config| config.redirect_uri.starts_with("https://"));
    let mut cookie = Cookie::new(SESSION_COOKIE, value);
    cookie.set_http_only(true);
    cookie.set_same_site(SameSite::Lax);
    cookie.set_path("/");
    cookie.set_secure(secure);
    cookie
}

fn timestamp_now() -> String {
    OffsetDateTime::now_utc()
        .format(TIMESTAMP_FORMAT)
        .expect("fixed timestamp format is valid")
}

fn timestamp_after(seconds: i64) -> String {
    (OffsetDateTime::now_utc() + Duration::seconds(seconds))
        .format(TIMESTAMP_FORMAT)
        .expect("fixed timestamp format is valid")
}
