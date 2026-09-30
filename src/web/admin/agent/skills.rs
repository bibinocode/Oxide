//! Skill 包管理 API：会话与 CSRF 保护，保留全部文件，不接收数据库指令配置。

use super::super::auth::require_admin;
use crate::{
    infrastructure::agent_skills::{
        MAX_ARCHIVE_BYTES, MAX_FILE_BYTES, MAX_FILES, MAX_PACKAGE_BYTES, PackageFile,
    },
    web::{ApiError, AppState, error},
};
use axum::{
    Json,
    body::Body,
    extract::{Multipart, Path, Query, State},
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
};
use futures_util::StreamExt;
use serde::Deserialize;
use std::time::Duration;
use utoipa::ToSchema;

/// 开关仅更改包外状态文件。
#[derive(Deserialize, ToSchema)]
pub struct SkillEnabledInput {
    pub enabled: bool,
}

/// 编辑原始文档，不拆解、重写 YAML，不修改包内其他文件。
#[derive(Deserialize, ToSchema)]
pub struct SkillDocumentInput {
    pub document: String,
}

/// GitHub 安装支持整个仓库或 tree 链接，显式指定子目录可以选择多技能仓库中的一个包。
#[derive(Deserialize, ToSchema)]
pub struct GithubInstallInput {
    pub url: String,
    pub revision: Option<String>,
    #[serde(default)]
    pub skill_path: String,
    #[serde(default)]
    pub overwrite: bool,
}

/// 文件预览使用查询路径，不接受服务器绝对路径。
#[derive(Deserialize)]
pub struct SkillFileQuery {
    pub path: String,
}

/// 预期输入错误可说明原因；文件系统故障不暴露服务器路径。
fn failure(cause: anyhow::Error) -> Response {
    if let Some(io) = cause.downcast_ref::<std::io::Error>() {
        if io.kind() == std::io::ErrorKind::NotFound {
            return error(
                StatusCode::NOT_FOUND,
                "skill_not_found",
                "Skill 或文件不存在",
            );
        }
        tracing::error!(error = %cause, "Skill 文件仓库操作失败");
        return error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "skill_storage_error",
            "技能目录暂时不可用，请检查服务端目录权限",
        );
    }
    (
        StatusCode::BAD_REQUEST,
        Json(serde_json::json!({"code": "invalid_skill_package", "message": cause.to_string()})),
    )
        .into_response()
}

/// 返回第一层技能发现信息，坏包单独告警，不妨碍其他包使用。
#[utoipa::path(get, path = "/api/v1/admin/agent/skills", responses((status = 200, body = crate::infrastructure::agent_skills::SkillCatalog), (status = 401, body = ApiError)), tag = "agent")]
pub async fn list(State(state): State<AppState>, headers: HeaderMap) -> Response {
    if let Err(response) = require_admin(&state, &headers, false).await {
        return response;
    }
    match state.skills.list().await {
        Ok(result) => Json(result).into_response(),
        Err(cause) => failure(cause),
    }
}

/// 完整包详情仅在管理员打开时读取，不预加载到模型。
#[utoipa::path(get, path = "/api/v1/admin/agent/skills/{name}", params(("name" = String, Path)), responses((status = 200, body = crate::infrastructure::agent_skills::SkillDetail), (status = 404, body = ApiError)), tag = "agent")]
pub async fn get(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(name): Path<String>,
) -> Response {
    if let Err(response) = require_admin(&state, &headers, false).await {
        return response;
    }
    match state.skills.detail(&name).await {
        Ok(result) => Json(result).into_response(),
        Err(cause) => failure(cause),
    }
}

