//! 文章生图：已绑定图像模型生成图片并登记到私有素材库。

use axum::{
    Json,
    body::Bytes,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use chrono::Utc;
use sea_orm::{ActiveModelTrait, EntityTrait, Set};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    agent::image::{self, ImagePurpose},
    entity::{asset, site_setting, status::AssetVisibility, storage_provider},
    infrastructure::{agent_image, storage},
    web::{ApiError, AppState, error},
};

use super::super::auth::require_admin;

/// 当前编辑器内容及画面补充要求；不会写回文章。
#[derive(Deserialize, ToSchema)]
pub struct ImageInput {
    pub title: String,
    pub source: String,
    pub instruction: String,
    pub purpose: ImagePurpose,
}

/// 生成素材仍为私有；管理员选择使用时通过素材可见性接口公开。
#[derive(Serialize, ToSchema)]
pub struct ImageOutput {
    pub asset_public_id: Uuid,
    pub media_url: String,
}

#[utoipa::path(post, path = "/api/v1/admin/agent/image", request_body = ImageInput, responses(
    (status = 201, body = ImageOutput), (status = 400, body = ApiError),
    (status = 401, body = ApiError), (status = 409, body = ApiError), (status = 502, body = ApiError)
), tag = "agent")]
pub async fn generate(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<ImageInput>,
) -> Response {
    if let Err(response) = require_admin(&state, &headers, true).await {
        return response;
    }
    if input.source.chars().count() > 50_000 {
        return error(
            StatusCode::BAD_REQUEST,
            "invalid_image_input",
            "文章内容过长",
        );
    }
    let prompt = match image::prompt(
        &input.title,
        &input.source,
        &input.instruction,
        input.purpose,
    ) {
        Ok(prompt) => prompt,
        Err(_) => {
            return error(
                StatusCode::BAD_REQUEST,
                "invalid_image_input",
                "标题或画面要求无效",
            );
        }
    };
    let provider = match agent_image::image_provider(&state.db).await {
        Ok(Some(provider)) => provider,
        Ok(None) => {
            return error(
                StatusCode::CONFLICT,
                "image_model_unavailable",
                "请先绑定图像模型",
            );
        }
        Err(cause) => {
            tracing::error!(error = %cause, "查询图像模型失败");
            return error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "服务暂时不可用",
            );
        }
    };
    let storage_provider = match site_setting::Entity::find_by_id(1).one(&state.db).await {
        Ok(Some(settings)) => match settings.active_provider_id {
            Some(id) => storage_provider::Entity::find_by_id(id)
                .one(&state.db)
                .await
                .ok()
                .flatten(),
            None => None,
        },
        _ => None,
    };
    let Some(storage_provider) = storage_provider.filter(|item| item.upload_enabled) else {
        return error(
            StatusCode::CONFLICT,
            "storage_unavailable",
            "请先启用素材存储提供商",
        );
    };
    if storage::configured(&storage_provider, &state.comment_hash_key).is_err() {
        return error(
            StatusCode::CONFLICT,
            "storage_unavailable",
            "素材存储配置不完整",
        );
    }
    let bytes = match agent_image::generate(&provider, &state.comment_hash_key, &prompt).await {
        Ok(bytes) => bytes,
        Err(cause) => {
            tracing::warn!(provider = %provider.id, error = %cause, "文章生图失败");
            return error(
                StatusCode::BAD_GATEWAY,
                "image_generation_failed",
                "生图失败，请检查模型接口或稍后重试",
            );
        }
    };
    if bytes.is_empty() || bytes.len() > 8 * 1024 * 1024 {
        return error(
            StatusCode::BAD_GATEWAY,
            "invalid_generated_image",
            "生成图片大小无效",
        );
    }
    let Some(kind) = infer::get(&bytes) else {
        return error(
            StatusCode::BAD_GATEWAY,
            "invalid_generated_image",
            "模型未返回可用图片",
        );
    };
    let (mime, extension) = match kind.mime_type() {
        "image/png" => ("image/png", "png"),
        "image/jpeg" => ("image/jpeg", "jpg"),
        "image/webp" => ("image/webp", "webp"),
        _ => {
            return error(
                StatusCode::BAD_GATEWAY,
                "invalid_generated_image",
                "模型返回的图片格式不支持",
            );
        }
    };
    let dimensions = match imagesize::blob_size(&bytes) {
        Ok(dimensions) if dimensions.width <= 10_000 && dimensions.height <= 10_000 => dimensions,
        _ => {
            return error(
                StatusCode::BAD_GATEWAY,
                "invalid_generated_image",
                "生成图片尺寸无效",
            );
        }
    };
    let public_id = Uuid::new_v4();
    let created_at = Utc::now();
    let key = crate::domain::asset::image_object_key(public_id, created_at, extension);
    if let Err(cause) = storage::put(
        &storage_provider,
        &state.comment_hash_key,
        &key,
        Bytes::from(bytes.clone()),
        mime,
    )
    .await
    {
        tracing::error!(error = %cause, "保存生成图片失败");
        return error(
            StatusCode::SERVICE_UNAVAILABLE,
            "storage_unavailable",
            "无法保存生成图片",
        );
    }
    let row = asset::ActiveModel {
        public_id: Set(public_id),
        provider_id: Set(storage_provider.id.clone()),
        object_key: Set(key.clone()),
        mime_type: Set(mime.into()),
        size_bytes: Set(bytes.len() as i64),
        width: Set(Some(dimensions.width as i32)),
        height: Set(Some(dimensions.height as i32)),
        visibility: Set(AssetVisibility::Private),
        created_at: Set(created_at),
        ..Default::default()
    };
    if let Err(cause) = row.insert(&state.db).await {
        tracing::error!(error = %cause, "登记生成素材失败");
        let _ = storage::delete(&storage_provider, &state.comment_hash_key, &key).await;
        return error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal_error",
            "无法登记生成素材",
        );
    }
    (
        StatusCode::CREATED,
        Json(ImageOutput {
            asset_public_id: public_id,
            media_url: format!("/media/{public_id}"),
        }),
    )
        .into_response()
}
