//! PostgreSQL 快照、age 流式加密和七牛异地备份；明文仅存在于权限受限的临时目录。

use crate::{
    domain::backup::{BackupManifest, BackupState, PREFIX, object_key, owned_object},
    entity::{status::StorageProviderKind, storage_provider},
    infrastructure::{secrets, storage::Credentials},
};
use anyhow::{Context, Result, bail};
use chrono::Utc;
use qiniu_sdk::{
    download::{DownloadManager, StaticDomainsUrlsGenerator, UrlsSigner},
    objects::{ObjectsManager, apis::credential::Credential},
    upload::{AutoUploader, AutoUploaderObjectParams, UploadManager, UploadTokenSigner},
};
use sea_orm::{ConnectOptions, Database, EntityTrait};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    env,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::Duration,
};
use uuid::Uuid;

/// 备份进程专用配置；恢复私钥绝不传给定时备份进程。
pub struct BackupConfig {
    /// 连接串仅通过环境传入子进程，不放入命令行或日志。
    pub database_url: String,
    /// 解密现有存储凭据和恢复数据库中的集成配置所需的服务端密钥。
    pub server_key: String,
    /// 固定使用指定七牛实例，站点切换图片存储不会意外迁移备份。
    pub provider_id: String,
    /// 固定空间名称，后台修改图片空间时阻止备份意外迁移。
    pub expected_bucket: String,
    /// age 公钥文件，独立于网站的存储凭据。
    pub recipient_file: PathBuf,
    /// 持久化调度与失败状态的目录。
    pub state_dir: PathBuf,
    /// 明文工作目录，每次创建独立子目录并在结束后删除。
    pub work_dir: PathBuf,
}

impl BackupConfig {
    /// 从专用容器环境读取，不记录任何配置值。
    pub fn load() -> Result<Self> {
        fn required(key: &str) -> Result<String> {
            env::var(key)
                .ok()
                .filter(|s| !s.is_empty())
                .with_context(|| format!("缺少备份配置 {key}"))
        }
        Ok(Self {
            database_url: required("DATABASE_URL")?,
            server_key: required("COMMENT_HASH_KEY")?,
            provider_id: required("BACKUP_STORAGE_PROVIDER_ID")?,
            expected_bucket: required("BACKUP_QINIU_BUCKET")?,
            recipient_file: env::var("BACKUP_RECIPIENT_FILE")
                .unwrap_or_else(|_| "/config/recipient.txt".into())
                .into(),
            state_dir: env::var("BACKUP_STATE_DIR")
                .unwrap_or_else(|_| "/state".into())
                .into(),
            work_dir: env::var("BACKUP_WORK_DIR")
                .unwrap_or_else(|_| "/work".into())
                .into(),
        })
    }
    /// 缺失代表首次运行；损坏状态不静默丢弃，防止错误推进清理。
    pub fn state(&self) -> Result<Option<BackupState>> {
        match fs::read(self.state_dir.join("last-success.json")) {
            Ok(bytes) => Ok(Some(
                serde_json::from_slice(&bytes).context("备份调度状态损坏")?,
            )),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.into()),
        }
    }
    /// 失败反馈不含凭据或签名 URL，1Panel 可查看健康状态与日志。
    pub fn record_failure(&self, message: &str) -> Result<()> {
        write_json(
            &self.state_dir.join("last-failure.json"),
            &serde_json::json!({"at": Utc::now(), "message": message}),
        )
    }
}

/// 同步对象存储边界；便于测试上传损坏与删除失败，不在测试中访问真实存储。
pub trait BackupStore {
    /// 按路径流式上传。
    fn upload(&mut self, key: &str, path: &Path) -> Result<()>;
    /// 按路径流式下载。
    fn download(&mut self, key: &str, path: &Path) -> Result<()>;
    /// 分页读取本任务命名空间的对象名。
    fn list(&mut self) -> Result<Vec<String>>;
    /// 删除明确识别的备份对象。
    fn delete(&mut self, key: &str) -> Result<()>;
}