/// 上传 ZIP 或完整文件夹；前端以 file 字段上传 ZIP，files 字段传文件夹相对路径。
#[utoipa::path(post, path = "/api/v1/admin/agent/skills/install", responses((status = 201, body = crate::infrastructure::agent_skills::SkillDetail), (status = 400, body = ApiError)), tag = "agent")]
pub async fn install(
    State(state): State<AppState>,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> Response {
    if let Err(response) = require_admin(&state, &headers, true).await {
        return response;
    }
    let result = async {
        let mut archive = None;
        let mut files = Vec::new();
        let mut total = 0usize;
        let mut skill_path = String::new();
        let mut overwrite = false;
        let mut fields = 0usize;
        while let Some(mut field) = multipart.next_field().await? {
            fields += 1;
            if fields > MAX_FILES + 4 {
                anyhow::bail!("上传字段过多");
            }
            let kind = field.name().unwrap_or_default().to_owned();
            let path = field.file_name().unwrap_or_default().to_owned();
            let limit = match kind.as_str() {
                "file" => MAX_ARCHIVE_BYTES,
                "files" => MAX_FILE_BYTES,
                "skill_path" | "overwrite" => 1024,
                _ => anyhow::bail!("未知的上传字段"),
            };
            let mut bytes = Vec::new();
            while let Some(chunk) = field.chunk().await? {
                if bytes.len() + chunk.len() > limit || total + chunk.len() > MAX_PACKAGE_BYTES {
                    anyhow::bail!("上传文件超过大小预算");
                }
                total += chunk.len();
                bytes.extend_from_slice(&chunk);
            }
            match kind.as_str() {
                "file" => {
                    if archive.is_some() {
                        anyhow::bail!("一次只能上传一个 ZIP");
                    }
                    archive = Some(bytes);
                }
                "files" => files.push(PackageFile {
                    path,
                    bytes,
                    mode: None,
                }),
                "skill_path" => skill_path = String::from_utf8(bytes)?,
                "overwrite" => overwrite = bytes == b"true",
                _ => unreachable!(),
            }
        }
        if let Some(bytes) = archive {
            if !files.is_empty() {
                anyhow::bail!("不能混合 ZIP 与文件夹上传");
            }
            state
                .skills
                .install_archive(bytes, skill_path, overwrite, "上传 ZIP".into())
                .await
        } else {
            state
                .skills
                .install_files(files, skill_path, overwrite)
                .await
        }
    }
    .await;
    match result {
        Ok(detail) => (StatusCode::CREATED, Json(detail)).into_response(),
        Err(cause) => failure(cause),
    }
}

/// 仅下载固定 GitHub codeload 主机，不跟随任意站点重定向，也不接收凭据或执行安装脚本。
#[utoipa::path(post, path = "/api/v1/admin/agent/skills/install/github", request_body = GithubInstallInput, responses((status = 201, body = crate::infrastructure::agent_skills::SkillDetail), (status = 400, body = ApiError)), tag = "agent")]
pub async fn install_github(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<GithubInstallInput>,
) -> Response {
    if let Err(response) = require_admin(&state, &headers, true).await {
        return response;
    }
    let result = async {
        if input.url.len() > 2048 {
            anyhow::bail!("GitHub 地址不能超过 2048 字符");
        }
        let url = url::Url::parse(&input.url)?;
        if url.scheme() != "https"
            || url.host_str() != Some("github.com")
            || !url.username().is_empty()
            || url.password().is_some()
            || url.port().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
        {
            anyhow::bail!("请填写公开的 https://github.com 仓库或 tree 链接");
        }
        let parts = url
            .path_segments()
            .into_iter()
            .flatten()
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>();
        if parts.len() < 2
            || !parts[..2].iter().all(|part| {
                part.bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b"-_.".contains(&byte))
            })
        {
            anyhow::bail!("GitHub 仓库地址无效");
        }
        if parts.len() > 2 && (parts.len() < 4 || parts[2] != "tree") {
            anyhow::bail!("请选择仓库主页或 tree 目录链接");
        }
        let revision = input
            .revision
            .as_deref()
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| parts.get(3).copied().unwrap_or("main"));
        if revision.len() > 200 || revision.contains(['?', '#', '\\']) {
            anyhow::bail!("GitHub revision 无效");
        }
        let skill_path = if input.skill_path.is_empty() {
            parts.get(4..).unwrap_or_default().join("/")
        } else {
            input.skill_path
        };
        let mut download = url::Url::parse("https://codeload.github.com")?;
        download
            .path_segments_mut()
            .map_err(|_| anyhow::anyhow!("下载地址无效"))?
            .extend([parts[0], parts[1].trim_end_matches(".git"), "zip", revision]);
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(90))
            .redirect(reqwest::redirect::Policy::none())
            .build()?;
        let response = client
            .get(download)
            .send()
            .await
            .map_err(|_| anyhow::anyhow!("无法连接 GitHub，请改用 ZIP 上传"))?;
        if !response.status().is_success() {
            anyhow::bail!("GitHub 下载失败，请检查公开仓库地址和分支；也可上传 ZIP");
        }
        if response
            .content_length()
            .is_some_and(|length| length > MAX_ARCHIVE_BYTES as u64)
        {
            anyhow::bail!("GitHub 仓库 ZIP 超过 32 MiB，请只上传目标技能目录");
        }
        let mut bytes = Vec::new();
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|_| anyhow::anyhow!("GitHub 下载中断"))?;
            if bytes.len() + chunk.len() > MAX_ARCHIVE_BYTES {
                anyhow::bail!("GitHub 仓库 ZIP 超过 32 MiB");
            }
            bytes.extend_from_slice(&chunk);
        }
        state
            .skills
            .install_archive(bytes, skill_path, input.overwrite, input.url)
            .await
    }
    .await;
    match result {
        Ok(detail) => (StatusCode::CREATED, Json(detail)).into_response(),
        Err(cause) => failure(cause),
    }
}

