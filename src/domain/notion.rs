//! Notion 页面块转换为本站 Markdown 文档，不依赖 HTTP 或数据库。

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde_json::Value;
use uuid::Uuid;

/// Notion 页面中用于文章同步的公开元数据。
#[derive(Clone, Debug)]
pub struct Page {
    /// Notion 页面 ID。
    pub id: Uuid,
    /// 页面标题。
    pub title: String,
    /// 原页面链接。
    pub url: String,
    /// Notion 记录的最后编辑时间。
    pub last_edited_at: DateTime<Utc>,
}

/// 已分页读取并展开子块的 Notion 块。
#[derive(Clone, Debug)]
pub struct Block {
    /// 块 ID，图片导入后用它定位稳定素材地址。
    pub id: Uuid,
    /// Notion 块类型。
    pub kind: String,
    /// 类型对应的结构化属性。
    pub data: Value,
    /// 子块，保留 Notion 的原始顺序。
    pub children: Vec<Block>,
}

/// 转换后的 Markdown 与需要管理员检查的块类型。
pub struct ConvertedPage {
    pub source: String,
    pub summary: Option<String>,
    pub warnings: Vec<String>,
}

/// 将富文本安全地写为 Markdown；外部链接只允许 HTTP(S) 和 mailto。
fn rich_text(value: &Value) -> String {
    value
        .as_array()
        .into_iter()
        .flatten()
        .map(|part| {
            let plain = part
                .get("plain_text")
                .and_then(Value::as_str)
                .or_else(|| part.pointer("/text/content").and_then(Value::as_str))
                .unwrap_or("");
            let annotations = &part["annotations"];
            let mut rendered = escape_markdown(plain);
            if annotations["code"] == true {
                rendered = format!("`{}`", plain.replace('`', "\\`"));
            } else {
                if annotations["bold"] == true {
                    rendered = format!("**{rendered}**");
                }
                if annotations["italic"] == true {
                    rendered = format!("*{rendered}*");
                }
                if annotations["strikethrough"] == true {
                    rendered = format!("~~{rendered}~~");
                }
            }
            if let Some(href) = part.get("href").and_then(Value::as_str)
                && safe_link(href)
            {
                rendered = format!("[{rendered}]({})", href.replace(')', "%29"));
            }
            rendered
        })
        .collect()
}

/// 对纯文本中的 Markdown 控制符转义，避免 Notion 内容改变文档结构。
fn escape_markdown(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    for character in input.chars() {
        if matches!(
            character,
            '\\' | '*' | '_' | '[' | ']' | '`' | '<' | '>' | '|'
        ) {
            result.push('\\');
        }
        result.push(character);
    }
    result
}

fn safe_link(input: &str) -> bool {
    url::Url::parse(input).is_ok_and(|url| matches!(url.scheme(), "http" | "https" | "mailto"))
}

fn safe_image(input: &str) -> bool {
    url::Url::parse(input).is_ok_and(|url| matches!(url.scheme(), "http" | "https"))
        || input.starts_with("/media/") && input[7..].parse::<Uuid>().is_ok()
}

fn caption(data: &Value) -> String {
    rich_text(&data["caption"])
}

/// 将一组块转换为文章正文，所有文件型图片须先映射到本站素材地址。
pub fn convert(page: &Page, blocks: &[Block], images: &HashMap<Uuid, String>) -> ConvertedPage {
    let mut warnings = Vec::new();
    let source = render_blocks(page, blocks, images, &mut warnings);
    let summary = blocks
        .iter()
        .find(|block| block.kind == "paragraph")
        .map(|block| rich_text(&block.data["rich_text"]))
        .filter(|value| !value.trim().is_empty())
        .map(|value| value.chars().take(500).collect());
    warnings.sort();
    warnings.dedup();
    ConvertedPage {
        source,
        summary,
        warnings,
    }
}