/// 七牛 SDK 适配器；HTTP 错误只输出阶段，不把签名 URL 或密钥写入日志。
pub struct QiniuBackupStore {
    manager: ObjectsManager,
    uploader: UploadManager,
    downloader: DownloadManager,
    bucket: String,
}

impl QiniuBackupStore {
    /// 使用 SeaORM 读取现有加密配置，不通过管理 API 导出密钥。
    pub async fn connect(config: &BackupConfig) -> Result<Self> {
        let mut options = ConnectOptions::new(config.database_url.clone());
        options
            .max_connections(1)
            .min_connections(1)
            .sqlx_logging(false);
        let db = Database::connect(options)
            .await
            .map_err(|_| anyhow::anyhow!("无法读取备份存储配置：数据库连接失败"))?;
        let row = storage_provider::Entity::find_by_id(&config.provider_id)
            .one(&db)
            .await
            .map_err(|_| anyhow::anyhow!("读取备份存储实例失败"))?
            .context("备份存储实例不存在")?;
        if row.kind != StorageProviderKind::QiniuKodo {
            bail!("备份实例必须是七牛 Kodo");
        }
        let encrypted = row
            .encrypted_credentials
            .context("请先在后台保存七牛存储密钥")?;
        let value: Credentials =
            serde_json::from_slice(&secrets::open(&encrypted, &config.server_key, "storage")?)
                .context("存储凭据格式无效")?;
        let bucket = row.bucket.context("七牛空间名称缺失")?;
        if bucket != config.expected_bucket {
            bail!("七牛空间与固定备份空间不一致，请检查配置");
        }
        let domain = url::Url::parse(&row.public_base_url).context("七牛下载域名格式无效")?;
        if domain.scheme() != "https" {
            bail!("备份校验需要 HTTPS 下载域名");
        }
        let domain = domain.host_str().context("七牛下载域名缺失")?.to_owned();
        let credential = Credential::new(value.access_key, value.secret_key);
        let manager = ObjectsManager::builder(credential.clone())
            .use_https(true)
            .build();
        let uploader = UploadManager::builder(UploadTokenSigner::new_credential_provider(
            credential.clone(),
            bucket.clone(),
            Duration::from_secs(3600),
        ))
        .use_https(true)
        .build();
        let downloader = DownloadManager::builder(UrlsSigner::new(
            credential,
            StaticDomainsUrlsGenerator::new(domain),
        ))
        .use_https(true)
        .build();
        Ok(Self {
            manager,
            uploader,
            downloader,
            bucket,
        })
    }
}

impl BackupStore for QiniuBackupStore {
    fn upload(&mut self, key: &str, path: &Path) -> Result<()> {
        let uploader: AutoUploader = self.uploader.auto_uploader();
        uploader
            .upload_path(
                path,
                AutoUploaderObjectParams::builder().object_name(key).build(),
            )
            .map_err(|_| anyhow::anyhow!("七牛备份上传失败"))?;
        Ok(())
    }
    fn download(&mut self, key: &str, path: &Path) -> Result<()> {
        let mut output = private_file(path)?;
        self.downloader
            .download(key)
            .map_err(|_| anyhow::anyhow!("生成备份下载请求失败"))?
            .to_writer(&mut output)
            .map_err(|_| anyhow::anyhow!("七牛备份下载校验失败"))?;
        output.sync_all()?;
        Ok(())
    }
    fn list(&mut self) -> Result<Vec<String>> {
        let bucket = self.manager.bucket(self.bucket.clone());
        bucket
            .list()
            .prefix(PREFIX)
            .iter()
            .map(|item| {
                item.map(|item| item.get_key_as_str().to_owned())
                    .map_err(|_| anyhow::anyhow!("读取七牛旧备份列表失败"))
            })
            .collect()
    }
    fn delete(&mut self, key: &str) -> Result<()> {
        self.manager
            .bucket(self.bucket.clone())
            .delete_object(key)
            .call()
            .map_err(|_| anyhow::anyhow!("删除七牛旧备份失败"))?;
        Ok(())
    }
}