/// 编辑原始 SKILL.md，其他配套文件保持不变。
#[utoipa::path(put, path = "/api/v1/admin/agent/skills/{name}", params(("name" = String, Path)), request_body = SkillDocumentInput, responses((status = 200, body = crate::infrastructure::agent_skills::SkillDetail)), tag = "agent")]
pub async fn update(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(name): Path<String>,
    Json(input): Json<SkillDocumentInput>,
) -> Response {
    if let Err(response) = require_admin(&state, &headers, true).await {
        return response;
    }
    match state.skills.save_document(&name, input.document).await {
        Ok(detail) => Json(detail).into_response(),
        Err(cause) => failure(cause),
    }
}

/// 启停仅修改仓库状态文件，第三方包保持原样。
#[utoipa::path(patch, path = "/api/v1/admin/agent/skills/{name}/enabled", params(("name" = String, Path)), request_body = SkillEnabledInput, responses((status = 204)), tag = "agent")]
pub async fn set_enabled(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(name): Path<String>,
    Json(input): Json<SkillEnabledInput>,
) -> Response {
    if let Err(response) = require_admin(&state, &headers, true).await {
        return response;
    }
    match state.skills.set_enabled(&name, input.enabled).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(cause) => failure(cause),
    }
}

/// 卸载整个包，而非仅删除一条指令记录。
#[utoipa::path(delete, path = "/api/v1/admin/agent/skills/{name}", params(("name" = String, Path)), responses((status = 204)), tag = "agent")]
pub async fn delete(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(name): Path<String>,
) -> Response {
    if let Err(response) = require_admin(&state, &headers, true).await {
        return response;
    }
    match state.skills.uninstall(&name).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(cause) => failure(cause),
    }
}

/// 原样下载资源；强制 attachment 与 nosniff，防止第三方 HTML 在管理域执行。
#[utoipa::path(get, path = "/api/v1/admin/agent/skills/{name}/files", params(("name" = String, Path), ("path" = String, Query)), responses((status = 200)), tag = "agent")]
pub async fn file(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(name): Path<String>,
    Query(query): Query<SkillFileQuery>,
) -> Response {
    if let Err(response) = require_admin(&state, &headers, false).await {
        return response;
    }
    match state.skills.read_file(&name, &query.path).await {
        Ok(bytes) => (
            [
                (header::CONTENT_TYPE, "application/octet-stream"),
                (header::CONTENT_DISPOSITION, "attachment"),
                (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
                (header::CACHE_CONTROL, "no-store"),
            ],
            Body::from(bytes),
        )
            .into_response(),
        Err(cause) => failure(cause),
    }
}

/// 完整 ZIP 导出便于备份和第三方共享。
#[utoipa::path(get, path = "/api/v1/admin/agent/skills/{name}/export", params(("name" = String, Path)), responses((status = 200)), tag = "agent")]
pub async fn export(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(name): Path<String>,
) -> Response {
    if let Err(response) = require_admin(&state, &headers, false).await {
        return response;
    }
    match state.skills.export(&name).await {
        Ok(bytes) => (
            [
                (header::CONTENT_TYPE, "application/zip".to_owned()),
                (
                    header::CONTENT_DISPOSITION,
                    format!("attachment; filename=\"{name}.zip\""),
                ),
                (header::CACHE_CONTROL, "no-store".to_owned()),
            ],
            Body::from(bytes),
        )
            .into_response(),
        Err(cause) => failure(cause),
    }
}
