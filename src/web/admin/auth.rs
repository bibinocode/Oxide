//! 单作者管理员初始化、登录和 Redis 会话。

use anyhow::{Context, Result};
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier, password_hash::SaltString};
use axum::{
    Json,
    extract::State,
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
};
use cookie::{Cookie, SameSite};
use rand_core::OsRng;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter,
    Set,
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::entity::admin_user;

use super::super::{ApiError, AppState, error};

/// Redis 中的短期会话；Cookie 仅包含随机令牌。
#[derive(Serialize, Deserialize)]
pub struct Session {
    /// 管理员名称，仅用于显示。
    pub username: String,
    /// 写请求要求的 CSRF 令牌。
    pub csrf_token: String,
}

/// 登录请求。
#[derive(Deserialize, ToSchema)]
pub struct LoginRequest {
    /// 管理员名称。
    pub username: String,
    /// 管理员密码。
    pub password: String,
}

/// 登录及当前会话响应。
#[derive(Serialize, ToSchema)]
pub struct SessionResponse {
    /// 当前管理员名称。
    pub username: String,
    /// 发送写请求时放入 `X-CSRF-Token` 请求头。
    pub csrf_token: String,
}

/// 无内容命令的响应。
#[derive(Serialize, ToSchema)]
pub struct OkResponse {
    /// 操作结果。
    pub status: &'static str,
}

/// 只在管理员表为空时通过环境变量创建唯一账号。
pub async fn bootstrap_admin(
    db: &DatabaseConnection,
    username: Option<&str>,
    password: Option<&str>,
) -> Result<()> {
    let (Some(username), Some(password)) = (username, password) else {
        return Ok(());
    };
    if admin_user::Entity::find().count(db).await? != 0 {
        return Ok(());
    }
    let password = password.to_owned();
    let hash = tokio::task::spawn_blocking(move || -> Result<String> {
        let salt = SaltString::generate(&mut OsRng);
        Ok(Argon2::default()
            .hash_password(password.as_bytes(), &salt)
            .map_err(|error| anyhow::anyhow!(error.to_string()))?
            .to_string())
    })
    .await
    .context("密码摘要任务异常")??;
    let now = chrono::Utc::now();
    admin_user::ActiveModel {
        username: Set(username.to_owned()),
        password_hash: Set(hash),
        created_at: Set(now),
        updated_at: Set(now),
        ..Default::default()
    }
    .insert(db)
    .await?;
    tracing::info!("管理员账号已初始化");
    Ok(())
}

/// 校验用户名和密码并创建 24 小时 Redis 会话。
#[utoipa::path(post, path = "/api/v1/admin/login", request_body = LoginRequest, responses(
    (status = 200, body = SessionResponse), (status = 401, body = ApiError), (status = 503, body = ApiError)
), tag = "admin")]
pub async fn login(State(state): State<AppState>, Json(input): Json<LoginRequest>) -> Response {
    let row = match admin_user::Entity::find()
        .filter(admin_user::Column::Username.eq(&input.username))
        .one(&state.db)
        .await
    {
        Ok(row) => row,
        Err(err) => {
            tracing::error!(error = %err, "读取管理员失败");
            return error(
                StatusCode::SERVICE_UNAVAILABLE,
                "dependency_unavailable",
                "依赖服务不可用",
            );
        }
    };
    let Some(row) = row else {
        return error(
            StatusCode::UNAUTHORIZED,
            "invalid_credentials",
            "账号或密码错误",
        );
    };
    let password = input.password;
    let hash = row.password_hash;
    let valid = tokio::task::spawn_blocking(move || {
        PasswordHash::new(&hash).ok().is_some_and(|parsed| {
            Argon2::default()
                .verify_password(password.as_bytes(), &parsed)
                .is_ok()
        })
    })
    .await
    .unwrap_or(false);
    if !valid {
        return error(
            StatusCode::UNAUTHORIZED,
            "invalid_credentials",
            "账号或密码错误",
        );
    }

    let token = Uuid::new_v4().to_string();
    let session = Session {
        username: row.username,
        csrf_token: Uuid::new_v4().to_string(),
    };
    let encoded = match serde_json::to_string(&session) {
        Ok(value) => value,
        Err(_) => {
            return error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "服务暂时不可用",
            );
        }
    };
    let mut redis = match state.redis.get_multiplexed_async_connection().await {
        Ok(redis) => redis,
        Err(_) => {
            return error(
                StatusCode::SERVICE_UNAVAILABLE,
                "dependency_unavailable",
                "依赖服务不可用",
            );
        }
    };
    if redis::cmd("SET")
        .arg(format!("session:{token}"))
        .arg(encoded)
        .arg("EX")
        .arg(86_400)
        .query_async::<()>(&mut redis)
        .await
        .is_err()
    {
        return error(
            StatusCode::SERVICE_UNAVAILABLE,
            "dependency_unavailable",
            "依赖服务不可用",
        );
    }
    let cookie = Cookie::build(("oxide_session", token))
        .http_only(true)
        .same_site(SameSite::Lax)
        .secure(state.session_secure)
        .path("/")
        .max_age(cookie::time::Duration::days(1))
        .build();
    let mut response = Json(SessionResponse {
        username: session.username,
        csrf_token: session.csrf_token,
    })
    .into_response();
    if let Ok(value) = cookie.to_string().parse() {
        response.headers_mut().insert(header::SET_COOKIE, value);
    }
    response
}

