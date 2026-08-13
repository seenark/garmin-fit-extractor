use axum::{
    Router,
    extract::{Form, Query, State},
    http::StatusCode,
    response::{IntoResponse, Redirect, Response},
    routing::{get, post},
};
use axum_extra::extract::CookieJar;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use url::{Url, form_urlencoded};

use crate::{
    app::AppState,
    auth::{SESSION_COOKIE, session_user},
    config::COACH_CLIENT_ID,
    db,
};

const SCOPE: &str = "activities:read";
const CODE_TTL_SECONDS: i64 = 5 * 60;
const LOGIN_TTL_SECONDS: i64 = 10 * 60;
const ACCESS_TTL_SECONDS: i64 = 60 * 60;
const REFRESH_TTL_SECONDS: i64 = 30 * 24 * 60 * 60;

#[derive(Debug)]
pub struct OAuthProtocolError(pub &'static str);

impl IntoResponse for OAuthProtocolError {
    fn into_response(self) -> Response {
        (
            StatusCode::BAD_REQUEST,
            axum::Json(serde_json::json!({ "error": self.0 })),
        )
            .into_response()
    }
}

#[derive(Debug, Deserialize)]
struct AuthorizeQuery {
    client_id: Option<String>,
    redirect_uri: Option<String>,
    response_type: Option<String>,
    scope: Option<String>,
    state: Option<String>,
    resume: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TokenForm {
    grant_type: Option<String>,
    client_id: Option<String>,
    client_secret: Option<String>,
    code: Option<String>,
    redirect_uri: Option<String>,
    refresh_token: Option<String>,
}

#[derive(Debug, Serialize)]
struct TokenResponse {
    access_token: String,
    token_type: &'static str,
    expires_in: i64,
    refresh_token: String,
    scope: &'static str,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/oauth/authorize", get(authorize))
        .route("/oauth/token", post(token))
}

async fn authorize(
    State(state): State<AppState>,
    jar: CookieJar,
    Query(query): Query<AuthorizeQuery>,
) -> Result<Response, OAuthProtocolError> {
    let Some(config) = state.auth.coach_oauth.as_ref() else {
        return Err(OAuthProtocolError("invalid_request"));
    };
    if let Some(resume) = query.resume.as_deref() {
        let user_id = valid_session_user(&state, &jar)
            .await?
            .ok_or(OAuthProtocolError("invalid_grant"))?;
        let pending = db::consume_oauth_login_request(&state.db, resume)
            .await
            .map_err(|_| OAuthProtocolError("server_error"))?
            .ok_or(OAuthProtocolError("invalid_grant"))?;
        if pending.client_id != config.client_id
            || pending.redirect_uri != config.redirect_uri
            || pending.scope != SCOPE
        {
            return Err(OAuthProtocolError("invalid_grant"));
        }
        let code = random_token();
        insert_code(&state, &code, &pending.redirect_uri, user_id).await?;
        return Ok(callback_redirect(
            &pending.redirect_uri,
            &[("code", code.as_str()), ("state", pending.state.as_str())],
        )?
        .into_response());
    }

    let client_id = query
        .client_id
        .as_deref()
        .ok_or(OAuthProtocolError("invalid_request"))?;
    if client_id != COACH_CLIENT_ID || client_id != config.client_id {
        return Err(OAuthProtocolError("invalid_request"));
    }
    let redirect_uri = query
        .redirect_uri
        .as_deref()
        .ok_or(OAuthProtocolError("invalid_request"))?;
    if redirect_uri != config.redirect_uri {
        return Err(OAuthProtocolError("invalid_request"));
    }

    let state_value = query.state.as_deref().unwrap_or("");
    if query.response_type.as_deref() != Some("code")
        || query.scope.as_deref() != Some(SCOPE)
        || state_value.is_empty()
    {
        return Ok(callback_redirect(
            redirect_uri,
            &[("error", "invalid_request"), ("state", state_value)],
        )?
        .into_response());
    }

    if let Some(user_id) = valid_session_user(&state, &jar).await? {
        let code = random_token();
        insert_code(&state, &code, redirect_uri, user_id).await?;
        return Ok(callback_redirect(
            redirect_uri,
            &[("code", code.as_str()), ("state", state_value)],
        )?
        .into_response());
    }

    let pending = random_token();
    db::insert_oauth_login_request(
        &state.db,
        &pending,
        client_id,
        redirect_uri,
        state_value,
        SCOPE,
        &db::timestamp_now(),
        &db::timestamp_after(LOGIN_TTL_SECONDS),
    )
    .await
    .map_err(|_| OAuthProtocolError("server_error"))?;
    Ok(login_redirect(&pending))
}

async fn token(
    State(state): State<AppState>,
    Form(form): Form<TokenForm>,
) -> Result<axum::Json<TokenResponse>, OAuthProtocolError> {
    let Some(config) = state.auth.coach_oauth.as_ref() else {
        return Err(OAuthProtocolError("invalid_client"));
    };
    if form.client_id.as_deref() != Some(config.client_id.as_str())
        || form.client_secret.as_deref() != Some(config.client_secret.as_str())
    {
        return Err(OAuthProtocolError("invalid_client"));
    }
    let grant_type = form
        .grant_type
        .as_deref()
        .ok_or(OAuthProtocolError("invalid_request"))?;
    let user_id = match grant_type {
        "authorization_code" => {
            let code = form
                .code
                .as_deref()
                .ok_or(OAuthProtocolError("invalid_request"))?;
            let redirect_uri = form
                .redirect_uri
                .as_deref()
                .ok_or(OAuthProtocolError("invalid_request"))?;
            if redirect_uri != config.redirect_uri {
                return Err(OAuthProtocolError("invalid_grant"));
            }
            db::consume_authorization_code(&state.db, code, &config.client_id, redirect_uri, SCOPE)
                .await
                .map_err(|_| OAuthProtocolError("server_error"))?
                .ok_or(OAuthProtocolError("invalid_grant"))?
                .user_id
        }
        "refresh_token" => {
            let old_refresh_token = form
                .refresh_token
                .as_deref()
                .ok_or(OAuthProtocolError("invalid_request"))?;
            let access_token = random_token();
            let refresh_token = random_token();
            let created_at = db::timestamp_now();
            return rotate_refresh(
                &state,
                config,
                old_refresh_token,
                &access_token,
                &refresh_token,
                &created_at,
            )
            .await;
        }
        _ => return Err(OAuthProtocolError("unsupported_grant_type")),
    };

    let access_token = random_token();
    let refresh_token = random_token();
    let created_at = db::timestamp_now();
    db::insert_token_pair(
        &state.db,
        &access_token,
        &refresh_token,
        &config.client_id,
        user_id,
        SCOPE,
        &created_at,
        &db::timestamp_after(ACCESS_TTL_SECONDS),
        &db::timestamp_after(REFRESH_TTL_SECONDS),
    )
    .await
    .map_err(|_| OAuthProtocolError("server_error"))?;
    Ok(axum::Json(token_response(access_token, refresh_token)))
}

async fn rotate_refresh(
    state: &AppState,
    config: &crate::config::CoachOAuthConfig,
    old_refresh_token: &str,
    access_token: &str,
    refresh_token: &str,
    created_at: &str,
) -> Result<axum::Json<TokenResponse>, OAuthProtocolError> {
    db::rotate_refresh_token(
        &state.db,
        old_refresh_token,
        &config.client_id,
        SCOPE,
        access_token,
        refresh_token,
        created_at,
        &db::timestamp_after(ACCESS_TTL_SECONDS),
        &db::timestamp_after(REFRESH_TTL_SECONDS),
    )
    .await
    .map_err(|_| OAuthProtocolError("server_error"))?
    .ok_or(OAuthProtocolError("invalid_grant"))?;
    Ok(axum::Json(token_response(
        access_token.to_owned(),
        refresh_token.to_owned(),
    )))
}

async fn insert_code(
    state: &AppState,
    code: &str,
    redirect_uri: &str,
    user_id: uuid::Uuid,
) -> Result<(), OAuthProtocolError> {
    db::insert_authorization_code(
        &state.db,
        code,
        COACH_CLIENT_ID,
        redirect_uri,
        user_id,
        SCOPE,
        &db::timestamp_now(),
        &db::timestamp_after(CODE_TTL_SECONDS),
    )
    .await
    .map_err(|_| OAuthProtocolError("server_error"))
}

async fn valid_session_user(
    state: &AppState,
    jar: &CookieJar,
) -> Result<Option<uuid::Uuid>, OAuthProtocolError> {
    let Some(token) = jar
        .get(SESSION_COOKIE)
        .map(|cookie| cookie.value().to_owned())
    else {
        return Ok(None);
    };
    session_user(&state.db, &token)
        .await
        .map(|user| user.map(|user| user.id))
        .map_err(|_| OAuthProtocolError("server_error"))
}

fn token_response(access_token: String, refresh_token: String) -> TokenResponse {
    TokenResponse {
        access_token,
        token_type: "Bearer",
        expires_in: ACCESS_TTL_SECONDS,
        refresh_token,
        scope: SCOPE,
    }
}

fn random_token() -> String {
    let mut bytes = [0_u8; 32];
    rand::rng().fill_bytes(&mut bytes);
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn login_redirect(token: &str) -> Response {
    let query = form_urlencoded::Serializer::new(String::new())
        .append_pair("continue", token)
        .finish();
    Redirect::to(&format!("/api/v1/auth/login?{query}")).into_response()
}

fn callback_redirect(base: &str, pairs: &[(&str, &str)]) -> Result<Redirect, OAuthProtocolError> {
    let mut url = Url::parse(base).map_err(|_| OAuthProtocolError("invalid_request"))?;
    {
        let mut query = url.query_pairs_mut();
        for (key, value) in pairs {
            query.append_pair(key, value);
        }
    }
    Ok(Redirect::to(url.as_str()))
}
