//! 文章图片生成的输入边界和编辑式视觉提示词。

use anyhow::{Result, bail};

/// 图片在编辑器中的最终用途，决定画幅和提示词。
#[derive(Clone, Copy, Debug, serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ImagePurpose {
    /// 文章标题前的横幅主图。
    Cover,
    /// 正文段落间的配图。
    Inline,
}

/// 控制发给外部模型的文本规模，并保留管理员对画面的具体要求。
pub fn prompt(
    title: &str,
    source: &str,
    instruction: &str,
    purpose: ImagePurpose,
) -> Result<String> {
    let title = title.trim();
    let instruction = instruction.trim();
    if title.is_empty() || title.chars().count() > 200 || instruction.chars().count() > 500 {
        bail!("文章标题或配图要求无效");
    }
    let excerpt: String = source.chars().take(3_000).collect();
    let framing = match purpose {
        ImagePurpose::Cover => "横向 16:9 的文章主图，主题一眼可辨，主体保留完整轮廓",
        ImagePurpose::Inline => "适合嵌入文章段落的横向配图，清楚呈现具体概念或场景",
    };
    Ok(format!(
        "为文章《{title}》生成{framing}。\n文章内容摘录：{excerpt}\n画面要求：{instruction}\n风格：编辑式摄影或精细插画，克制的双色点缀，自然光或清晰材质，适量留白。不要文字、水印、边框、相纸效果或网页界面。"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompt_limits_context_and_selects_purpose() {
        let cover = prompt("标题", &"甲".repeat(4_000), "真实物件", ImagePurpose::Cover).unwrap();
        assert!(cover.contains("16:9"));
        assert_eq!(cover.matches('甲').count(), 3_000);
        assert!(prompt("", "正文", "", ImagePurpose::Inline).is_err());
    }
}
