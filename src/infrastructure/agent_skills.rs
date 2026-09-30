//! Skill 文件仓库：原包内容保存在目录，启停状态保存在独立 JSON 文件，不依赖数据库。

use crate::domain::agent_skill::{self, SkillSummary};
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashSet},
    fs,
    io::{Cursor, Read, Write},
    path::{Path, PathBuf},
    sync::Arc,
};
use tokio::sync::Mutex;
use utoipa::ToSchema;
use uuid::Uuid;

/// 压缩包与展开预算分别检查，避免压缩炸弹和无限文件树。
pub const MAX_ARCHIVE_BYTES: usize = 32 * 1024 * 1024;
pub const MAX_PACKAGE_BYTES: usize = 128 * 1024 * 1024;
pub const MAX_FILE_BYTES: usize = 20 * 1024 * 1024;
pub const MAX_FILES: usize = 2048;

/// 安装载荷中的一个普通文件；不允许符号链接或绝对路径。
pub struct PackageFile {
    pub path: String,
    pub bytes: Vec<u8>,
    pub mode: Option<u32>,
}

/// 技能索引只读取 SKILL.md 的元数据，损坏的目录单独告警。
#[derive(Clone, Serialize, ToSchema)]
pub struct SkillCatalog {
    pub skills: Vec<SkillSummary>,
    pub warnings: Vec<String>,
}

/// 可浏览的包文件，不暴露服务器绝对路径。
#[derive(Clone, Serialize, ToSchema)]
pub struct SkillFile {
    pub path: String,
    pub size: u64,
}

/// 详情保留原始 SKILL.md，同时展示完整资源目录。
#[derive(Serialize, ToSchema)]
pub struct SkillDetail {
    pub skill: SkillSummary,
    pub document: String,
    pub files: Vec<SkillFile>,
    pub source: Option<String>,
}

/// 管理状态在技能包之外保存，启停不会修改第三方文件。
#[derive(Clone, Default, Deserialize, Serialize)]
struct InstallState {
    enabled: bool,
    source: Option<String>,
}

/// 独立文件仓库；变更串行化，阻塞文件操作进入专用线程池。
#[derive(Clone)]
pub struct SkillStore {
    root: PathBuf,
    mutation: Arc<Mutex<()>>,
}

impl SkillStore {
    /// 目录仅在实际写入时创建，便于只读部署与无 Skill 的现有项目。
    pub fn new(root: PathBuf) -> Self {
        Self {
            root,
            mutation: Arc::new(Mutex::new(())),
        }
    }

