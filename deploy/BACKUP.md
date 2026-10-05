# 七牛数据库备份与恢复

生产环境使用独立 `backup` 容器（1Panel 可查看），使用 `backup.env` 指定的存储提供商与七牛空间。每次成功快照后间隔 **72 小时**，状态持久化，容器重启不会重置时间。首次运行立即备份；失败每小时重试。上传、完整下载 SHA-256 校验和清单校验成功后才清理旧备份；删除失败持久化为待清理状态，重试时不会重新上传快照。

对象目录为 `oxide-backups/postgres/YYYY/MM/DD/`，正常只保留最新 `.tar.age` 与 `.tar.age.json`。清理严格限于程序生成的日期 / UUID 对象，不会清理网站素材或原有手动快照。不要把其他系统的备份放进同一命名空间。

## 密钥与内容

归档使用 age 公钥加密，包含 PostgreSQL 17 自定义格式 `database.dump` 和 `recovery.json`。后者包含原服务端 `COMMENT_HASH_KEY`，用于恢复数据库内加密的存储 / AI 集成凭据。现有七牛空间访问属性不变，即使公开访问也只能获得加密归档。

恢复私钥在服务器 `/opt/oxide/backup-recovery/age-key.txt`，另保存到本地项目 `data/backup-recovery/age-key.txt`（权限 600，忽略版本控制）。**必须另外离线保存私钥**；服务器和本地私钥同时丢失将无法恢复。备份容器只挂载公钥 `recipient.txt`，不会接触私钥。不要将私钥、恢复配置或数据库明文上传公开空间。

专用 `backup.env` 仅包含数据库连接、原服务端密钥、存储实例 ID 和固定空间名；均为 root-only 文件。空间名改变时任务停止，避免后台图片存储配置调整导致备份迁移。

## 安装与升级

```sh
# 在目标架构构建，生产无需重新构建 API / 前端。
docker build -f deploy/Dockerfile.backup -t oxide-backup:20261005-backup .
```

复制 `deploy/backup.env.example` 为 `/opt/oxide/stack/backup.env` 并填入与 API 相同的连接和服务端密钥，权限设为 600。预先生成 age 私钥和公钥，并创建权限 700 的 `/opt/oxide/backups/cloud-state`、`cloud-work` 目录。Compose 的 `BACKUP_RELEASE` 可单独更新备份镜像版本；首次 `once` 成功后再启动 `docker compose up -d --no-deps backup`。

## 管理与手动备份

```sh
cd /opt/oxide/stack
docker compose logs --tail 50 backup
docker compose exec backup oxide-backup status
# 手动备份会立即生成新快照并安全轮换；避免与计划任务同时运行。
docker compose run --rm backup once
```

`status` 在从未成功、清理待重试、上次快照超过 78 小时时返回失败，Docker 健康检查可在 1Panel 查看。当前周期以状态文件的 UTC `created_at` 为基准；下次快照为其后 72 小时。

## 下载、解密和隔离恢复

先安装 age、PostgreSQL 17 客户端。以下示例在服务器上操作，**先恢复到新数据库检查，禁止直接覆盖生产数据库**。从 `status` 的 `object_key` 或七牛目录获取最新对象名，不使用旧缓存链接。

```sh
cd /opt/oxide/stack
install -d -m 700 /opt/oxide/restore
# 替换对象名，目标文件必须不存在；会自动下载清单并校验 SHA-256。
docker compose run --rm -v /opt/oxide/restore:/restore backup \
  download 'oxide-backups/postgres/YYYY/MM/DD/UUID.tar.age' /restore/backup.tar.age
age --decrypt -i /opt/oxide/backup-recovery/age-key.txt \
  -o /opt/oxide/restore/backup.tar /opt/oxide/restore/backup.tar.age
chmod 600 /opt/oxide/restore/backup.tar
tar -xf /opt/oxide/restore/backup.tar -C /opt/oxide/restore
# PGDATABASE 从权限受限环境文件注入，指向新数据库；不要写进公开日志。
pg_restore --single-transaction --exit-on-error --no-owner --no-acl \
  --dbname="$PGDATABASE" /opt/oxide/restore/database.dump
```

使用 `oxide-backup audit` 对生产与恢复库进行 SeaORM 全部业务表计数比较。完整恢复后，将 `recovery.json` 中的密钥恢复为 API 的 `COMMENT_HASH_KEY`，再恢复其他运行配置；Redis 会话与 Tantivy 搜索索引不在数据库备份内，需重新登录并重建搜索索引。七牛网站图片仍保存在原空间中，数据库备份不复制图片文件。

完成检查后安全清除临时明文目录。备份程序持跨进程锁；每次启动会清理强制终止遗留的专用工作目录。请定期关注 1Panel 健康状态，异地备份不能代替恢复私钥的离线保管。
