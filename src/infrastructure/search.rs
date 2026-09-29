//! Jieba 分词与 Tantivy 持久化索引；索引始终可以从 PostgreSQL 重建。

use std::{path::Path, sync::Arc};

use anyhow::{Context, Result};
use jieba_rs::Jieba;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, IntoActiveModel, QueryFilter,
    QueryOrder, QuerySelect, Set,
};
use tantivy::{
    Index, IndexReader, TantivyDocument, Term,
    collector::{Count, TopDocs},
    directory::MmapDirectory,
    query::{BooleanQuery, BoostQuery, Occur, Query, TermQuery},
    schema::{Field, IndexRecordOption, STORED, STRING, Schema, TEXT, Value},
};

use crate::entity::{
    article, search_job,
    status::{ArticleStatus, SearchAction, SearchJobStatus},
};

/// 搜索字段和 reader 由整个进程共享；写入仅由单个后台任务执行。
pub struct SearchEngine {
    index: Index,
    reader: IndexReader,
    jieba: Jieba,
    id: Field,
    title: Field,
    summary: Field,
    body: Field,
}

impl SearchEngine {
    /// 在本地目录打开或创建索引，不将索引用作业务真相来源。
    pub fn open(path: &Path) -> Result<Self> {
        std::fs::create_dir_all(path).context("创建搜索索引目录失败")?;
        let directory = MmapDirectory::open(path).context("打开搜索索引目录失败")?;
        Self::new(Index::open_or_create(directory, schema())?)
    }

    /// 供单元测试使用的内存索引。
    #[cfg(test)]
    pub fn in_memory() -> Self {
        Self::new(Index::create_in_ram(schema())).expect("测试索引应可创建")
    }

    /// 从索引元数据取得字段，防止已有目录与当前 schema 不匹配。
    fn new(index: Index) -> Result<Self> {
        let schema = index.schema();
        let id = schema.get_field("id")?;
        let title = schema.get_field("title")?;
        let summary = schema.get_field("summary")?;
        let body = schema.get_field("body")?;
        let reader = index.reader()?;
        Ok(Self {
            index,
            reader,
            jieba: Jieba::new(),
            id,
            title,
            summary,
            body,
        })
    }

    /// 索引单篇文章，重试时先删除旧文档保证幂等。
    pub fn upsert(&self, row: article::Model) -> Result<()> {
        self.upsert_batch(vec![row])
    }

    /// 批量写入文章，重建索引时复用一个有界 Tantivy writer。
    pub fn upsert_batch(&self, rows: Vec<article::Model>) -> Result<()> {
        let mut writer = self.index.writer::<TantivyDocument>(20_000_000)?;
        for row in rows {
            writer.delete_term(Term::from_field_text(self.id, &row.id.to_string()));
            if row.status == ArticleStatus::Published {
                let body = document_text(&row.document);
                let mut document = TantivyDocument::default();
                document.add_text(self.id, row.id.to_string());
                document.add_text(self.title, self.tokenize(&row.title));
                document.add_text(
                    self.summary,
                    self.tokenize(row.summary.as_deref().unwrap_or("")),
                );
                document.add_text(self.body, self.tokenize(&body));
                writer.add_document(document)?;
            }
        }
        writer.commit()?;
        self.reader.reload()?;
        Ok(())
    }

    /// 清空旧索引，仅用于离线重建命令。
    pub fn clear(&self) -> Result<()> {
        let mut writer = self.index.writer::<TantivyDocument>(20_000_000)?;
        writer.delete_all_documents()?;
        writer.commit()?;
        self.reader.reload()?;
        Ok(())
    }

    /// 删除已撤回或已删除文章的索引文档。
    pub fn delete(&self, id: i64) -> Result<()> {
        let mut writer = self.index.writer::<TantivyDocument>(20_000_000)?;
        writer.delete_term(Term::from_field_text(self.id, &id.to_string()));
        writer.commit()?;
        self.reader.reload()?;
        Ok(())
    }

    /// 对查询做同样的 Jieba 分词，按 Tantivy 相关度返回内部 ID。
    pub fn search(&self, text: &str, page: u64, per_page: u64) -> Result<(usize, Vec<i64>)> {
        let normalized = text.to_lowercase();
        let tokens: Vec<_> = self
            .jieba
            .cut(&normalized, true)
            .into_iter()
            .filter(|token| token.word.chars().any(char::is_alphanumeric))
            .take(12)
            .collect();
        if tokens.is_empty() {
            return Ok((0, Vec::new()));
        }
        let mut terms: Vec<(Occur, Box<dyn Query>)> = Vec::new();
        for token in tokens {
            let title = TermQuery::new(
                Term::from_field_text(self.title, token.word),
                IndexRecordOption::Basic,
            );
            terms.push((
                Occur::Should,
                Box::new(BoostQuery::new(Box::new(title), 4.0)),
            ));
            for field in [self.summary, self.body] {
                terms.push((
                    Occur::Should,
                    Box::new(TermQuery::new(
                        Term::from_field_text(field, token.word),
                        IndexRecordOption::Basic,
                    )),
                ));
            }
        }
        let query = BooleanQuery::new(terms);
        let searcher = self.reader.searcher();
        let total = searcher.search(&query, &Count)?;
        let top = searcher.search(
            &query,
            &TopDocs::with_limit(per_page as usize)
                .and_offset(((page - 1) * per_page) as usize)
                .order_by_score(),
        )?;
        let ids = top
            .into_iter()
            .filter_map(|(_, address)| {
                searcher
                    .doc::<TantivyDocument>(address)
                    .ok()
                    .and_then(|doc| {
                        doc.get_first(self.id)
                            .and_then(|value| value.as_str())
                            .and_then(|text| text.parse().ok())
                    })
            })
            .collect();
        Ok((total, ids))
    }

