# Oxide Docker 生产部署

以下使用 `example.com` 与 `/opt/oxide` 演示部署。将域名、目录和 OpenResty 容器名称替换为自己的配置；1Panel OpenResty 负责 HTTPS 和反向代理，应用使用独立 Compose 项目 `oxide`。

## 服务器目录

- `/opt/oxide/stack/compose.yaml`：PostgreSQL、Redis、API、前端编排。
- `/opt/oxide/stack/.env`：镜像版本和数据库、Redis 密码，权限 600。
- `/opt/oxide/stack/app.env`：后端服务密钥及生产配置，权限 600。
- `/opt/oxide/stack/data`：搜索索引、Skill、工具设置，容器 UID/GID 为 10001。
- `/opt/oxide/releases/20261005`：源码制品、构建日志、镜像归档及校验文件。
- `/opt/oxide/backups`：数据库备份，仅 root 可读。
- `/opt/1panel/www/conf.d/oxide-example.com.conf`：1Panel OpenResty 代理配置。
- `/opt/1panel/www/sites/example.com/ssl`：OpenResty 使用的证书。

生产前端运行 Nitro 构建制品，不运行 Vite。只有前端的 `127.0.0.1:18080` 映射到宿主机；数据库、Redis、API 不发布宿主机端口。容器之间使用内部网络访问，所有服务配置健康检查和自动重启。

## 日常管理

在 1Panel 的容器列表中管理 `oxide-*` 容器；需要统一编排时，导入上述 Compose 文件，并保持工作目录及环境文件一致。网站代理配置已写入现有 OpenResty；如果以后在 1Panel 新建同域名网站，请先合并此配置，避免重复 `server_name`。

```sh
cd /opt/oxide/stack
docker compose ps
docker compose logs --tail=100 api frontend
docker compose restart api frontend
/opt/oxide/stack/backup.sh
```

**不要运行 `docker compose down -v`**，它会删除 PostgreSQL 和 Redis 数据卷。

## 数据迁移

首次部署使用本地 PostgreSQL 的 custom-format 一致性快照，导入全新的 `oxide` 数据卷。管理员、文章、草稿、分类、小册、订单、站点设置和存储配置一并迁移。对象存储保持原对象和引用，`COMMENT_HASH_KEY` 必须保持一致，才能解密原存储配置。Redis 会话不迁移，上线后需要重新登录。Tantivy 索引由服务端重建。

后续同步必须先备份生产数据库，核对生产期间新增的数据，再选择合并或替换；不得直接重复导入同一快照到使用中的数据库。

## 版本更新与回退

用 `deploy/Dockerfile.api` 和 `deploy/Dockerfile.frontend` 构建两个镜像，标签使用新版本；上传镜像后修改 `.env` 的 `RELEASE`，执行 `docker compose up -d`。生产构建固定 npm 11.11.0，与当前锁文件生成版本一致。

数据库结构升级使用镜像中的 SeaORM 迁移程序：

```sh
cd /opt/oxide/stack
./backup.sh
docker compose run --rm --entrypoint /usr/local/bin/oxide-migrate api up
docker compose up -d
```

回退应用时修改 `RELEASE` 为上一版本。涉及数据库结构变化时，先验证旧代码兼容性；数据库恢复会丢弃快照之后的数据，必须单独确认恢复范围。

## HTTPS

Certbot 的定时器负责续签。`/etc/letsencrypt/renewal-hooks/deploy/oxide-openresty.sh` 同步更新证书，测试 nginx 配置后重载现有 OpenResty。模板仅处理 `example.com`；使用前替换域名与证书目录，并在续签脚本环境设置 `OPENRESTY_CONTAINER` 为自己的 OpenResty 容器名称。

## 自动异地备份

生产环境每 72 小时向现有七牛空间上传加密数据库快照，验证成功后删除旧快照。密钥保管、状态检查及恢复步骤见 [BACKUP.md](BACKUP.md)。
