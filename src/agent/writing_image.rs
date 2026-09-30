//! 私密图片附件校验与 Rig 多模态消息构造，不保存公开素材或访问任意图片 URL。

use anyhow::{Result, bail};
use base64::{Engine, engine::general_purpose::STANDARD};
use rig::completion::Message;
use rig::message::{ImageDetail, ImageMediaType, UserContent};
use serde::Deserialize;
use utoipa::ToSchema;

/// 每条消息最多三张图片，每张最多 2 MiB，当前请求含历史图片合计最多 6 MiB。
pub const MAX_IMAGE_BYTES: usize = 2 * 1024 * 1024;
pub const MAX_TOTAL_BYTES: usize = 6 * 1024 * 1024;

/// 客户端只提供内联图片，不提供远程 URL、服务端路径或第三方 file_id。
#[derive(Clone, Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct WritingImage {
    pub name: String,
    pub mime_type: String,
    pub data: String,
}

impl WritingImage {
    /// 真实魔数、尺寸和长度均由服务端确认，避免把伪造文件交给模型。
    fn validated_size(&self) -> Result<usize> {
        if self.name.chars().count() > 160 || self.data.len() > MAX_IMAGE_BYTES.div_ceil(3) * 4 {
            bail!("图片名称或文件大小超过上限");
        }
        let bytes = STANDARD
            .decode(&self.data)
            .map_err(|_| anyhow::anyhow!("图片编码无效"))?;
        if bytes.is_empty() || bytes.len() > MAX_IMAGE_BYTES {
            bail!("单张图片不能超过 2 MiB");
        }
        let mime = infer::get(&bytes)
            .map(|kind| kind.mime_type())
            .unwrap_or_default();
        if !matches!(
            mime,
            "image/jpeg" | "image/png" | "image/gif" | "image/webp"
        ) || mime != self.mime_type
        {
            bail!("仅支持真实的 JPEG、PNG、GIF、WebP 图片");
        }
        let size = imagesize::blob_size(&bytes).map_err(|_| anyhow::anyhow!("无法读取图片尺寸"))?;
        if size.width == 0 || size.height == 0 || size.width > 8192 || size.height > 8192 {
            bail!("图片单边尺寸不能超过 8192 像素");
        }
        Ok(bytes.len())
    }
}

/// 一次请求统一统计当前图片与历史图片，禁止绕过总预算。
pub fn validate_images<'a>(groups: impl IntoIterator<Item = &'a [WritingImage]>) -> Result<()> {
    let mut total = 0usize;
    for images in groups {
        if images.len() > 3 {
            bail!("每条消息最多附加 3 张图片");
        }
        for image in images {
            total += image.validated_size()?;
            if total > MAX_TOTAL_BYTES {
                bail!("当前与历史图片总大小不能超过 6 MiB，请开始新对话");
            }
        }
    }
    Ok(())
}

/// 使用 Rig 的标准 image_url/base64 消息，图片留在用户消息中并参与后续工具回合。
pub fn user_message(text: String, images: Vec<WritingImage>) -> Message {
    let mut content = vec![UserContent::text(text)];
    for image in images {
        let media = match image.mime_type.as_str() {
            "image/jpeg" => ImageMediaType::JPEG,
            "image/gif" => ImageMediaType::GIF,
            "image/webp" => ImageMediaType::WEBP,
            _ => ImageMediaType::PNG,
        };
        content.push(UserContent::image_base64(
            image.data,
            Some(media),
            Some(ImageDetail::Auto),
        ));
    }
    Message::User { content }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn validates_content_not_extension_and_bounds() {
        let png = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+aL1sAAAAASUVORK5CYII=";
        let image = WritingImage {
            name: "private.png".into(),
            mime_type: "image/png".into(),
            data: png.into(),
        };
        assert!(validate_images([std::slice::from_ref(&image)]).is_ok());
        let message = user_message("描述图片".into(), vec![image.clone()]);
        assert!(serde_json::to_string(&message).unwrap().contains(png));
        let mut bad = image.clone();
        bad.mime_type = "image/jpeg".into();
        assert!(validate_images([std::slice::from_ref(&bad)]).is_err());
        bad = image.clone();
        bad.data = STANDARD.encode(b"<svg>not a supported image</svg>");
        assert!(validate_images([std::slice::from_ref(&bad)]).is_err());
        assert!(validate_images([vec![image; 4].as_slice()]).is_err());
    }
}