    /// 用空格分隔词语，使 Tantivy 默认分词器保留 Jieba 的词边界。
    fn tokenize(&self, text: &str) -> String {
        let normalized = text.to_lowercase();
        self.jieba
            .cut(&normalized, true)
            .into_iter()
            .map(|token| token.word)
            .collect::<Vec<_>>()
            .join(" ")
    }
}

/// 服务器启动前从 PostgreSQL 分页全量重建 Tantivy 索引。
pub async fn rebuild(db: &DatabaseConnection, engine: Arc<SearchEngine>) -> Result<u64> {
    let clear = engine.clone();
    tokio::task::spawn_blocking(move || clear.clear()).await??;
    let mut page = 0_u64;
    let mut total = 0_u64;
    loop {
        let rows = article::Entity::find()
            .filter(article::Column::Status.eq(ArticleStatus::Published))
            .order_by_asc(article::Column::Id)
            .limit(100)
            .offset(page * 100)
            .all(db)
            .await?;
        if rows.is_empty() {
            break;
        }
        total += rows.len() as u64;
        let writer = engine.clone();
        tokio::task::spawn_blocking(move || writer.upsert_batch(rows)).await??;
        page += 1;
    }
    Ok(total)
}

/// 首版索引只保留搜索所需字段和稳定的内部关联标识。
fn schema() -> Schema {
    let mut schema = Schema::builder();
    schema.add_text_field("id", STRING | STORED);
    schema.add_text_field("title", TEXT);
    schema.add_text_field("summary", TEXT);
    schema.add_text_field("body", TEXT);
    schema.build()
}

/// 提取 Tiptap 文本节点，避免把 HTML 标签索引为词语。
fn document_text(value: &serde_json::Value) -> String {
    fn visit(value: &serde_json::Value, output: &mut String) {
        if let Some(text) = value.get("text").and_then(serde_json::Value::as_str) {
            output.push_str(text);
            output.push(' ');
        }
        if let Some(children) = value.get("content").and_then(serde_json::Value::as_array) {
            for child in children {
                visit(child, output);
            }
        }
    }
    let mut output = String::new();
    visit(value, &mut output);
    output
}

/// 单个后台循环按顺序消费索引任务，失败后最多重试三次。
pub async fn run_worker(db: DatabaseConnection, engine: Arc<SearchEngine>) {
    let mut interval = tokio::time::interval(std::time::Duration::from_secs(2));
    loop {
        interval.tick().await;
        let jobs = match search_job::Entity::find()
            .filter(search_job::Column::Status.eq(SearchJobStatus::Pending))
            .order_by_asc(search_job::Column::Id)
            .limit(20)
            .all(&db)
            .await
        {
            Ok(rows) => rows,
            Err(err) => {
                tracing::error!(error = %err, "读取搜索任务失败");
                continue;
            }
        };
        for job in jobs {
            let row = match article::Entity::find_by_id(job.article_id).one(&db).await {
                Ok(row) => row,
                Err(err) => {
                    tracing::error!(error = %err, "读取待索引文章失败");
                    continue;
                }
            };
            let engine = engine.clone();
            let action = job.action;
            let article_id = job.article_id;
            let result = tokio::task::spawn_blocking(move || match (action, row) {
                (SearchAction::Upsert, Some(row)) => engine.upsert(row),
                _ => engine.delete(article_id),
            })
            .await;
            let mut active = job.into_active_model();
            active.attempts = Set(active.attempts.as_ref().to_owned() + 1);
            active.updated_at = Set(chrono::Utc::now());
            active.status = Set(if result.is_ok_and(|result| result.is_ok()) {
                SearchJobStatus::Done
            } else if active.attempts.as_ref() >= &3 {
                SearchJobStatus::Failed
            } else {
                SearchJobStatus::Pending
            });
            if let Err(err) = active.update(&db).await {
                tracing::error!(error = %err, "保存搜索任务状态失败");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chinese_terms_survive_index_roundtrip() {
        let engine = SearchEngine::in_memory();
        let now = chrono::Utc::now();
        let row = article::Model {
            id: 1,
            public_id: uuid::Uuid::new_v4(),
            slug: "rust-search".into(),
            title: "中文分词搜索".into(),
            summary: Some("使用 Jieba".into()),
            document: serde_json::json!({"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"博客文章"}]}]}),
            rendered_html: "<p>博客文章</p>".into(),
            status: ArticleStatus::Published,
            cover_asset_id: None,
            published_at: Some(now),
            created_at: now,
            updated_at: now,
        };
        engine.upsert(row).unwrap();
        assert_eq!(engine.search("分词", 1, 20).unwrap().1, vec![1]);
        engine.delete(1).unwrap();
        assert!(engine.search("分词", 1, 20).unwrap().1.is_empty());
    }
}