/// 有界流式 SHA-256，数据库较大时也不把整个备份装入内存。
pub fn file_digest(path: &Path) -> Result<String> {
    let mut input = File::open(path)?;
    let mut digest = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let size = input.read(&mut buffer)?;
        if size == 0 {
            break;
        }
        digest.update(&buffer[..size]);
    }
    Ok(hex::encode(digest.finalize()))
}

/// 原子持久化状态，进程中断不会留下半份 JSON；同目录 rename 后同步父目录。
fn write_json(path: &Path, value: &impl Serialize) -> Result<()> {
    let parent = path.parent().context("备份状态目录无效")?;
    fs::create_dir_all(parent)?;
    let temporary = parent.join(format!(".{}.tmp", Uuid::new_v4()));
    let mut output = private_file(&temporary)?;
    serde_json::to_writer(&mut output, value)?;
    output.write_all(b"\n")?;
    output.sync_all()?;
    fs::rename(temporary, path)?;
    File::open(parent)?.sync_all()?;
    Ok(())
}

/// 所有明文与下载临时文件均限制为当前用户读写。
fn private_file(path: &Path) -> Result<File> {
    Ok(OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)?)
}

/// 上传后下载完整对象检查 SHA-256，校验失败不会进入旧备份删除阶段。
fn verify(store: &mut impl BackupStore, manifest: &BackupManifest, work: &Path) -> Result<()> {
    let path = work.join("verify.age");
    store.download(&manifest.object_key, &path)?;
    if fs::metadata(&path)?.len() != manifest.bytes || file_digest(&path)? != manifest.sha256 {
        bail!("七牛备份完整性校验失败，已保留上一份备份");
    }
    fs::remove_file(path)?;
    Ok(())
}

/// 新备份及清单均确认可读取后先记录成功状态，再删除旧对象，重启后可继续清理。
pub fn publish(
    store: &mut impl BackupStore,
    manifest: BackupManifest,
    encrypted: &Path,
    state_dir: &Path,
    work: &Path,
) -> Result<()> {
    store.upload(&manifest.object_key, encrypted)?;
    verify(store, &manifest, work)?;
    let path = work.join("manifest.json");
    write_json(&path, &manifest)?;
    let manifest_key = format!("{}.json", manifest.object_key);
    store.upload(&manifest_key, &path)?;
    let check = work.join("manifest-check.json");
    store.download(&manifest_key, &check)?;
    if file_digest(&path)? != file_digest(&check)? {
        bail!("备份清单校验失败，已保留上一份备份");
    }
    let mut state = BackupState {
        backup: manifest,
        cleanup_pending: true,
    };
    write_json(&state_dir.join("last-success.json"), &state)?;
    rotate(store, &mut state, state_dir, work)
}

/// 清理前再次确认最新备份仍可完整读取；仅删除本任务的 UUID 对象。
pub fn rotate(
    store: &mut impl BackupStore,
    state: &mut BackupState,
    state_dir: &Path,
    work: &Path,
) -> Result<()> {
    if !owned_object(&state.backup.object_key) || !state.backup.object_key.ends_with(".tar.age") {
        bail!("备份状态中的对象名无效，停止清理");
    }
    verify(store, &state.backup, work)?;
    let current = &state.backup.object_key;
    let current_manifest = format!("{current}.json");
    for key in store.list()? {
        if owned_object(&key) && key != *current && key != current_manifest {
            store.delete(&key)?;
        }
    }
    state.cleanup_pending = false;
    write_json(&state_dir.join("last-success.json"), state)?;
    match fs::remove_file(state_dir.join("last-failure.json")) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(e.into()),
    }
    Ok(())
}

