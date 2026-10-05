//! 管理员维护专栏的标题、简介和当前售价。

use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use chrono::Utc;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, IntoActiveModel, QueryFilter, Set};
use serde::Deserialize;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::entity::paid_column;

use super::{
    super::{ApiError, AppState, client::columns::ColumnResponse, error},
    auth::require_admin,
};

/// 专栏编辑输入；价格以分为单位，避免浮点误差。
#[derive(Deserialize, ToSchema)]
pub struct ColumnInput {
    /// 读者看到的专栏标题。
    pub title: String,
    /// 公开简介，最长 2000 字。
    pub description: String,
    /// 当前售价，单位为分。
    pub price_cents: i32,
}

/// 限制公开标识和价格范围。
fn valid(input: &ColumnInput) -> bool {
    !input.title.trim().is_empty()
        && input.title.chars().count() <= 160
        && input.description.chars().count() <= 2000
        && (100..=1_000_000).contains(&input.price_cents)
}

/// 创建付费专栏，不会自动更改已有文章。
#[utoipa::path(post, path = "/api/v1/admin/columns", request_body = ColumnInput, responses((status = 201, body = ColumnResponse), (status = 401, body = ApiError)), tag = "admin")]
pub async fn create(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<ColumnInput>,
) -> Response {
    if let Err(response) = require_admin(&state, &headers, true).await {
        return response;
    }
    if !valid(&input) {
        return error(StatusCode::BAD_REQUEST, "invalid_column", "专栏内容无效");
    }
    let now = Utc::now();
    let row = paid_column::ActiveModel {
        visible: Set(true),
        id: sea_orm::NotSet,
        public_id: Set(Uuid::new_v4()),
        // 永久链接由服务端生成，不要求作者填写技术标识。
        slug: Set(Uuid::new_v4().simple().to_string()),
        title: Set(input.title.trim().to_owned()),
        description: Set(input.description.trim().to_owned()),
        price_cents: Set(input.price_cents),
        created_at: Set(now),
        updated_at: Set(now),
    }
    .insert(&state.db)
    .await;
    match row {
        Ok(row) => (StatusCode::CREATED, Json(ColumnResponse::from(row))).into_response(),
        Err(err) => {
            tracing::warn!(error = %err, "创建专栏失败");
            error(StatusCode::CONFLICT, "column_conflict", "专栏链接已存在")
        }
    }
}

/// 修改专栏资料；已创建订单保留原价格快照。
#[utoipa::path(put, path = "/api/v1/admin/columns/{public_id}", params(("public_id" = Uuid, Path)), request_body = ColumnInput, responses((status = 200, body = ColumnResponse), (status = 404, body = ApiError)), tag = "admin")]
pub async fn update(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(public_id): Path<Uuid>,
    Json(input): Json<ColumnInput>,
) -> Response {
    if let Err(response) = require_admin(&state, &headers, true).await {
        return response;
    }
    if !valid(&input) {
        return error(StatusCode::BAD_REQUEST, "invalid_column", "专栏内容无效");
    }
    let row = match paid_column::Entity::find()
        .filter(paid_column::Column::PublicId.eq(public_id))
        .one(&state.db)
        .await
    {
        Ok(Some(row)) => row,
        Ok(None) => return error(StatusCode::NOT_FOUND, "column_not_found", "专栏不存在"),
        Err(err) => {
            tracing::error!(error = %err, "读取专栏失败");
            return error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "服务暂时不可用",
            );
        }
    };
    let mut active = row.into_active_model();
    active.title = Set(input.title.trim().to_owned());
    active.description = Set(input.description.trim().to_owned());
    active.price_cents = Set(input.price_cents);
    active.updated_at = Set(Utc::now());
    match active.update(&state.db).await {
        Ok(row) => Json(ColumnResponse::from(row)).into_response(),
        Err(err) => {
            tracing::warn!(error = %err, "修改专栏失败");
            error(StatusCode::CONFLICT, "column_conflict", "专栏链接已存在")
        }
    }
}

/// 管理端读取全部小册，包括已下架记录，避免下架后无法恢复。
#[utoipa::path(get, path = "/api/v1/admin/columns", responses((status = 200, body = Vec<ColumnResponse>)), tag = "admin")]
pub async fn list(State(state): State<AppState>, headers: HeaderMap) -> Response {
    use sea_orm::QueryOrder;
    if let Err(response) = require_admin(&state, &headers, false).await {
        return response;
    }
    match paid_column::Entity::find()
        .order_by_desc(paid_column::Column::CreatedAt)
        .all(&state.db)
        .await
    {
        Ok(rows) => Json(
            rows.into_iter()
                .map(ColumnResponse::from)
                .collect::<Vec<_>>(),
        )
        .into_response(),
        Err(_) => error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal_error",
            "小册读取失败",
        ),
    }
}

