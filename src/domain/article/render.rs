//! 将 Markdown 原文或旧版 Tiptap 文档转换为可公开展示的 HTML。

use anyhow::{Result, bail};
use pulldown_cmark::{Event, Options, Parser, html};
use serde_json::Value;

/// 公开文章大纲只含二、三级标题的文字；不携带标题内链接、属性或正文节点。
pub fn heading_outline(html: &str) -> Vec<(u8, String)> {
    let fragment = scraper::Html::parse_fragment(html);
    let selector = scraper::Selector::parse("h2, h3").expect("静态标题选择器有效");
    fragment
        .select(&selector)
        .map(|heading| {
            let level = if heading.value().name() == "h2" { 2 } else { 3 };
            (level, heading.text().collect::<String>().trim().to_owned())
        })
        .collect()
}

/// 从已净化的正文按可见文字数量生成前 30% 试看；不接受客户端指定截取范围。
/// 使用 HTML 树裁剪并重新配对标签，截断后的文字、节点和属性均不会进入响应。
pub fn preview_html(html: &str) -> String {
    let fragment = scraper::Html::parse_fragment(html);
    let root = fragment.root_element();
    let total = root.text().map(|text| text.chars().count()).sum::<usize>();
    let mut remaining = total * 3 / 10;
    let mut output = String::new();
    preview_children(root, &mut remaining, &mut output);
    output
}

/// 遍历正文树的前缀；预算耗尽后停止遍历，但始终闭合已输出的容器。
fn preview_children(element: scraper::ElementRef<'_>, remaining: &mut usize, output: &mut String) {
    for child in element.children() {
        if *remaining == 0 {
            break;
        }
        match child.value() {
            scraper::Node::Text(text) => {
                let prefix: String = text.chars().take(*remaining).collect();
                *remaining -= prefix.chars().count();
                output.push_str(&html_escape::encode_safe(&prefix));
            }
            scraper::Node::Element(_) => {
                let Some(child) = scraper::ElementRef::wrap(child) else {
                    continue;
                };
                let name = child.value().name();
                output.push('<');
                output.push_str(name);
                // 只保留排版与链接所需属性，避免 title 等不可见文本透露正文。
                for (key, value) in child.value().attrs() {
                    if matches!(
                        key,
                        "href" | "src" | "class" | "type" | "disabled" | "checked"
                    ) {
                        output.push(' ');
                        output.push_str(key);
                        output.push_str("=\"");
                        output.push_str(&html_escape::encode_double_quoted_attribute(value));
                        output.push('"');
                    }
                }
                output.push('>');
                if !matches!(name, "img" | "br" | "hr" | "input") {
                    preview_children(child, remaining, output);
                    output.push_str("</");
                    output.push_str(name);
                    output.push('>');
                }
            }
            _ => {}
        }
    }
}

/// 限制单篇文章的输入和输出大小，防止深度嵌套消耗过多内存。
pub fn render_document(document: &Value) -> Result<String> {
    if serde_json::to_vec(document)?.len() > 256 * 1024 {
        bail!("文章文档过大");
    }
    if document.get("type").and_then(Value::as_str) == Some("markdown") {
        let source = document
            .get("source")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow::anyhow!("Markdown 正文缺失"))?;
        return render_markdown(source);
    }
    if document.get("type").and_then(Value::as_str) != Some("doc") {
        bail!("文章文档根节点必须是 doc");
    }
    let mut output = String::new();
    render_children(document, &mut output, 0)?;
    if output.len() > 512 * 1024 {
        bail!("文章 HTML 过大");
    }
    Ok(output)
}

/// 渲染 Markdown 原文；原生 HTML 作为文字显示，生成的 HTML 再执行白名单净化。
fn render_markdown(source: &str) -> Result<String> {
    let options =
        Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS;
    let parser = Parser::new_ext(source, options).map(|event| match event {
        Event::Html(raw) | Event::InlineHtml(raw) => Event::Text(raw),
        other => other,
    });
    let mut html_output = String::new();
    html::push_html(&mut html_output, parser);
    let clean = ammonia::Builder::default()
        .add_generic_attributes(&["class"])
        .add_tags(&["img", "input"])
        .add_tag_attributes("input", &["type", "disabled", "checked"])
        .add_tag_attributes("img", &["src", "alt", "title"])
        .clean(&html_output)
        .to_string();
    if clean.len() > 512 * 1024 {
        bail!("文章 HTML 过大");
    }
    Ok(clean)
}

/// 递归渲染节点；只允许显式列出的结构和标记。
fn render_children(node: &Value, output: &mut String, depth: usize) -> Result<()> {
    if depth > 32 {
        bail!("文章嵌套层级过深");
    }
    let Some(children) = node.get("content").and_then(Value::as_array) else {
        return Ok(());
    };
    for child in children {
        render_node(child, output, depth + 1)?;
    }
    Ok(())
}

/// 对链接协议做白名单校验，避免 `javascript:` 等可执行地址。
fn safe_url(value: &str) -> Option<&str> {
    if value.starts_with('/') && !value.starts_with("//") {
        return Some(value);
    }
    let parsed = url::Url::parse(value).ok()?;
    matches!(parsed.scheme(), "http" | "https").then_some(value)
}

