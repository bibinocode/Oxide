# Axum API 对接

Axum 0.8.9 默认监听 `127.0.0.1:3001`。TanStack Start 开发服务监听 `127.0.0.1:3000`，同源代理 `/api/*`、`/feed.xml`、`/media/*` 和 `/openapi.json`。生产部署时也需配置同等反向代理；SSR 进程用 `API_INTERNAL_URL` 访问 Axum，默认 `http://127.0.0.1:3001`。

## 本地启动

1. 复制 `.env.example` 为 `.env`，设置 `DATABASE_URL`、`REDIS_URL` 和至少 32 字节的 `COMMENT_HASH_KEY`。进程环境变量优先于 `.env`。
2. 启动 PostgreSQL 和 Redis：`docker compose up -d postgres redis`。运行数据库迁移：`docker compose --profile tools run --rm migrate`；若在宿主机运行迁移程序，先将 `DATABASE_URL` 导入进程环境，再执行 `cargo run -p rust-oxide-migration`。
3. 首次启动前设置 `ADMIN_USERNAME` 与至少 12 字符的 `ADMIN_PASSWORD`，再运行 `cargo run --bin rust-Oxide`。仅管理员表为空时创建账号；之后可从配置中移除这两个值。
4. 在 `frontend/` 运行 `npm install`、`npm run dev`，打开 `http://127.0.0.1:3000/`。

常用配置：`APP_BIND` 默认为 `127.0.0.1:3001`，`DATABASE_MAX_CONNECTIONS` 默认为 10，`RUST_LOG` 默认为 `rust_oxide=info,tower_http=info`，`PUBLIC_BASE_URL` 用于 RSS 绝对链接，`SEARCH_INDEX_DIR` 默认为 `./data/search-index`。HTTPS 部署时设置 `SESSION_SECURE=true`。连接 URL、密码和密钥不要写入日志或提交到仓库。

## 接口清单

以下路径均已在 `GET /openapi.json` 中描述；`GET /docs` 提供 Swagger UI。时间以 UTC RFC 3339 表示。公开资源使用 UUID `public_id`，不返回内部数据库 ID。

| 方法 | 路径 | 用途 |
| --- | --- | --- |
| GET | `/health/live`、`/health/ready` | 进程存活、PostgreSQL 与 Redis 就绪检查 |
| GET | `/api/v1/site` | 公开站点信息及 presentation 中的页脚、联系方式、作品集、服务模块 |
| GET | `/api/v1/link-preview?url=...` | 外链标题、简介、图标及 Open Graph 图片，按需抓取并缓存 |
| GET | `/api/v1/articles`、`/api/v1/articles/{slug}` | 已发布文章分页列表与详情 |
| GET | `/api/v1/categories`、`/api/v1/tags` | 分类与标签列表 |
| GET | `/api/v1/categories/{slug}/articles`、`/api/v1/tags/{slug}/articles` | 按分类或标签分页列出文章 |
| GET | `/api/v1/search?q=关键词` | Jieba 分词与 Tantivy 持久化索引搜索 |
| GET | `/feed.xml` | 已发布文章的 RSS 订阅 |
| GET、POST | `/api/v1/articles/{slug}/comments` | 已审核评论列表、匿名评论提交 |
| GET | `/api/v1/comments/{public_id}/avatar.svg` | 邮箱稳定生成的匿名头像 |
| POST | `/api/v1/admin/login`、`/api/v1/admin/logout` | 登录和退出 |
| GET | `/api/v1/admin/session` | 当前会话与 CSRF 令牌 |
| GET、POST | `/api/v1/admin/articles` | 管理文章列表、创建草稿 |
| GET、PUT、DELETE | `/api/v1/admin/articles/{public_id}` | 读取、编辑、删除文章 |
| POST | `/api/v1/admin/articles/{public_id}/publish`、`/unpublish` | 发布与撤回 |
| GET | `/api/v1/admin/articles/{public_id}/revisions` | 修订历史 |
| GET、PUT | `/api/v1/admin/articles/{public_id}/taxonomy` | 文章分类与标签关联 |
| GET、PUT | `/api/v1/admin/articles/{public_id}/cover` | 主图；写入 `{ "asset_public_id": "素材 UUID" }`，移除时该字段为 `null`，只允许公开图片 |
| POST | `/api/v1/admin/preview` | 使用正式渲染器生成预览；请求 `{ "document": { "type": "markdown", "source": "..." } }`，返回 `{ "html": "..." }` |
| GET | `/api/v1/admin/comments` | 待审核及历史评论 |
| PATCH、DELETE | `/api/v1/admin/comments/{public_id}` | 审核、拒绝或删除评论 |
| PUT | `/api/v1/admin/site` | 更新站点信息 |
| POST | `/api/v1/admin/categories`、`/api/v1/admin/tags` | 创建分类、标签 |
| PUT、DELETE | `/api/v1/admin/categories/{slug}`、`/api/v1/admin/tags/{slug}` | 修改、删除分类或标签 |
| GET、POST | `/api/v1/admin/storage-providers` | 列出、创建 OSS/Kodo 提供商 |
| PUT、DELETE | `/api/v1/admin/storage-providers/{id}` | 修改、删除提供商 |
| POST | `/api/v1/admin/storage-providers/{id}/activate` | 切换新上传使用的提供商 |
| GET、POST | `/api/v1/admin/assets` | 素材列表、图片上传 |
| PATCH、DELETE | `/api/v1/admin/assets/{public_id}` | 素材公开状态、删除 |
| GET | `/media/{public_id}` | 稳定的素材访问地址 |
| GET、POST | `/api/v1/admin/agent/providers` | 模型提供商列表、创建配置 |
| PUT、DELETE | `/api/v1/admin/agent/providers/{id}` | 更新或删除模型提供商 |
| GET | `/api/v1/admin/agent/bindings` | 查询任务到模型的绑定 |
| PUT | `/api/v1/admin/agent/bindings/{task}` | 绑定 writing、summary、image、chat 任务 |