/// 下架停止公开展示和新订单，已购买读者的权益不会删除。
#[utoipa::path(patch, path = "/api/v1/admin/columns/{public_id}", params(("public_id" = Uuid, Path)), request_body = super::ContentVisibilityInput, responses((status = 200, body = ColumnResponse), (status = 404, body = ApiError)), tag = "admin")]
pub async fn visibility(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(public_id): Path<Uuid>,
    Json(input): Json<super::ContentVisibilityInput>,
) -> Response {
    if let Err(response) = require_admin(&state, &headers, true).await {
        return response;
    }
    use crate::entity::{
        article, search_job,
        status::{SearchAction, SearchJobStatus},
    };
    use sea_orm::{QuerySelect, TransactionTrait};
    let result = async {
        let txn = state.db.begin().await?;
        let Some(row) = paid_column::Entity::find()
            .filter(paid_column::Column::PublicId.eq(public_id))
            .lock_exclusive()
            .one(&txn)
            .await?
        else {
            return Ok::<_, sea_orm::DbErr>(None);
        };
        let mut active = row.into_active_model();
        active.visible = Set(input.visible);
        active.updated_at = Set(Utc::now());
        let updated = active.update(&txn).await?;
        // 可见性和索引任务一起提交，后台读取最新状态，恢复时同样重新索引。
        let mut page = 0;
        loop {
            let ids: Vec<i64> = article::Entity::find()
                .select_only()
                .column(article::Column::Id)
                .filter(article::Column::PaidColumnPublicId.eq(public_id))
                .offset(page * 100)
                .limit(100)
                .into_tuple()
                .all(&txn)
                .await?;
            if ids.is_empty() {
                break;
            }
            let now = Utc::now();
            search_job::Entity::insert_many(ids.into_iter().map(|id| search_job::ActiveModel {
                article_id: Set(id),
                action: Set(SearchAction::Upsert),
                status: Set(SearchJobStatus::Pending),
                attempts: Set(0),
                created_at: Set(now),
                updated_at: Set(now),
                ..Default::default()
            }))
            .exec(&txn)
            .await?;
            page += 1;
        }
        txn.commit().await?;
        Ok(Some(updated))
    }
    .await;
    match result {
        Ok(Some(row)) => Json(ColumnResponse::from(row)).into_response(),
        Ok(None) => error(StatusCode::NOT_FOUND, "column_not_found", "小册不存在"),
        Err(_) => error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal_error",
            "小册状态修改失败",
        ),
    }
}

/// 只删除没有章节、订单、订阅的小册；事务及外键共同保护历史权益。
#[utoipa::path(delete, path = "/api/v1/admin/columns/{public_id}", params(("public_id" = Uuid, Path)), responses((status = 204), (status = 409, body = ApiError), (status = 404, body = ApiError)), tag = "admin")]
pub async fn delete(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(public_id): Path<Uuid>,
) -> Response {
    use crate::entity::{article, column_order, column_subscription};
    use sea_orm::{PaginatorTrait, QuerySelect, TransactionTrait};
    if let Err(response) = require_admin(&state, &headers, true).await {
        return response;
    }
    let result = async {
        let txn = state.db.begin().await?;
        let Some(row) = paid_column::Entity::find()
            .filter(paid_column::Column::PublicId.eq(public_id))
            .lock_exclusive()
            .one(&txn)
            .await?
        else {
            return Ok::<_, sea_orm::DbErr>(0);
        };
        if article::Entity::find()
            .filter(article::Column::PaidColumnPublicId.eq(public_id))
            .count(&txn)
            .await?
            > 0
            || column_order::Entity::find()
                .filter(column_order::Column::PaidColumnId.eq(row.id))
                .count(&txn)
                .await?
                > 0
            || column_subscription::Entity::find()
                .filter(column_subscription::Column::PaidColumnId.eq(row.id))
                .count(&txn)
                .await?
                > 0
        {
            return Ok(2);
        }
        paid_column::Entity::delete_by_id(row.id).exec(&txn).await?;
        txn.commit().await?;
        Ok(1)
    }
    .await;
    match result {
        Ok(1) => StatusCode::NO_CONTENT.into_response(),
        Ok(0) => error(StatusCode::NOT_FOUND, "column_not_found", "小册不存在"),
        Ok(2) => error(
            StatusCode::CONFLICT,
            "column_in_use",
            "小册仍有关联文章、订单或订阅，请先处理关联；已有购买记录请使用下架",
        ),
        _ => error(
            StatusCode::CONFLICT,
            "column_in_use",
            "小册仍被使用，请刷新后重试或使用下架",
        ),
    }
}