fn render_blocks(
    page: &Page,
    blocks: &[Block],
    images: &HashMap<Uuid, String>,
    warnings: &mut Vec<String>,
) -> String {
    let mut output = String::new();
    let mut previous_list = false;
    for block in blocks {
        let rendered = render_block(page, block, images, warnings);
        if rendered.trim().is_empty() {
            continue;
        }
        let current_list = matches!(
            block.kind.as_str(),
            "bulleted_list_item" | "numbered_list_item" | "to_do"
        );
        if !output.is_empty() {
            output.push_str(if previous_list && current_list {
                "\n"
            } else {
                "\n\n"
            });
        }
        output.push_str(&rendered);
        previous_list = current_list;
    }
    output
}

fn render_block(
    page: &Page,
    block: &Block,
    images: &HashMap<Uuid, String>,
    warnings: &mut Vec<String>,
) -> String {
    let data = &block.data;
    let text = rich_text(&data["rich_text"]);
    let child_text = if block.kind == "table" {
        String::new()
    } else {
        render_blocks(page, &block.children, images, warnings)
    };
    match block.kind.as_str() {
        "paragraph" => {
            format!("{text}{}", if child_text.is_empty() { "" } else { "\n\n" }) + &child_text
        }
        "heading_1" => format!("# {text}"),
        "heading_2" => format!("## {text}"),
        "heading_3" => format!("### {text}"),
        "bulleted_list_item" | "numbered_list_item" | "to_do" => {
            let marker = match block.kind.as_str() {
                "numbered_list_item" => "1. ",
                "to_do" if data["checked"] == true => "- [x] ",
                "to_do" => "- [ ] ",
                _ => "- ",
            };
            if child_text.is_empty() {
                format!("{marker}{text}")
            } else {
                format!("{marker}{text}\n{}", indent(&child_text, "  "))
            }
        }
        "quote" | "callout" => {
            let body = if child_text.is_empty() {
                text
            } else {
                format!("{text}\n\n{child_text}")
            };
            indent(&body, "> ")
        }
        "toggle" => format!("**{text}**\n\n{child_text}"),
        "code" => {
            let language = data["language"]
                .as_str()
                .unwrap_or("text")
                .split_whitespace()
                .next()
                .unwrap_or("text");
            let raw = data["rich_text"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|part| part["plain_text"].as_str())
                .collect::<String>();
            let fence = "`".repeat(3 + raw.matches("```").count());
            format!("{fence}{language}\n{raw}\n{fence}")
        }
        "divider" => "---".into(),
        "image" => {
            let url = images
                .get(&block.id)
                .map(String::as_str)
                .or_else(|| data.pointer("/external/url").and_then(Value::as_str));
            match url.filter(|url| safe_image(url)) {
                Some(url) => format!("![{}]({})", caption(data), url.replace(')', "%29")),
                None => {
                    warnings.push("图片未能转存到素材库".into());
                    String::new()
                }
            }
        }
        "bookmark" | "link_preview" | "embed" | "video" | "audio" => {
            let url = data["url"].as_str().unwrap_or("");
            if safe_link(url) {
                let label = caption(data);
                format!(
                    "[{}]({})",
                    if label.is_empty() { url } else { &label },
                    url.replace(')', "%29")
                )
            } else {
                warnings.push(format!("{} 链接无法导入", block.kind));
                String::new()
            }
        }
        "child_page" | "child_database" => {
            let title = data["title"].as_str().unwrap_or("Notion 页面");
            format!(
                "[{}](https://www.notion.so/{})",
                escape_markdown(title),
                block.id.simple()
            )
        }
        "file" | "pdf" => {
            let name = data["name"].as_str().unwrap_or("Notion 附件");
            format!(
                "[{}]({}#{})",
                escape_markdown(name),
                page.url,
                block.id.simple()
            )
        }
        "table" => render_table(block),
        "column_list" | "column" | "synced_block" => child_text,
        "table_of_contents" | "breadcrumb" => String::new(),
        _ => {
            warnings.push(format!("未映射的 Notion 块：{}", block.kind));
            if text.is_empty() {
                child_text
            } else {
                format!("{text}\n\n{child_text}")
            }
        }
    }
}