文章、分类、标签及搜索列表使用 `page`（默认 1）和 `per_page`（默认 20，最多 50），响应包含 `items`、`page`、`per_page`、`total`。新文章正文提交 `document: { "type": "markdown", "source": "## 标题\n正文" }`；已有 Tiptap JSON 文档仍可读取和更新。公开详情返回服务端生成并净化的 `rendered_html`，不接受前端直接提交 HTML。匿名评论需提交 `nickname`、`email`、`body`，可选 `parent_public_id`；新评论先进入 `pending` 状态，公开接口不返回邮箱。错误统一为 `{ "code": "...", "message": "..." }`，前端应按 `code` 处理；精确请求体、状态码与字段以 OpenAPI 为准。

管理员接口使用 `HttpOnly` 会话 Cookie。登录或读取会话后，所有写请求携带响应中的 `csrf_token`，请求头名为 `X-CSRF-Token`。浏览器通过同源路径发送请求。

文章详情新增 `cover_url`，为空时不渲染主图占位。后台主图响应为 `asset_public_id` 和 `media_url`，不返回素材内部 ID。新草稿无需用户填写 slug，编辑器先生成稳定临时地址，发布面板可修改。

`presentation` 包含 `footer_text`、`contacts`、`projects`、`services`。后三者结构为 `{ "enabled": false, "items": [{ "title": "名称", "url": "https://example.com", "description": "说明", "avatar_url": "https://example.com/avatar.jpg", "stat_text": "128 位订阅者" }] }`。`avatar_url` 和 `stat_text` 可选，用于联系方式悬停卡片；旧配置无需迁移。关闭模块保留条目，数组顺序即展示顺序。保存站点时省略 `presentation` 将保留旧值；不在此对象保存私密 AI 或 OSS 配置。数据库需应用 `m20260928_000003_site_presentation` 迁移。

## 搜索与素材

搜索索引保存在 `SEARCH_INDEX_DIR`。发布、撤回、编辑文章后由后台任务同步；更换索引目录或需要全量修复时，停止 API 进程并运行 `cargo run --bin rust-Oxide -- --reindex`，随后重启 API。索引可从 PostgreSQL 重建。

在管理端创建提供商时，`id` 使用 2 至 40 位大写字母、数字或下划线，`kind` 为 `aliyun_oss` 或 `qiniu_kodo`。后台可直接填写 HTTPS `endpoint`、`bucket`、`region`、`access_key` 和 `secret_key`。七牛填写对应区域的 S3 兼容端点，公开域名填绑定的 CDN 域名。保存后在素材存储列表将其设为当前上传源，然后到素材库上传。更新时 AK/SK 同时留空表示保留现有凭据；响应仅返回 `credentials_configured`，不返回密钥。旧环境变量仍兼容，例如 ID 为 `MY_OSS` 时：

```text
STORAGE_MY_OSS_ENDPOINT=https://oss-cn-hangzhou.aliyuncs.com
STORAGE_MY_OSS_BUCKET=my-bucket
STORAGE_MY_OSS_REGION=oss-cn-hangzhou
STORAGE_MY_OSS_ACCESS_KEY=...
STORAGE_MY_OSS_SECRET_KEY=...
```

数据库中的凭据使用 AES-256-GCM 加密，密钥从服务端 `COMMENT_HASH_KEY` 用途隔离派生。生产环境必须使用随机值并保管；更换该值前需重新录入所有 OSS 和 Agent 凭据，否则旧密文无法解密。激活提供商只影响后续上传；旧素材仍记录原提供商，访问地址保持 `/media/{public_id}`。目前上传仅接受 PNG、JPEG、GIF、WebP，路由请求体上限为 9 MiB。云端读写需用真实账号和 bucket 单独验证。

## Agent 与外链

Agent 提供商配置独立于公开站点设置。`adapter` 可为 `openai_compatible` 或 `jimeng`，`capability` 为 `text` 或 `image`，`model_id` 填提供商真实模型 ID。创建时需提供 `api_key`，更新时留空保留原值。绑定任务时服务端校验模型已启用且能力匹配。此阶段实现模型注册和任务路由，模型请求、Skill/MCP 安装、工具调用与图像生成任务尚未开放执行接口，不能把配置完成视为已可生图。

正文中写普通 Markdown 链接即可：`[网页标题](https://example.com/article)`。服务端渲染器继续净化 URL；前台与编辑器右侧共用外链增强，自动补 favicon，鼠标或键盘聚焦时请求网页预览。触屏设备保持普通可点击链接。抓取仅允许公网 HTTP(S)，禁止重定向，限制响应大小和耗时，Redis 缓存一天；目标网页拒绝抓取或没有 Open Graph 图片时只显示可取得的文字信息。