    /// 将文件访问移到阻塞线程，避免占用 Axum 的异步执行线程。
    async fn blocking<T: Send + 'static>(
        &self,
        operation: impl FnOnce(PathBuf) -> Result<T> + Send + 'static,
    ) -> Result<T> {
        let root = self.root.clone();
        tokio::task::spawn_blocking(move || operation(root)).await?
    }

    /// 读取可发现技能与目录告警；不读取 references、scripts、assets 正文。
    pub async fn list(&self) -> Result<SkillCatalog> {
        self.blocking(|root| catalog(&root)).await
    }

    /// 打开单个包时才枚举全部资源。
    pub async fn detail(&self, name: &str) -> Result<SkillDetail> {
        let name = name.to_owned();
        self.blocking(move |root| detail(&root, &name)).await
    }

    /// 验证目录边界并读取单个文件，二进制资源原样返回。
    pub async fn read_file(&self, name: &str, path: &str) -> Result<Vec<u8>> {
        let name = name.to_owned();
        let path = path.to_owned();
        self.blocking(move |root| read_file(&root, &name, &path))
            .await
    }

    /// 管理开关不修改原包内容；新安装默认禁用。
    pub async fn set_enabled(&self, name: &str, enabled: bool) -> Result<()> {
        let _guard = self.mutation.lock().await;
        let name = name.to_owned();
        self.blocking(move |root| {
            detail(&root, &name)?;
            let mut states = read_states(&root)?;
            states.entry(name).or_default().enabled = enabled;
            write_states(&root, &states)
        })
        .await
    }

    /// 上传 ZIP 后先检查展开预算和路径，再原子安装选中的技能子目录。
    pub async fn install_archive(
        &self,
        bytes: Vec<u8>,
        skill_path: String,
        overwrite: bool,
        source: String,
    ) -> Result<SkillDetail> {
        let _guard = self.mutation.lock().await;
        self.blocking(move |root| install(&root, unpack(bytes)?, &skill_path, overwrite, source))
            .await
    }

    /// 文件夹上传保留相对路径与所有普通文件，与 ZIP 使用相同安装边界。
    pub async fn install_files(
        &self,
        files: Vec<PackageFile>,
        skill_path: String,
        overwrite: bool,
    ) -> Result<SkillDetail> {
        let _guard = self.mutation.lock().await;
        self.blocking(move |root| {
            install(&root, files, &skill_path, overwrite, "上传文件夹".into())
        })
        .await
    }

    /// 只更新 SKILL.md 原文，不改名、不删除附属文件。
    pub async fn save_document(&self, name: &str, source: String) -> Result<SkillDetail> {
        let _guard = self.mutation.lock().await;
        let name = name.to_owned();
        self.blocking(move |root| {
            let document = agent_skill::parse(&source)?;
            if document.manifest.name != name {
                bail!("编辑时不能改变 name；请作为新包安装");
            }
            let path = safe_file(&root, &name, "SKILL.md")?;
            atomic_write(&path, source.as_bytes())?;
            detail(&root, &name)
        })
        .await
    }

    /// 导出整个目录，包含 scripts、references、assets 和任意其他普通文件。
    pub async fn export(&self, name: &str) -> Result<Vec<u8>> {
        let name = name.to_owned();
        self.blocking(move |root| {
            let item = detail(&root, &name)?;
            let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
            for file in item.files {
                writer.start_file(
                    format!("{name}/{}", file.path),
                    zip::write::SimpleFileOptions::default()
                        .compression_method(zip::CompressionMethod::Deflated),
                )?;
                writer.write_all(&read_file(&root, &name, &file.path)?)?;
            }
            Ok(writer.finish()?.into_inner())
        })
        .await
    }

    /// 卸载只删除指定技能包，先隔离到仓库内临时目录，再更新管理状态。
    pub async fn uninstall(&self, name: &str) -> Result<()> {
        let _guard = self.mutation.lock().await;
        let name = name.to_owned();
        self.blocking(move |root| {
            let target = skill_dir(&root, &name)?;
            let trash = root.join(format!(".trash-{}", Uuid::new_v4()));
            fs::rename(&target, &trash)?;
            let result = (|| {
                let mut states = read_states(&root)?;
                states.remove(&name);
                write_states(&root, &states)
            })();
            if let Err(error) = result {
                fs::rename(&trash, &target)?;
                return Err(error);
            }
            fs::remove_dir_all(trash)?;
            Ok(())
        })
        .await
    }
}

/// 允许嵌套相对路径，拒绝跨平台路径穿越、设备名和 NTFS 数据流。
pub fn validate_path(path: &str) -> Result<()> {
    if path.is_empty() || path.len() > 1024 || path.contains(['\\', ':', '\0']) {
        bail!("包内文件路径无效");
    }
    for part in path.split('/') {
        let stem = part
            .split('.')
            .next()
            .unwrap_or_default()
            .to_ascii_lowercase();
        if part.is_empty()
            || matches!(part, "." | "..")
            || part.ends_with(['.', ' '])
            || part
                .chars()
                .any(|character| character.is_control() || "<>\"|?*".contains(character))
            || matches!(stem.as_str(), "con" | "prn" | "aux" | "nul")
            || (stem.len() == 4
                && (stem.starts_with("com") || stem.starts_with("lpt"))
                && matches!(stem.as_bytes()[3], b'1'..=b'9'))
        {
            bail!("包内文件路径不安全");
        }
    }
    Ok(())
}

