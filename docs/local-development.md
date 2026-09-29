# Docker 本地开发环境

## 启动依赖

在项目根目录运行：

```powershell
docker compose up -d postgres redis
docker compose ps
```

默认仅监听本机 `127.0.0.1:15432`（PostgreSQL）和 `127.0.0.1:16379`（Redis）。可将 `.env.example` 复制为不纳入版本控制的 `.env` 来修改端口和开发密码；不复制时 Compose 使用相同的默认值。

## 执行数据库迁移

```powershell
docker compose --profile tools run --build --rm migrate
```

迁移容器连接 Compose 内的 `postgres:5432`，按 SeaORM 迁移版本执行尚未应用的结构变更。重复运行不会重复创建表。也可以在宿主机设置 `DATABASE_URL` 后运行 `cargo run -p rust-oxide-migration`。

外部 UUID 的旧数据回填测试需要专用空数据库；创建测试库后设置 `MIGRATION_TEST_DATABASE_URL`，运行 `cargo test -p rust-oxide-migration backfills_existing_rows -- --ignored`。该测试先运行首版迁移、写入旧记录，再验证升级和重复执行。不要指向日常开发库，因为测试会写入样本记录。

## Notion 文章导入

在 `.env` 中配置 `NOTION_API_KEY`，把需要导入的 Notion 页面共享给该集成，运行最新 SeaORM 迁移后重启 API。后台“Notion 导入”可搜索页面、创建草稿及再次同步。Notion 文件图片会转存到当前启用的 OSS，因此含图片的页面须先配置上传源。服务端只读检查不会导入文章；手动按“导入”才会写入数据库和素材库。

## 验证七牛 Kodo 配置

在 `.env` 中配置 `DATABASE_URL` 和用于加密后台凭据的 `COMMENT_HASH_KEY` 后运行：

```powershell
cargo run --example qiniu_probe
```

示例通过 SeaORM 读取已保存的七牛提供商，再用七牛官方 Rust SDK 对每个空间执行最多一条记录的只读列举。命令不会上传、删除或输出对象名和密钥；成功表示当前密钥具有该空间的列举权限，不能代替上传功能测试。

需要验证真实素材通路时运行 `cargo run --example qiniu_probe -- --write`。该模式向每个已配置七牛空间上传随机测试对象、签名读取并删除，不写入博客数据库；失败时只输出阶段结果，不输出密钥。

模型配置可用 `cargo run --example agent_probe` 只读检查。输出模型适配器、能力、主机名、密钥是否已配置，以及任务绑定；不显示密钥。配置和绑定本身不等于推理调用已接入。

## 检查与停止

```powershell
docker compose exec postgres pg_isready -U oxide -d oxide_blog
docker compose exec redis redis-cli ping
docker compose down
```

`docker compose down` 保留命名卷中的数据；需要重新开始时再单独处理卷。当前 Compose 编排数据库、缓存和一次性迁移任务，Web 与 API 进程可在宿主机启动。
