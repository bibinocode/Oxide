//! 数据库异地备份的对象命名、完整性记录和轮换规则；不依赖数据库或七牛 SDK。

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// 仅在这个命名空间内轮换备份，避免清理同空间中的网站素材。
pub const PREFIX: &str = "oxide-backups/postgres/";
/// 每 72 小时执行一次；以最后成功时间计算，容器重启不会重置周期。
pub const INTERVAL_SECONDS: i64 = 72 * 60 * 60;

/// 可公开的完整性清单；不包含数据库连接串、业务数据或解密私钥。
#[derive(Clone, Serialize, Deserialize)]
pub struct BackupManifest {
    /// 加密归档的七牛对象名。
    pub object_key: String,
    /// 开始生成快照的 UTC 时间。
    pub created_at: DateTime<Utc>,
    /// 加密归档的 SHA-256。
    pub sha256: String,
    /// 加密归档的字节数。
    pub bytes: u64,
    /// 恢复所需的格式说明。
    pub format: String,
}

/// 持久化调度状态；清理失败时只重试清理，不重复生成快照。
#[derive(Clone, Serialize, Deserialize)]
pub struct BackupState {
    /// 最近已经上传并下载校验通过的备份。
    pub backup: BackupManifest,
    /// 新备份有效，但旧备份清理尚未全部完成。
    pub cleanup_pending: bool,
}

/// 每次使用独立对象名；新上传不会覆盖上一份可恢复的备份。
pub fn object_key(now: DateTime<Utc>, id: Uuid) -> String {
    format!("{PREFIX}{}/{id}.tar.age", now.format("%Y/%m/%d"))
}

/// 只识别本任务生成的 UUID 备份与清单，不根据宽泛前缀删除其他文件。
pub fn owned_object(key: &str) -> bool {
    let Some(relative) = key.strip_prefix(PREFIX) else {
        return false;
    };
    let relative = relative.strip_suffix(".json").unwrap_or(relative);
    let Some(relative) = relative.strip_suffix(".tar.age") else {
        return false;
    };
    let parts: Vec<_> = relative.split('/').collect();
    parts.len() == 4
        && NaiveDate::parse_from_str(&parts[..3].join("/"), "%Y/%m/%d").is_ok()
        && Uuid::parse_str(parts[3]).is_ok()
}

/// 以成功快照时间而非进程启动时间计算下一次执行，避免跨月 cron 的周期漂移。
pub fn due(now: DateTime<Utc>, state: Option<&BackupState>) -> bool {
    state.is_none_or(|state| (now - state.backup.created_at).num_seconds() >= INTERVAL_SECONDS)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cleanup_scope_excludes_images_and_unrecognized_objects() {
        let key = object_key(Utc::now(), Uuid::new_v4());
        assert!(owned_object(&key));
        assert!(owned_object(&format!("{key}.json")));
        assert!(!owned_object("assets/images/2026/10/photo.png"));
        assert!(!owned_object("oxide-backups/postgres/notes.tar.age"));
        assert!(!owned_object(
            "oxide-backups/postgres/2026/99/05/00000000-0000-0000-0000-000000000000.tar.age"
        ));
    }
    #[test]
    fn schedule_survives_restart_and_month_boundary() {
        let at = DateTime::parse_from_rfc3339("2026-10-30T19:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let state = BackupState {
            backup: BackupManifest {
                object_key: object_key(at, Uuid::new_v4()),
                created_at: at,
                sha256: "0".repeat(64),
                bytes: 10,
                format: "pgdump-custom+tar+age-v1".into(),
            },
            cleanup_pending: false,
        };
        assert!(!due(at + chrono::Duration::hours(71), Some(&state)));
        assert!(due(at + chrono::Duration::hours(72), Some(&state)));
        assert!(due(at, None));
    }
}