fn render_table(block: &Block) -> String {
    let rows: Vec<Vec<String>> = block
        .children
        .iter()
        .filter(|row| row.kind == "table_row")
        .map(|row| {
            row.data["cells"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|cell| rich_text(cell).replace('|', "\\|").replace('\n', " "))
                .collect()
        })
        .collect();
    let Some(first) = rows.first() else {
        return String::new();
    };
    let columns = first.len();
    if columns == 0 {
        return String::new();
    }
    let mut output = format!(
        "| {} |\n| {} |",
        first.join(" | "),
        vec!["---"; columns].join(" | ")
    );
    for row in rows.iter().skip(1) {
        output.push_str(&format!("\n| {} |", row.join(" | ")));
    }
    output
}

fn indent(text: &str, prefix: &str) -> String {
    text.lines()
        .map(|line| format!("{prefix}{line}"))
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn page() -> Page {
        Page {
            id: Uuid::nil(),
            title: "文章".into(),
            url: "https://www.notion.so/example".into(),
            last_edited_at: Utc::now(),
        }
    }

    fn block(kind: &str, data: Value) -> Block {
        Block {
            id: Uuid::nil(),
            kind: kind.into(),
            data,
            children: Vec::new(),
        }
    }

    #[test]
    fn converts_rich_text_links_lists_and_code() {
        let blocks = vec![
            block("heading_2", json!({"rich_text":[{"plain_text":"标题"}]})),
            block(
                "paragraph",
                json!({"rich_text":[{"plain_text":"链接", "href":"https://example.com", "annotations":{"bold":true}}]}),
            ),
            block(
                "to_do",
                json!({"rich_text":[{"plain_text":"完成"}],"checked":true}),
            ),
            block(
                "code",
                json!({"rich_text":[{"plain_text":"fn main() {}"}],"language":"rust"}),
            ),
        ];
        let result = convert(&page(), &blocks, &HashMap::new());
        assert!(result.source.contains("## 标题"));
        assert!(result.source.contains("[**链接**](https://example.com)"));
        assert!(result.source.contains("- [x] 完成"));
        assert!(result.source.contains("```rust\nfn main() {}\n```"));
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn keeps_only_stable_image_urls() {
        let image = block(
            "image",
            json!({"type":"file","file":{"url":"https://temporary.example/img"}}),
        );
        let result = convert(&page(), std::slice::from_ref(&image), &HashMap::new());
        assert!(result.source.is_empty());
        assert!(!result.warnings.is_empty());
        let result = convert(
            &page(),
            &[image],
            &HashMap::from([(Uuid::nil(), format!("/media/{}", Uuid::new_v4()))]),
        );
        assert!(result.source.starts_with("![](/media/"));
    }

    #[test]
    fn keeps_adjacent_list_items_in_one_markdown_list() {
        let blocks = vec![
            block(
                "numbered_list_item",
                json!({"rich_text":[{"plain_text":"第一项"}]}),
            ),
            block(
                "numbered_list_item",
                json!({"rich_text":[{"plain_text":"第二项"}]}),
            ),
        ];
        let result = convert(&page(), &blocks, &HashMap::new());
        assert_eq!(result.source, "1. 第一项\n1. 第二项");
    }

    #[test]
    fn converts_table_without_warning_for_rows() {
        let mut table = block("table", json!({}));
        table.children = vec![
            block("table_row", json!({"cells":[[{"plain_text":"名称"}]]})),
            block("table_row", json!({"cells":[[{"plain_text":"示例"}]]})),
        ];
        let result = convert(&page(), &[table], &HashMap::new());
        assert_eq!(result.source, "| 名称 |\n| --- |\n| 示例 |");
        assert!(result.warnings.is_empty());
    }
}