/// 工作目录的清理守卫：无论备份在哪个阶段失败，都会删除此次产生的明文。
struct Workspace(PathBuf);
impl Drop for Workspace {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// 执行快照、归档和加密工具；连接信息保留在环境内，错误输出不包含密钥。
fn tool(mut command: Command, phase: &str) -> Result<()> {
    let status = command
        .stderr(Stdio::null())
        .status()
        .with_context(|| format!("无法启动{phase}"))?;
    if !status.success() {
        bail!("{phase}失败，退出码 {:?}", status.code());
    }
    Ok(())
}

/// 将应用 URI 转为 libpq 环境变量；pg_dump 不会把 PGDATABASE 环境值作为 URI 展开。
/// 通过环境传递密码，避免在进程命令行出现凭据；保留百分号编码的真实字符。
fn snapshot_command(database_url: &str) -> Result<Command> {
    let url = url::Url::parse(database_url).map_err(|_| anyhow::anyhow!("数据库连接格式无效"))?;
    if !matches!(url.scheme(), "postgres" | "postgresql") {
        bail!("数据库快照仅支持 PostgreSQL");
    }
    fn decode(value: &str) -> String {
        let encoded = format!("value={}", value.replace('+', "%2B"));
        url::form_urlencoded::parse(encoded.as_bytes())
            .next()
            .unwrap()
            .1
            .into_owned()
    }
    let mut command = Command::new("pg_dump");
    command
        .env("PGHOST", url.host_str().context("数据库主机缺失")?)
        .env("PGPORT", url.port().unwrap_or(5432).to_string())
        .env("PGUSER", decode(url.username()))
        .env("PGPASSWORD", decode(url.password().unwrap_or_default()))
        .env("PGDATABASE", decode(url.path().trim_start_matches('/')));
    for (key, value) in url.query_pairs() {
        let variable = match key.as_ref() {
            "sslmode" => "PGSSLMODE",
            "sslrootcert" => "PGSSLROOTCERT",
            "sslcert" => "PGSSLCERT",
            "sslkey" => "PGSSLKEY",
            "connect_timeout" => "PGCONNECT_TIMEOUT",
            "application_name" => "PGAPPNAME",
            "options" => "PGOPTIONS",
            _ => bail!("数据库连接含快照工具不支持的参数"),
        };
        command.env(variable, value.as_ref());
    }
    Ok(command)
}

/// 单次备份或清理重试；跨进程锁防止手动执行与定时任务同时轮换。
pub fn run(config: &BackupConfig, store: &mut impl BackupStore) -> Result<()> {
    fs::create_dir_all(&config.state_dir)?;
    fs::set_permissions(&config.state_dir, fs::Permissions::from_mode(0o700))?;
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .open(config.state_dir.join("job.lock"))?;
    lock.try_lock()
        .map_err(|_| anyhow::anyhow!("已有备份任务正在运行"))?;
    fs::create_dir_all(&config.work_dir)?;
    // 持锁后清除强制终止遗留的专用工作目录，不触碰其他目录或符号链接。
    for entry in fs::read_dir(&config.work_dir)? {
        let entry = entry?;
        let name = entry.file_name();
        if entry.file_type()?.is_dir()
            && name
                .to_str()
                .and_then(|s| s.strip_prefix("oxide-backup-"))
                .is_some_and(|s| Uuid::parse_str(s).is_ok())
        {
            fs::remove_dir_all(entry.path())?;
        }
    }
    let directory = config
        .work_dir
        .join(format!("oxide-backup-{}", Uuid::new_v4()));
    fs::create_dir(&directory)?;
    fs::set_permissions(&directory, fs::Permissions::from_mode(0o700))?;
    let workspace = Workspace(directory);
    if let Some(mut state) = config.state()?
        && state.cleanup_pending
    {
        return rotate(store, &mut state, &config.state_dir, &workspace.0);
    }
    let created_at = Utc::now();
    let dump = workspace.0.join("database.dump");
    let mut command = snapshot_command(&config.database_url)?;
    command
        .args([
            "--format=custom",
            "--no-owner",
            "--no-acl",
            "--lock-wait-timeout=60s",
        ])
        .stdout(Stdio::from(private_file(&dump)?));
    tool(command, "数据库快照")?;
    let mut command = Command::new("pg_restore");
    command.arg("--list").arg(&dump).stdout(Stdio::null());
    tool(command, "快照格式校验")?;
    // 加密归档携带原服务端密钥，服务器丢失后仍能解密数据库内的集成凭据。
    write_json(
        &workspace.0.join("recovery.json"),
        &serde_json::json!({"comment_hash_key": config.server_key, "format": "pgdump-custom", "created_at": created_at}),
    )?;
    let plain = workspace.0.join("database.tar");
    let mut command = Command::new("tar");
    command
        .arg("-cf")
        .arg(&plain)
        .arg("-C")
        .arg(&workspace.0)
        .args(["database.dump", "recovery.json"]);
    tool(command, "备份归档")?;
    let encrypted = workspace.0.join("database.tar.age");
    let mut command = Command::new("age");
    command
        .arg("--encrypt")
        .arg("--recipients-file")
        .arg(&config.recipient_file)
        .arg("--output")
        .arg(&encrypted)
        .arg(&plain);
    tool(command, "备份加密")?;
    fs::remove_file(plain)?;
    fs::remove_file(dump)?;
    fs::remove_file(workspace.0.join("recovery.json"))?;
    let manifest = BackupManifest {
        object_key: object_key(created_at, Uuid::new_v4()),
        created_at,
        sha256: file_digest(&encrypted)?,
        bytes: fs::metadata(&encrypted)?.len(),
        format: "pgdump-custom+tar+age-v1".into(),
    };
    publish(store, manifest, &encrypted, &config.state_dir, &workspace.0)
}

/// 下载指定备份及清单，离线恢复之前验证完整性；不自动写入任何数据库。
pub fn download(store: &mut impl BackupStore, key: &str, output: &Path) -> Result<()> {
    if !owned_object(key) || !key.ends_with(".tar.age") {
        bail!("不是本任务创建的备份对象");
    }
    if output.exists() {
        bail!("恢复文件已存在，请指定新路径");
    }
    let manifest_path = output.with_extension("manifest.json");
    store.download(&format!("{key}.json"), &manifest_path)?;
    let manifest: BackupManifest = serde_json::from_slice(&fs::read(&manifest_path)?)?;
    if manifest.object_key != key {
        bail!("备份对象与清单不一致");
    }
    store.download(key, output)?;
    if fs::metadata(output)?.len() != manifest.bytes || file_digest(output)? != manifest.sha256 {
        fs::remove_file(output)?;
        bail!("恢复文件 SHA-256 校验失败");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    /// 内存存储注入下载损坏和清理故障，模拟七牛失败而不访问真实空间。
    #[derive(Default)]
    struct FakeStore {
        objects: BTreeMap<String, Vec<u8>>,
        corrupt: bool,
        fail_delete: bool,
    }
    impl BackupStore for FakeStore {
        fn upload(&mut self, key: &str, path: &Path) -> Result<()> {
            self.objects.insert(key.into(), fs::read(path)?);
            Ok(())
        }
        fn download(&mut self, key: &str, path: &Path) -> Result<()> {
            let value = self.objects.get(key).context("对象不存在")?;
            fs::write(
                path,
                if self.corrupt {
                    b"corrupt".as_slice()
                } else {
                    value.as_slice()
                },
            )?;
            Ok(())
        }
        fn list(&mut self) -> Result<Vec<String>> {
            Ok(self.objects.keys().cloned().collect())
        }
        fn delete(&mut self, key: &str) -> Result<()> {
            if self.fail_delete {
                bail!("模拟删除失败");
            }
            self.objects.remove(key);
            Ok(())
        }
    }
    /// 每个测试使用独立目录，释放时自动清理。
    fn fixture() -> (Workspace, BackupManifest, PathBuf, FakeStore, String) {
        let dir = env::temp_dir().join(format!("oxide-backup-test-{}", Uuid::new_v4()));
        fs::create_dir(&dir).unwrap();
        let path = dir.join("encrypted.age");
        fs::write(&path, b"encrypted archive fixture").unwrap();
        let manifest = BackupManifest {
            object_key: object_key(Utc::now(), Uuid::new_v4()),
            created_at: Utc::now(),
            sha256: file_digest(&path).unwrap(),
            bytes: fs::metadata(&path).unwrap().len(),
            format: "pgdump-custom+tar+age-v1".into(),
        };
        let old = object_key(Utc::now() - chrono::Duration::days(3), Uuid::new_v4());
        let mut store = FakeStore::default();
        store
            .objects
            .insert(old.clone(), b"old-good-backup".to_vec());
        store
            .objects
            .insert("assets/images/photo.png".into(), b"image".to_vec());
        (Workspace(dir), manifest, path, store, old)
    }
    #[test]
    fn snapshot_credentials_use_decoded_environment_only() {
        let command =
            snapshot_command("postgres://user:p%2B%40ss@postgres:5432/blog?sslmode=require")
                .unwrap();
        let variables: BTreeMap<_, _> = command
            .get_envs()
            .map(|(k, v)| {
                (
                    k.to_string_lossy().into_owned(),
                    v.unwrap().to_string_lossy().into_owned(),
                )
            })
            .collect();
        assert_eq!(variables["PGPASSWORD"], "p+@ss");
        assert_eq!(variables["PGDATABASE"], "blog");
        assert_eq!(variables["PGHOST"], "postgres");
        assert_eq!(variables["PGSSLMODE"], "require");
        assert_eq!(command.get_args().count(), 0);
    }
    #[test]
    fn corrupt_upload_never_removes_previous_backup() {
        let (work, manifest, path, mut store, old) = fixture();
        store.corrupt = true;
        assert!(publish(&mut store, manifest, &path, &work.0.join("state"), &work.0).is_err());
        assert!(store.objects.contains_key(&old));
        assert!(!work.0.join("state/last-success.json").exists());
    }
    #[test]
    fn verified_backup_replaces_old_backup_without_touching_images() {
        let (work, manifest, path, mut store, old) = fixture();
        let key = manifest.object_key.clone();
        publish(&mut store, manifest, &path, &work.0.join("state"), &work.0).unwrap();
        assert!(!store.objects.contains_key(&old));
        assert!(store.objects.contains_key(&key));
        assert!(store.objects.contains_key("assets/images/photo.png"));
        let state: BackupState =
            serde_json::from_slice(&fs::read(work.0.join("state/last-success.json")).unwrap())
                .unwrap();
        assert!(!state.cleanup_pending);
    }
    #[test]
    fn cleanup_failure_persists_new_backup_and_retries_without_new_upload() {
        let (work, manifest, path, mut store, old) = fixture();
        let key = manifest.object_key.clone();
        store.fail_delete = true;
        assert!(publish(&mut store, manifest, &path, &work.0.join("state"), &work.0).is_err());
        let mut state: BackupState =
            serde_json::from_slice(&fs::read(work.0.join("state/last-success.json")).unwrap())
                .unwrap();
        assert!(state.cleanup_pending);
        assert!(store.objects.contains_key(&old));
        assert!(store.objects.contains_key(&key));
        store.fail_delete = false;
        rotate(&mut store, &mut state, &work.0.join("state"), &work.0).unwrap();
        assert!(!state.cleanup_pending);
        assert!(!store.objects.contains_key(&old));
    }
}