/// 读取有界 ZIP，保留所有普通文件，拒绝链接、重复路径和虚假的展开尺寸。
fn unpack(bytes: Vec<u8>) -> Result<Vec<PackageFile>> {
    if bytes.len() > MAX_ARCHIVE_BYTES {
        bail!("ZIP 不能超过 32 MiB");
    }
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).context("不是有效的 ZIP 文件")?;
    if archive.len() > MAX_FILES {
        bail!("技能包最多 2048 个条目");
    }
    let mut files = Vec::new();
    let mut total = 0usize;
    for index in 0..archive.len() {
        let mut file = archive.by_index(index)?;
        if file
            .unix_mode()
            .is_some_and(|mode| mode & 0o170000 == 0o120000)
        {
            bail!("技能包不能包含符号链接");
        }
        validate_path(file.name().trim_end_matches('/'))?;
        if file.is_dir() {
            continue;
        }
        if file.size() > MAX_FILE_BYTES as u64 {
            bail!("单个文件不能超过 20 MiB");
        }
        let path = file.name().to_owned();
        let mode = file.unix_mode();
        let mut content = Vec::new();
        (&mut file)
            .take((MAX_FILE_BYTES + 1) as u64)
            .read_to_end(&mut content)?;
        total += content.len();
        if content.len() > MAX_FILE_BYTES || total > MAX_PACKAGE_BYTES {
            bail!("技能包展开不能超过 128 MiB");
        }
        files.push(PackageFile {
            path,
            bytes: content,
            mode,
        });
    }
    Ok(files)
}

/// 从单技能 ZIP 或仓库 ZIP 中选择根目录；多技能仓库须显式指定子目录。
fn install(
    root: &Path,
    files: Vec<PackageFile>,
    requested: &str,
    overwrite: bool,
    source: String,
) -> Result<SkillDetail> {
    if files.is_empty() || files.len() > MAX_FILES {
        bail!("技能包为空或超过 2048 个文件");
    }
    let mut seen = HashSet::new();
    let mut total = 0usize;
    for file in &files {
        validate_path(&file.path)?;
        if !seen.insert(file.path.to_lowercase()) {
            bail!("技能包包含重复或大小写冲突的路径");
        }
        total += file.bytes.len();
        if file.bytes.len() > MAX_FILE_BYTES || total > MAX_PACKAGE_BYTES {
            bail!("技能包超过文件或展开预算");
        }
    }
    let candidates = files
        .iter()
        .filter(|file| file.path == "SKILL.md" || file.path.ends_with("/SKILL.md"))
        .collect::<Vec<_>>();
    let selected = if !requested.is_empty() {
        validate_path(requested)?;
        let key = format!("{requested}/SKILL.md");
        let matches = candidates
            .iter()
            .filter(|file| {
                file.path == key
                    || file
                        .path
                        .split_once('/')
                        .is_some_and(|(_, tail)| tail == key)
            })
            .collect::<Vec<_>>();
        if matches.len() != 1 {
            bail!("指定的子目录必须恰好包含一个 SKILL.md");
        }
        *matches[0]
    } else {
        let top = candidates
            .iter()
            .filter(|file| file.path.split('/').count() <= 2)
            .collect::<Vec<_>>();
        if top.len() == 1 {
            *top[0]
        } else if candidates.len() == 1 {
            candidates[0]
        } else {
            bail!("未找到唯一 SKILL.md；多技能仓库请填写技能子目录，例如 skills/pdf");
        }
    };
    let text = std::str::from_utf8(&selected.bytes).context("SKILL.md 必须使用 UTF-8")?;
    let document = agent_skill::parse(text)?;
    let name = document.manifest.name;
    let prefix = selected
        .path
        .strip_suffix("SKILL.md")
        .unwrap_or_default()
        .to_owned();
    fs::create_dir_all(root)?;
    let root = root.canonicalize()?;
    let target = root.join(&name);
    if target.exists() && !overwrite {
        bail!("同名 Skill 已安装；确认替换后再重试");
    }
    if target.exists() {
        skill_dir(&root, &name)?;
    }
    let current = catalog(&root)?;
    if !target.exists() && current.skills.len() >= 200 {
        bail!("最多安装 200 个 Skill");
    }
    let stage = root.join(format!(".stage-{}", Uuid::new_v4()));
    let backup = root.join(format!(".backup-{}", Uuid::new_v4()));
    fs::create_dir(&stage)?;
    let prepared = (|| -> Result<()> {
        for file in files {
            let Some(relative) = file.path.strip_prefix(&prefix) else {
                continue;
            };
            validate_path(relative)?;
            let output = stage.join(relative);
            if let Some(parent) = output.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(&output, file.bytes)?;
            #[cfg(unix)]
            if let Some(mode) = file.mode {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(&output, fs::Permissions::from_mode(mode & 0o777))?;
            }
        }
        Ok(())
    })();
    if let Err(error) = prepared {
        fs::remove_dir_all(&stage)?;
        return Err(error);
    }
    let replacing = target.exists();
    if replacing {
        fs::rename(&target, &backup)?;
    }
    if let Err(error) = fs::rename(&stage, &target) {
        if replacing {
            fs::rename(&backup, &target)?;
        }
        fs::remove_dir_all(&stage)?;
        return Err(error.into());
    }
    let committed = (|| -> Result<()> {
        let mut states = read_states(&root)?;
        states.insert(
            name.clone(),
            InstallState {
                enabled: false,
                source: Some(source),
            },
        );
        write_states(&root, &states)
    })();
    if let Err(error) = committed {
        fs::remove_dir_all(&target)?;
        if replacing {
            fs::rename(&backup, &target)?;
        }
        return Err(error);
    }
    if replacing {
        fs::remove_dir_all(backup)?;
    }
    detail(&root, &name)
}

