//! 素材的稳定身份与存储目录规则，不依赖 HTTP 或对象存储提供商。

use chrono::{DateTime, FixedOffset, Utc};
use uuid::Uuid;

/// 图片按上传时的上海年月归档；扩展名由调用方的文件头白名单校验确定。
/// UUID 保证同名图片不互相覆盖，分类或文章标题变化不触发对象重命名。
/// 对象键生成和元数据入库必须使用同一个 `created_at`，避免跨月上传落入不同月份。
pub fn image_object_key(public_id: Uuid, created_at: DateTime<Utc>, extension: &str) -> String {
    let timezone = FixedOffset::east_opt(8 * 3600).expect("上海时区偏移有效");
    format!(
        "assets/images/{}/{public_id}.{extension}",
        created_at.with_timezone(&timezone).format("%Y/%m")
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    /// 上海跨年的八小时窗口不能沿用 UTC 旧年份，与前端归档时区保持一致。
    #[test]
    fn image_key_uses_shanghai_calendar_boundary() {
        let id = Uuid::nil();
        let before = Utc.with_ymd_and_hms(2026, 12, 31, 15, 59, 59).unwrap();
        let after = Utc.with_ymd_and_hms(2026, 12, 31, 16, 0, 0).unwrap();
        assert_eq!(
            image_object_key(id, before, "png"),
            format!("assets/images/2026/12/{id}.png")
        );
        assert_eq!(
            image_object_key(id, after, "jpg"),
            format!("assets/images/2027/01/{id}.jpg")
        );
    }
}