/// 转换单个 Tiptap 节点并转义所有来自编辑器的文本和属性。
fn render_node(node: &Value, output: &mut String, depth: usize) -> Result<()> {
    let kind = node.get("type").and_then(Value::as_str).unwrap_or("");
    match kind {
        "text" => {
            let text = node.get("text").and_then(Value::as_str).unwrap_or("");
            let marks = node.get("marks").and_then(Value::as_array);
            let mut opened = Vec::new();
            if let Some(marks) = marks {
                for mark in marks {
                    match mark.get("type").and_then(Value::as_str) {
                        Some("bold") => {
                            output.push_str("<strong>");
                            opened.push("strong");
                        }
                        Some("italic") => {
                            output.push_str("<em>");
                            opened.push("em");
                        }
                        Some("code") => {
                            output.push_str("<code>");
                            opened.push("code");
                        }
                        Some("strike") => {
                            output.push_str("<s>");
                            opened.push("s");
                        }
                        Some("link") => {
                            let href = mark
                                .get("attrs")
                                .and_then(|attrs| attrs.get("href"))
                                .and_then(Value::as_str);
                            if let Some(href) = href.and_then(safe_url) {
                                output.push_str("<a rel=\"noopener noreferrer\" href=\"");
                                output.push_str(&html_escape::encode_double_quoted_attribute(href));
                                output.push_str("\">");
                                opened.push("a");
                            }
                        }
                        _ => {}
                    }
                }
            }
            output.push_str(&html_escape::encode_safe(text));
            for tag in opened.into_iter().rev() {
                output.push_str("</");
                output.push_str(tag);
                output.push('>');
            }
        }
        "paragraph" => wrap(node, output, depth, "p")?,
        "blockquote" => wrap(node, output, depth, "blockquote")?,
        "bulletList" => wrap(node, output, depth, "ul")?,
        "orderedList" => wrap(node, output, depth, "ol")?,
        "listItem" => wrap(node, output, depth, "li")?,
        "codeBlock" => {
            output.push_str("<pre><code>");
            render_children(node, output, depth)?;
            output.push_str("</code></pre>");
        }
        "heading" => {
            let level = node
                .get("attrs")
                .and_then(|attrs| attrs.get("level"))
                .and_then(Value::as_u64)
                .unwrap_or(2)
                .clamp(2, 6);
            let tag = format!("h{level}");
            wrap(node, output, depth, &tag)?;
        }
        "hardBreak" => output.push_str("<br>"),
        "horizontalRule" => output.push_str("<hr>"),
        "image" => {
            let attrs = node.get("attrs");
            let src = attrs
                .and_then(|attrs| attrs.get("src"))
                .and_then(Value::as_str);
            if let Some(src) = src
                .filter(|src| src.starts_with("/media/") && src[7..].parse::<uuid::Uuid>().is_ok())
            {
                let alt = attrs
                    .and_then(|attrs| attrs.get("alt"))
                    .and_then(Value::as_str)
                    .unwrap_or("");
                output.push_str("<img loading=\"lazy\" src=\"");
                output.push_str(&html_escape::encode_double_quoted_attribute(src));
                output.push_str("\" alt=\"");
                output.push_str(&html_escape::encode_double_quoted_attribute(alt));
                output.push_str("\">");
            }
        }
        _ => bail!("不支持的文章节点: {kind}"),
    }
    Ok(())
}

/// 统一处理容器节点，确保开闭标签始终配对。
fn wrap(node: &Value, output: &mut String, depth: usize, tag: &str) -> Result<()> {
    output.push('<');
    output.push_str(tag);
    output.push('>');
    render_children(node, output, depth)?;
    output.push_str("</");
    output.push_str(tag);
    output.push('>');
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// 中文按字符裁剪，后续节点与隐藏属性不能泄露，嵌套标签必须闭合。
    #[test]
    fn preview_limits_visible_text_and_excludes_hidden_content() {
        let html = "<p title=\"SECRET\"><strong>一二三四五六七八九十</strong></p><p>SECRET</p>";
        assert_eq!(preview_html(html), "<p><strong>一二三四</strong></p>");
        assert_eq!(preview_html("<p>一</p>"), "");
        assert_eq!(
            preview_html("<p>1234567890</p><img src=\"/secret\">"),
            "<p>123</p>"
        );
    }

    /// 完整大纲仅输出标题，标题属性和随后的隐藏正文不会进入公开数据。
    #[test]
    fn outline_contains_only_heading_text() {
        assert_eq!(
            heading_outline("<h2 title=\"secret\">第一节</h2><p>secret</p><h3>第二节</h3>"),
            vec![(2, "第一节".into()), (3, "第二节".into())]
        );
    }

    #[test]
    fn escapes_untrusted_text_and_rejects_unknown_nodes() {
        let document = json!({"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"<script>alert(1)</script>"}]}]});
        assert_eq!(
            render_document(&document).unwrap(),
            "<p>&lt;script&gt;alert(1)&lt;&#x2F;script&gt;</p>"
        );
        assert!(render_document(&json!({"type":"doc","content":[{"type":"iframe"}]})).is_err());
    }

    #[test]
    fn markdown_renders_headings_code_and_escapes_html() {
        let document = json!({"type":"markdown","source":"## 标题\n\n```rust\nfn main() {}\n```\n\n<script>alert(1)</script>"});
        let html = render_document(&document).unwrap();
        assert!(html.contains("<h2>标题</h2>"));
        assert!(html.contains("class=\"language-rust\""));
        assert!(!html.contains("<script>"));
    }

    #[test]
    fn markdown_sanitizes_links_and_preserves_tables_and_tasks() {
        let document = json!({"type":"markdown","source":"[危险](javascript:alert%281%29)\n\n- [x] 已完成\n\n| 项目 | 值 |\n| --- | --- |\n| a | 1 |"});
        let html = render_document(&document).unwrap();
        assert!(!html.contains("href=\"javascript:"));
        assert!(html.contains("disabled"));
        assert!(html.contains("checked"));
        assert!(html.contains("<table>"));
        assert!(
            render_document(&json!({"type":"markdown","source":"a".repeat(256 * 1024)})).is_err()
        );
    }
}