/// 根目录下的直接子目录才是技能包，忽略内部临时目录。
fn catalog(root: &Path) -> Result<SkillCatalog> {
    let mut result = SkillCatalog {
        skills: Vec::new(),
        warnings: Vec::new(),
    };
    if !root.exists() {
        return Ok(result);
    }
    let states = read_states(root)?;
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') || !entry.file_type()?.is_dir() {
            continue;
        }
        if result.skills.len() >= 200 {
            result
                .warnings
                .push("超过 200 个技能目录，剩余目录未加载".into());
            break;
        }
        match summary(root, &name, &states) {
            Ok(skill) => result.skills.push(skill),
            Err(_) => result.warnings.push(format!(
                "{name}: SKILL.md 无效、目录名称不匹配或包含不安全链接"
            )),
        }
    }
    result
        .skills
        .sort_by(|left, right| left.name.cmp(&right.name));
    Ok(result)
}

/// 读取元数据并合并包外管理状态，目录名必须匹配标准 name。
fn summary(
    root: &Path,
    name: &str,
    states: &BTreeMap<String, InstallState>,
) -> Result<SkillSummary> {
    let source = read_file(root, name, "SKILL.md")?;
    let document = agent_skill::parse(std::str::from_utf8(&source)?)?;
    if document.manifest.name != name {
        bail!("目录名称与 name 不匹配");
    }
    let manifest = document.manifest;
    let warnings = manifest.warnings();
    Ok(SkillSummary {
        name: manifest.name,
        description: manifest.description,
        enabled: states.get(name).is_some_and(|state| state.enabled),
        manual_only: manifest.manual_only,
        license: manifest.license,
        compatibility: manifest.compatibility,
        warnings,
    })
}