/// 返回当前会话和 CSRF 令牌，供前端刷新后恢复管理状态。
#[utoipa::path(get, path = "/api/v1/admin/session", responses((status = 200, body = SessionResponse), (status = 401, body = ApiError)), tag = "admin")]
pub async fn current(State(state): State<AppState>, headers: HeaderMap) -> Response {
    match get_session(&state, &headers).await {
        Ok(Some(session)) => Json(SessionResponse {
            username: session.username,
            csrf_token: session.csrf_token,
        })
        .into_response(),
        Ok(None) => error(StatusCode::UNAUTHORIZED, "unauthorized", "请先登录"),
        Err(err) => {
            tracing::error!(error = %err, "读取会话失败");
            error(
                StatusCode::SERVICE_UNAVAILABLE,
                "dependency_unavailable",
                "依赖服务不可用",
            )
        }
    }
}

/// 删除 Redis 会话并清除浏览器 Cookie。
#[utoipa::path(post, path = "/api/v1/admin/logout", responses((status = 200, body = OkResponse), (status = 401, body = ApiError)), tag = "admin")]
pub async fn logout(State(state): State<AppState>, headers: HeaderMap) -> Response {
    if let Err(response) = require_admin(&state, &headers, true).await {
        return response;
    }
    if let Some(token) = session_token(&headers)
        && let Ok(mut redis) = state.redis.get_multiplexed_async_connection().await
    {
        let _: redis::RedisResult<()> = redis::cmd("DEL")
            .arg(format!("session:{token}"))
            .query_async(&mut redis)
            .await;
    }
    let cookie = Cookie::build(("oxide_session", ""))
        .http_only(true)
        .same_site(SameSite::Lax)
        .secure(state.session_secure)
        .path("/")
        .max_age(cookie::time::Duration::ZERO)
        .build();
    let mut response = Json(OkResponse { status: "ok" }).into_response();
    if let Ok(value) = cookie.to_string().parse() {
        response.headers_mut().insert(header::SET_COOKIE, value);
    }
    response
}

/// 从 Cookie 提取随机令牌，不读取或信任客户端提供的用户名。
fn session_token(headers: &HeaderMap) -> Option<String> {
    let cookies = headers.get(header::COOKIE)?.to_str().ok()?;
    Cookie::split_parse(cookies)
        .flatten()
        .find(|cookie| cookie.name() == "oxide_session")
        .map(|cookie| cookie.value().to_owned())
}

/// 从 Redis 查找会话；Redis 错误与未登录严格区分。
async fn get_session(state: &AppState, headers: &HeaderMap) -> Result<Option<Session>> {
    let Some(token) = session_token(headers) else {
        return Ok(None);
    };
    let mut redis = state.redis.get_multiplexed_async_connection().await?;
    let data: Option<String> = redis::cmd("GET")
        .arg(format!("session:{token}"))
        .query_async(&mut redis)
        .await?;
    data.map(|data| serde_json::from_str(&data).context("会话数据损坏"))
        .transpose()
}

/// 所有管理端处理器都通过此处执行登录和 CSRF 校验。
pub async fn require_admin(
    state: &AppState,
    headers: &HeaderMap,
    write: bool,
) -> std::result::Result<Session, Response> {
    let session = match get_session(state, headers).await {
        Ok(Some(session)) => session,
        Ok(None) => return Err(error(StatusCode::UNAUTHORIZED, "unauthorized", "请先登录")),
        Err(err) => {
            tracing::error!(error = %err, "校验管理员会话失败");
            return Err(error(
                StatusCode::SERVICE_UNAVAILABLE,
                "dependency_unavailable",
                "依赖服务不可用",
            ));
        }
    };
    if write
        && headers
            .get("x-csrf-token")
            .and_then(|value| value.to_str().ok())
            != Some(session.csrf_token.as_str())
    {
        return Err(error(StatusCode::FORBIDDEN, "invalid_csrf", "请求校验失败"));
    }
    Ok(session)
}
