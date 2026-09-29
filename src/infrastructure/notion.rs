//! Notion API 适配器：仅服务端持有密钥，分页读取页面和子块。

use std::{sync::Arc, time::Duration};

use anyhow::{Context, Result, bail};
use chrono::{DateTime, Utc};
use reqwest::{Client, Method, StatusCode};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::domain::notion::{Block, Page};

const API_BASE: &str = "https://api.notion.com/v1";
const API_VERSION: &str = "2022-06-28";
const MAX_BLOCKS: usize = 500;
const MAX_DEPTH: usize = 6;

/// 一页搜索结果及后续分页游标。
pub struct SearchResult {
    pub pages: Vec<Page>,
    pub next_cursor: Option<String>,
}

/// 只接受固定 Notion API 主机的 HTTP 客户端。
pub struct NotionClient {
    client: Client,
    key: Arc<str>,
}

impl NotionClient {
    /// 设置连接与响应超时，不输出密钥到日志。
    pub fn new(key: Arc<str>) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(20))
            .redirect(reqwest::redirect::Policy::none())
            .build()?;
        Ok(Self { client, key })
    }

    /// 搜索集成已授权的页面，每次最多返回 50 篇。
    pub async fn search(&self, query: &str, cursor: Option<&str>) -> Result<SearchResult> {
        let mut body = json!({
            "page_size": 50,
            "sort": {"direction": "descending", "timestamp": "last_edited_time"},
            "filter": {"property": "object", "value": "page"}
        });
        if !query.trim().is_empty() {
            body["query"] = query.trim().into();
        }
        if let Some(cursor) = cursor {
            body["start_cursor"] = cursor.into();
        }
        let value = self.request(Method::POST, "/search", Some(&body)).await?;
        let pages = value["results"]
            .as_array()
            .context("Notion 搜索结果格式无效")?
            .iter()
            .filter_map(|value| parse_page(value).ok())
            .collect();
        Ok(SearchResult {
            pages,
            next_cursor: value["next_cursor"].as_str().map(str::to_owned),
        })
    }

    /// 读取单个已共享页面的元数据。
    pub async fn page(&self, id: Uuid) -> Result<Page> {
        let value = self
            .request(Method::GET, &format!("/pages/{id}"), None)
            .await?;
        parse_page(&value)
    }

    /// 递归获取页面块并限制总量与深度，避免超大页面耗尽资源。
    pub async fn blocks(&self, id: Uuid) -> Result<Vec<Block>> {
        let mut remaining = MAX_BLOCKS;
        self.block_children(id, 0, &mut remaining).await
    }

    async fn block_children(
        &self,
        id: Uuid,
        depth: usize,
        remaining: &mut usize,
    ) -> Result<Vec<Block>> {
        if depth > MAX_DEPTH {
            bail!("Notion 页面嵌套超过 {MAX_DEPTH} 层");
        }
        let mut blocks = Vec::new();
        let mut cursor: Option<String> = None;
        loop {
            let mut path = format!("/blocks/{id}/children?page_size=100");
            if let Some(cursor) = &cursor {
                path.push_str("&start_cursor=");
                path.push_str(
                    &url::form_urlencoded::byte_serialize(cursor.as_bytes()).collect::<String>(),
                );
            }
            let value = self.request(Method::GET, &path, None).await?;
            let results = value["results"]
                .as_array()
                .context("Notion 块列表格式无效")?;
            if results.len() > *remaining {
                bail!("Notion 页面超过 {MAX_BLOCKS} 个块");
            }
            *remaining -= results.len();
            for raw in results {
                let block_id = raw["id"].as_str().context("Notion 块 ID 缺失")?.parse()?;
                let kind = raw["type"]
                    .as_str()
                    .context("Notion 块类型缺失")?
                    .to_owned();
                let children = if raw["has_children"] == true
                    && kind != "child_page"
                    && kind != "child_database"
                {
                    Box::pin(self.block_children(block_id, depth + 1, remaining)).await?
                } else {
                    Vec::new()
                };
                blocks.push(Block {
                    id: block_id,
                    data: raw[&kind].clone(),
                    kind,
                    children,
                });
            }
            cursor = value["next_cursor"].as_str().map(str::to_owned);
            if cursor.is_none() {
                break;
            }
        }
        Ok(blocks)
    }

    async fn request(&self, method: Method, path: &str, body: Option<&Value>) -> Result<Value> {
        let mut builder = self
            .client
            .request(method, format!("{API_BASE}{path}"))
            .bearer_auth(&self.key)
            .header("Notion-Version", API_VERSION);
        if let Some(body) = body {
            builder = builder.json(body);
        }
        let response = builder.send().await.context("Notion 请求失败")?;
        let status = response.status();
        if !status.is_success() {
            if status == StatusCode::TOO_MANY_REQUESTS {
                bail!("Notion API 限流，请稍后重试");
            }
            bail!("Notion API 返回 HTTP {}", status.as_u16());
        }
        response.json().await.context("Notion JSON 响应无效")
    }
}

/// 数据库页面的标题属性名称可自定义，因此按属性类型寻找 title。
fn parse_page(value: &Value) -> Result<Page> {
    let id = value["id"]
        .as_str()
        .context("Notion 页面 ID 缺失")?
        .parse()?;
    let title = value["properties"]
        .as_object()
        .and_then(|properties| {
            properties
                .values()
                .find(|property| property["type"] == "title")
        })
        .and_then(|property| property["title"].as_array())
        .map(|parts| {
            parts
                .iter()
                .filter_map(|part| part["plain_text"].as_str())
                .collect::<String>()
        })
        .filter(|title| !title.trim().is_empty())
        .unwrap_or_else(|| "未命名 Notion 页面".into());
    let last_edited_at: DateTime<Utc> = value["last_edited_time"]
        .as_str()
        .context("Notion 编辑时间缺失")?
        .parse()?;
    let url = value["url"]
        .as_str()
        .context("Notion 页面地址缺失")?
        .to_owned();
    Ok(Page {
        id,
        title,
        url,
        last_edited_at,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_title_property_regardless_of_name() {
        let value = json!({
            "id": "00000000-0000-0000-0000-000000000001",
            "url": "https://www.notion.so/example",
            "last_edited_time": "2026-09-29T00:00:00Z",
            "properties": {"文章名称": {"type": "title", "title": [{"plain_text": "Notion 标题"}]}}
        });
        assert_eq!(parse_page(&value).unwrap().title, "Notion 标题");
    }
}