/// 单条详情枚举有界文件树，但不加载资源正文。
fn detail(root: &Path, name: &str) -> Result<SkillDetail> {
    let states = read_states(root)?;
    let skill = summary(root, name, &states)?;
    let directory = skill_dir(root, name)?;
    let mut files = Vec::new();
    let mut pending = vec![directory.clone()];
    let mut entries = 0usize;
    let mut total = 0u64;
    while let Some(parent) = pending.pop() {
        for entry in fs::read_dir(parent)? {
            entries += 1;
            if entries > MAX_FILES * 2 {
                bail!("技能包目录条目过多");
            }
            let entry = entry?;
            let kind = entry.file_type()?;
            if kind.is_symlink() {
                bail!("技能包不允许符号链接");
            }
            if kind.is_dir() {
                pending.push(entry.path());
            } else if kind.is_file() {
                let path = entry
                    .path()
                    .strip_prefix(&directory)?
                    .to_string_lossy()
                    .replace('\\', "/");
                validate_path(&path)?;
                let size = entry.metadata()?.len();
                total += size;
                if size > MAX_FILE_BYTES as u64
                    || total > MAX_PACKAGE_BYTES as u64
                    || files.len() >= MAX_FILES
                {
                    bail!("技能包超出预算");
                }
                files.push(SkillFile { path, size });
            } else {
                bail!("只允许普通文件和目录");
            }
            if pending.len() > MAX_FILES {
                bail!("技能包目录过多");
            }
        }
    }
    files.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(SkillDetail {
        skill,
        document: String::from_utf8(read_file(root, name, "SKILL.md")?)?,
        files,
        source: states.get(name).and_then(|state| state.source.clone()),
    })
}

/// 返回真实包目录，禁止链接及逃离配置根目录。
fn skill_dir(root: &Path, name: &str) -> Result<PathBuf> {
    if !agent_skill::valid_name(name) {
        bail!("Skill 名称无效");
    }
    let base = root.canonicalize()?;
    let path = base.join(name);
    let metadata = fs::symlink_metadata(&path)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        bail!("Skill 目录不安全");
    }
    let resolved = path.canonicalize()?;
    if !resolved.starts_with(&base) {
        bail!("Skill 目录越界");
    }
    Ok(resolved)
}

/// 逐层拒绝链接后再检查真实文件范围，不信任客户端路径或包内引用。
fn safe_file(root: &Path, name: &str, relative: &str) -> Result<PathBuf> {
    validate_path(relative)?;
    let directory = skill_dir(root, name)?;
    let mut path = directory.clone();
    for part in relative.split('/') {
        path.push(part);
        if fs::symlink_metadata(&path)?.file_type().is_symlink() {
            bail!("技能包不允许符号链接");
        }
    }
    let resolved = path.canonicalize()?;
    if !resolved.starts_with(&directory) || !resolved.is_file() {
        bail!("Skill 文件不存在或越界");
    }
    Ok(resolved)
}

/// 有界读取用于管理预览与工具读取，SKILL.md 使用更小的独立预算。
fn read_file(root: &Path, name: &str, relative: &str) -> Result<Vec<u8>> {
    let path = safe_file(root, name, relative)?;
    let limit = if relative == "SKILL.md" {
        agent_skill::MAX_SKILL_BYTES
    } else {
        MAX_FILE_BYTES
    };
    let mut content = Vec::new();
    fs::File::open(path)?
        .take((limit + 1) as u64)
        .read_to_end(&mut content)?;
    if content.len() > limit {
        bail!("Skill 文件超过读取预算");
    }
    Ok(content)
}

/// 包外状态文件有独立预算，不能因目录损坏而默默重新启用技能。
fn read_states(root: &Path) -> Result<BTreeMap<String, InstallState>> {
    let path = root.join(".state.json");
    if !path.exists() {
        return Ok(BTreeMap::new());
    }
    if fs::symlink_metadata(&path)?.file_type().is_symlink() {
        bail!("Skill 状态文件不能是链接");
    }
    let mut bytes = Vec::new();
    fs::File::open(path)?
        .take(262_145)
        .read_to_end(&mut bytes)?;
    if bytes.len() > 262_144 {
        bail!("Skill 状态文件过大");
    }
    Ok(serde_json::from_slice(&bytes)?)
}

/// 原子替换状态文件，不把状态写入第三方包。
fn write_states(root: &Path, states: &BTreeMap<String, InstallState>) -> Result<()> {
    let bytes = serde_json::to_vec_pretty(states)?;
    if bytes.len() > 262_144 {
        bail!("Skill 管理状态超过大小预算，请减少包数量或缩短来源信息");
    }
    atomic_write(&root.join(".state.json"), &bytes)
}

