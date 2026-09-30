//! 使用 Readability 内容评分识别正文，再统一清理和转换；不依赖站点主题或类名。

use super::{FetchFormat, PageError};
use dom_query::Document;
use dom_smoothie::{Config, Readability};
use url::Url;

const MAX_ELEMENTS: usize = 100_000;

/// Readability 根据段落、链接密度与 DOM 结构选取正文，并处理标题和相对链接。
/// 短页面提取失败时回退到清理后的文档，避免丢失没有 article/main 的内容。
pub(super) fn extract_html(
    html: &str,
    url: &str,
    format: FetchFormat,
) -> Result<(String, String), PageError> {
    let document = Document::from(html);
    if document.select("*").length() > MAX_ELEMENTS {
        return Err(PageError::TooLarge);
    }
    let config = Config {
        char_threshold: 80,
        max_elements_to_parse: MAX_ELEMENTS,
        keep_classes: true,
        ..Default::default()
    };
    let mut reader = Readability::with_document(document, Some(url), Some(config))
        .map_err(|_| PageError::UnsupportedContent)?;
    let fallback_title = reader.get_article_title().to_string();
    let (title, article_html) = match reader.parse() {
        Ok(article) if !article.text_content.trim().is_empty() => {
            (article.title.to_string(), article.content.to_string())
        }
        _ => (fallback_title, html.to_owned()),
    };
    let document = Document::from(article_html);
    document
        .select("script, style, noscript, iframe, object, embed, nav, aside, footer, [hidden], [aria-hidden='true'], [role='navigation'], [role='complementary']")
        .remove();
    let base = Url::parse(url).map_err(|_| PageError::InvalidUrl)?;
    let cleaned = ammonia::Builder::default()
        .add_tag_attributes("code", &["class"])
        .url_relative(ammonia::UrlRelative::RewriteWithBase(base))
        .clean(&document.select("body").html())
        .to_string();
    let title: String = title.trim().chars().take(200).collect();
    let content = match format {
        FetchFormat::Markdown => {
            let markdown = htmd::convert(&cleaned).map_err(|_| PageError::UnsupportedContent)?;
            let has_heading = Document::from(cleaned.as_str())
                .select("h1, h2, h3, h4, h5, h6")
                .length()
                > 0;
            if !title.is_empty() && !has_heading && !markdown.trim().is_empty() {
                format!("# {title}\n\n{markdown}")
            } else {
                markdown
            }
        }
        FetchFormat::Text | FetchFormat::Json => {
            let text = Document::from(cleaned.as_str())
                .formatted_text()
                .to_string();
            if !title.is_empty() && !text.trim().is_empty() && !text.starts_with(&title) {
                format!("{title}\n\n{text}")
            } else {
                text
            }
        }
        FetchFormat::Html => cleaned,
    };
    Ok((title, content))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 任意 div 布局依靠内容评分识别，保留多个正文段落，排除导航和侧栏。
    #[test]
    fn extracts_article_without_site_selectors() {
        let paragraph = "RAG 将检索到的资料作为上下文，帮助模型基于证据回答问题。索引阶段先清理和切分资料，再计算向量并保存来源；查询阶段组合关键词和向量召回，然后重排结果。";
        let html = format!(
            "<title>检索增强生成</title><body><nav><a href='/'>导航噪声</a></nav><div><div id='unusual-layout'><h1>检索增强生成</h1><p>{paragraph}</p><p>{paragraph}</p><p>末段知识点：保留文档来源，方便回溯与核对。</p></div><aside><p>侧栏广告</p></aside></div></body>"
        );
        let (_, content) =
            extract_html(&html, "https://example.com/guide", FetchFormat::Markdown).unwrap();
        assert!(content.contains(paragraph));
        assert!(content.contains("末段知识点"));
        assert!(!content.contains("导航噪声"));
        assert!(!content.contains("侧栏广告"));
    }

    /// 转换必须保留列表、表格与代码缩进，且相对链接能直接作为资料来源访问。
    #[test]
    fn preserves_markdown_structure_and_resolves_links() {
        let html = "<title>技术文档</title><div><h1>技术文档</h1><p>检索流程应保留代码和表格，方便理解与复现。可以参考<a href='../reference'>详细说明</a>。</p><ul><li>建立索引</li><li>检索资料</li></ul><pre><code class='language-rust'>fn main() {\n    println!(\"hello\");\n}</code></pre><table><tr><th>阶段</th><th>产物</th></tr><tr><td>切分</td><td>文档块</td></tr></table></div>";
        let (_, content) = extract_html(
            html,
            "https://example.com/docs/intro",
            FetchFormat::Markdown,
        )
        .unwrap();
        assert!(content.contains("https://example.com/reference"));
        assert!(content.contains("```rust"));
        assert!(content.contains("    println!"));
        assert!(content.contains("建立索引"));
        assert!(content.contains("|"));
        assert!(content.contains("文档块"));
    }

    /// 短页面、裸文本节点及嵌套块不会因缺少段落标签而丢失或重复。
    #[test]
    fn handles_short_pages_and_nested_plain_text() {
        let html = "<body><div>短页面内容<span>与行内文字</span><div>第二行</div></div></body>";
        let (_, content) = extract_html(html, "https://example.com/", FetchFormat::Text).unwrap();
        assert!(content.contains("短页面内容"));
        assert_eq!(content.matches("第二行").count(), 1);
        assert!(!content.contains('<'));
    }

    /// HTML 输出移除可执行内容与隐藏噪声，保留可读正文。
    #[test]
    fn cleans_html_and_does_not_invent_body_from_title() {
        let html = "<body><p onclick='alert(1)'>可读正文</p><script>alert(2)</script><div hidden>隐藏噪声</div></body>";
        let (_, content) = extract_html(html, "https://example.com/", FetchFormat::Html).unwrap();
        assert!(content.contains("可读正文"));
        assert!(!content.contains("alert"));
        assert!(!content.contains("隐藏噪声"));
        let (_, content) = extract_html(
            "<title>空壳页面</title><script>load()</script>",
            "https://example.com/",
            FetchFormat::Markdown,
        )
        .unwrap();
        assert!(content.trim().is_empty());
    }
}
