//! 专栏目录仅公开元信息，付费正文通过文章详情单独授权。

use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
use serde::Serialize;
use utoipa::ToSchema;

use crate::entity::{article, paid_column, status::ArticleStatus};

use super::{
    super::{ApiError, AppState, ArticleSummaryResponse, error},
    reader,
};

/// 可公开展示的专栏资料，不含内部 ID 与订单信息。
#[derive(Serialize, ToSchema)]
pub struct ColumnResponse {
    /// 对外 UUID。
    pub public_id: uuid::Uuid,
    /// 永久链接标识。
    pub slug: String,
    /// 专栏名称。
    pub title: String,
    /// 专栏简介。
    pub description: String,
    /// 人民币价格，单位为分。
    pub price_cents: i32,
    /// 是否公开展示和接受新购买。
    pub visible: bool,
}

impl From<paid_column::Model> for ColumnResponse {
    fn from(row: paid_column::Model) -> Self {
        Self {
            public_id: row.public_id,
            slug: row.slug,
            title: row.title,
            description: row.description,
            price_cents: row.price_cents,
            visible: row.visible,
        }
    }
}

/// 专栏详情携带章节元数据与当前读者的订阅状态。
#[derive(Serialize, ToSchema)]
pub struct ColumnDetailResponse {
    /// 公开专栏资料。
    #[serde(flatten)]
    pub column: ColumnResponse,
    /// 已发布章节，按首次发布时间排序。
    pub articles: Vec<ArticleSummaryResponse>,
    /// 当前读者是否已购买本专栏。
    pub subscribed: bool,
    /// 商户资料齐全后才允许读者下单。
    pub payment_available: bool,
}

/// 公开专栏列表。
#[utoipa::path(get, path = "/api/v1/columns", responses((status = 200, body = Vec<ColumnResponse>), (status = 500, body = ApiError)), tag = "columns")]
pub async fn list(State(state): State<AppState>) -> Response {
    match paid_column::Entity::find()
        .filter(paid_column::Column::Visible.eq(true))
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
        Err(err) => {
            tracing::error!(error = %err, "读取专栏列表失败");
            error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "服务暂时不可用",
            )
        }
    }
}

/// 公开章节仅投影标题与摘要，订阅状态由服务端校验。
#[utoipa::path(get, path = "/api/v1/columns/{slug}", params(("slug" = String, Path)), responses((status = 200, body = ColumnDetailResponse), (status = 404, body = ApiError)), tag = "columns")]
pub async fn detail(
    State(state): State<AppState>,
    Path(slug): Path<String>,
    headers: HeaderMap,
) -> Response {
    let column = match paid_column::Entity::find()
        .filter(paid_column::Column::Slug.eq(slug))
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
    let session = match reader::get_session(&state, &headers).await {
        Ok(session) => session,
        Err(err) => {
            tracing::error!(error = %err, "读取读者会话失败");
            return error(
                StatusCode::SERVICE_UNAVAILABLE,
                "dependency_unavailable",
                "服务暂时不可用",
            );
        }
    };
    let subscribed = if let Some(session) = session {
        match reader::owns_column(&state, session.reader_id, column.id).await {
            Ok(value) => value,
            Err(err) => {
                tracing::error!(error = %err, "校验专栏权益失败");
                return error(
                    StatusCode::SERVICE_UNAVAILABLE,
                    "dependency_unavailable",
                    "服务暂时不可用",
                );
            }
        }
    } else {
        false
    };
    if !column.visible && !subscribed {
        return error(StatusCode::NOT_FOUND, "column_not_found", "小册不存在");
    }
    let articles = match article::Entity::find()
        .filter(article::Column::PaidColumnPublicId.eq(column.public_id))
        .filter(article::Column::Status.eq(ArticleStatus::Published))
        .order_by_asc(article::Column::PublishedAt)
        .all(&state.db)
        .await
    {
        Ok(rows) => rows,
        Err(err) => {
            tracing::error!(error = %err, "读取专栏章节失败");
            return error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "服务暂时不可用",
            );
        }
    };
    let mut response = Json(ColumnDetailResponse {
        column: column.into(),
        articles: articles
            .into_iter()
            .map(|row| ArticleSummaryResponse {
                public_id: row.public_id,
                slug: row.slug,
                title: row.title,
                summary: row.summary,
                cover_url: None,
                published_at: row.published_at,
                paid_column_public_id: row.paid_column_public_id,
                subscriber_only: row.subscriber_only,
            })
            .collect(),
        subscribed,
        payment_available: state.wechat_pay.is_some(),
    })
    .into_response();
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, "private, no-store".parse().unwrap());
    response
        .headers_mut()
        .insert(header::VARY, "Cookie".parse().unwrap());
    response
}