/// 同目录临时文件写入后重命名，失败时清理临时文件。
fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    let temporary = path.with_file_name(format!(".write-{}", Uuid::new_v4()));
    fs::write(&temporary, bytes)?;
    if let Err(error) = fs::rename(&temporary, path) {
        fs::remove_file(temporary)?;
        return Err(error.into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 每个测试使用独立临时仓库，退出后只清理自身创建的目录。
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            Self(std::env::temp_dir().join(format!("oxide-skill-{}", Uuid::new_v4())))
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            if self.0.exists() {
                fs::remove_dir_all(&self.0).unwrap();
            }
        }
    }
    fn files() -> Vec<PackageFile> {
        [("repo/skills/test-skill/SKILL.md", b"---\nname: test-skill\ndescription: Test file packages\n---\nRead references/info.md".as_slice()),
         ("repo/skills/test-skill/references/info.md", b"reference content".as_slice()),
         ("repo/skills/test-skill/scripts/check.py", b"print('test')".as_slice()),
         ("repo/skills/test-skill/assets/blob.bin", &[0, 255, 12])]
        .into_iter().map(|(path, bytes)| PackageFile { path: path.into(), bytes: bytes.into(), mode: None }).collect()
    }

    #[test]
    fn rejects_cross_platform_unsafe_paths() {
        for path in [
            "../secret",
            "/etc/passwd",
            "C:/secret",
            "a\\b",
            "a/../b",
            "con.txt",
            "a:stream",
            "a/",
            "a./b",
        ] {
            assert!(validate_path(path).is_err(), "{path}");
        }
        assert!(validate_path("references/说明.md").is_ok());
    }

    #[tokio::test]
    async fn installs_full_package_exports_and_removes_without_database() {
        let fixture = Fixture::new();
        let store = SkillStore::new(fixture.0.clone());
        let installed = store
            .install_files(files(), "skills/test-skill".into(), false)
            .await
            .unwrap();
        assert_eq!(installed.files.len(), 4);
        assert!(!installed.skill.enabled);
        assert_eq!(
            store
                .read_file("test-skill", "assets/blob.bin")
                .await
                .unwrap(),
            vec![0, 255, 12]
        );
        assert!(
            store
                .read_file("test-skill", "../.state.json")
                .await
                .is_err()
        );
        assert!(
            store
                .install_files(files(), String::new(), false)
                .await
                .is_err()
        );
        store.set_enabled("test-skill", true).await.unwrap();
        assert!(store.list().await.unwrap().skills[0].enabled);
        let exported = store.export("test-skill").await.unwrap();
        assert_eq!(unpack(exported).unwrap().len(), 4);
        let original = store.detail("test-skill").await.unwrap().document;
        store
            .save_document(
                "test-skill",
                original.replace("Read references/info.md", "Updated instructions"),
            )
            .await
            .unwrap();
        assert_eq!(store.detail("test-skill").await.unwrap().files.len(), 4);
        store
            .install_files(files(), String::new(), true)
            .await
            .unwrap();
        assert!(!store.list().await.unwrap().skills[0].enabled);
        store.uninstall("test-skill").await.unwrap();
        assert!(store.list().await.unwrap().skills.is_empty());
    }

    #[test]
    fn rejects_duplicate_paths_and_ambiguous_packages() {
        let fixture = Fixture::new();
        let mut package = files();
        package.push(PackageFile {
            path: package[0].path.to_uppercase(),
            bytes: Vec::new(),
            mode: None,
        });
        assert!(install(&fixture.0, package, "", false, "test".into()).is_err());
        let mut package = files();
        package.push(PackageFile {
            path: "repo/other/SKILL.md".into(),
            bytes: b"---\nname: other\ndescription: test\n---\nbody".into(),
            mode: None,
        });
        assert!(install(&fixture.0, package, "", false, "test".into()).is_err());
    }
}
