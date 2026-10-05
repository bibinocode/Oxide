# 博客系统架构设计（首版）

## 1. 目标与边界

- 单作者博客，公开内容以桌面端阅读体验为优先，同时适配移动端；视觉风格参考 cali.so。
- Web 界面采用 React + TanStack Start：公开页面和管理端由其负责路由、服务端渲染与交互；文章编辑采用 CodeMirror Markdown 源码与实时预览。
- Axum 0.8.9 负责版本化 API、认证、业务调用、RSS 与素材访问，不承担页面模板渲染。
- PostgreSQL 是业务数据的唯一事实来源；所有数据库访问和迁移使用 SeaORM 2.0 的实体、查询和 Schema API，不编写 SQL 字符串。
- Redis 承担管理员会话、评论限流和热点缓存；Tantivy 在本地磁盘保存文章搜索索引，jieba-rs 负责中文分词。
- 首版部署一个 TanStack Start Web 进程和一个 Axum API 进程。Tantivy 的索引写入器只由一个 Axum 实例持有；扩展到多个 API 实例前需要改造索引分发方案。
- 付费专栏、独立读者身份、微信订单与数据库订阅权益见 [付费专栏说明](paid-columns.md)；付费正文只在 Axum 验证权益后返回。
- 公开文章支持 RSS 2.0 订阅；评论匿名提交，审核后展示。

## 2. 运行时组成

```text
浏览器 / RSS 阅读器
        |
        v
同源入口（反向代理）
  |-- 页面请求 -> TanStack Start（React SSR、路由、元数据）
  `-- /api/*、/feed.xml、/media/* -> Axum 0.8.9
        |
        v
Axum API -> 业务服务层
  |-- SeaORM -> PostgreSQL（内容、评论、素材元数据、索引任务）
  |-- Redis（会话、限流、缓存）
  |-- jieba-rs + Tantivy（文章检索）
  `-- StorageProvider -> 阿里云 OSS / 七牛云 Kodo
```

Rust 后端仍使用单个二进制和清晰的内部模块。TanStack Start 是独立的 Web 运行时，公开页面优先在服务端获取数据并输出可阅读的 HTML；CodeMirror 只在管理端编辑页加载。Web 服务不直接连接 PostgreSQL、Redis、Tantivy 或 OSS，业务访问统一经过 Axum API。

浏览器始终访问同一个站点域名。反向代理将页面交给 TanStack Start，将 `/api/*`、`/feed.xml`、`/media/*`、`/health/*` 交给 Axum，避免跨域会话与双重 Cookie 策略。TanStack Start 的服务端加载器通过内部 API 地址调用 Axum，并按请求转发管理员 Cookie；浏览器请求使用同源路径。转发必须限定请求范围，不能把某位管理员的 Cookie 放进全局客户端或共享缓存。

## 3. 仓库结构

```text
src/
  main.rs                 # 启动、配置加载、迁移、路由组装和后台任务
  config.rs               # 环境配置及校验
  state.rs                # 共享连接池和服务句柄
  web/
    mod.rs                # 共享状态、响应类型、路由装配与 OpenAPI
    client/               # 公开文章、分类、评论、搜索、RSS 与素材读取
    admin/                # 登录、内容、评论审核、存储和站点设置
  domain/
    article/              # 草稿、发布、修订、分类和标签规则
    comment/              # 提交、审核、回复和头像标识
    asset/                # 上传校验、元数据和提供商选择
    search/               # 分词、索引任务、检索和重建
    feed/                 # RSS 生成
  entity/                 # SeaORM 2.0 实体及关联
  infrastructure/
    storage/              # StorageProvider trait 与 OSS/Kodo 实现
    cache/                # Redis 适配器
    search/               # Tantivy 索引适配器
migration/                # SeaORM 迁移 crate，使用 Schema API
frontend/                 # TanStack Start 应用（独立 package 与锁文件）
  src/
    routes/               # 文件路由；页面、loader、元数据与错误边界
    features/             # article、comment、contact、editor、home、admin
      admin/              # agent、home、settings 子功能；会话上下文留在 admin 根目录
      contact/            # 资料解析、服务端抓取与悬停卡片
    components/
      ui/                 # Button、Input、Dialog 等通用组件
      layout/             # 站点导航、页脚、管理端布局
    hooks/                # 跨功能复用的交互 Hook
    lib/
      api/                # 类型化 API 客户端、请求上下文、错误映射
      utils/              # 无状态工具函数
    styles/
      tailwind.css        # Tailwind CSS 4 与主题 token 入口
      app.scss            # 业务样式入口，按原层叠顺序加载 partial
      tokens.css          # Tailwind @theme 与运行时语义变量
      admin/ article/ contact/ editor/ home/ layout/ base/ shared/
                           # 按功能或跨功能职责拆分的 SCSS partial
    router.tsx            # TanStack Router 配置
  public/                 # 字体、站点图标等静态资源
tests/                    # 关键业务流程与接口集成测试
docs/                     # 设计和部署文档
```

`domain` 不依赖 Axum 的请求类型；API 路由负责解析与响应，业务服务负责规则和事务，`infrastructure` 封装外部系统。前端 `routes` 只负责页面组合和加载入口，功能代码放在 `features`；实际实现时如模块较小，可先合并文件，不为了目录结构增加空抽象。

## 4. 前端架构与设计体系

### 路由、数据与状态

- 公开路由包括首页、文章、分类、标签、归档、搜索和关于页；`/admin/*` 使用独立布局和登录保护。路由负责 SSR 数据加载、页面元数据、错误边界和组件组合。
- 首屏数据在 TanStack Start 的服务端 loader 获取并随 HTML 交付，避免浏览器加载后再发起首屏请求。客户端导航沿用路由加载机制；编辑、评论审核等写操作通过类型化 API 客户端完成。
- TanStack Start 的 server function 只作为请求范围内的 BFF 入口，不实现业务规则，也不直接访问数据库。管理员鉴权由 Axum 校验；Web 层可以提前重定向，但不能把它当作唯一权限检查。
- Axum 提供 `/api/v1/*` JSON 契约。Rust 响应模型生成 OpenAPI 描述，前端从描述生成 TypeScript 类型；业务数据、字段校验和错误码以 API 为准。生成文件不手改。
- 公开文章 HTML 和后台预览均使用 Axum 的同一渲染器；预览请求防抖并取消过期请求。文章标题、主图、元信息、正文及 Shiki 高亮在前后台共用 `ArticlePresentation`。发布属性在点击发布后集中填写，日常写作只显示标题和源码/预览双栏。
- 首版不引入全局状态容器。路由 loader 管服务端数据，组件局部状态管交互；只有明确出现跨页面共享状态时再增加状态工具。

### 组件与 Hook 边界

| 层级 | 职责 | 示例 |
| --- | --- | --- |
| `components/ui` | 无业务含义的可访问交互原件；状态、尺寸、外观通过明确属性控制 | `Button`、`TextField`、`Dialog`、`Tabs` |
| `components/layout` | 页面框架与响应式导航 | `SiteHeader`、`PublicShell`、`AdminShell` |
| `features/<name>/components` | 某个功能自己的展示与交互 | `ArticleCard`、`CommentThread`、`EditorToolbar` |
| `features/<name>/hooks` | 复用的功能交互与副作用 | `useArticleEditor`、`useAutosave`、`useCommentForm`、`useAssetPicker` |
| `lib/api` | 请求封装、类型与错误解析；不包含 React 状态 | `getArticle`、`publishArticle` |

组件通过属性接收数据和回调，不直接依赖全局请求客户端。Hook 处理表单、编辑器生命周期、自动保存和素材选择等有实际复用价值的行为；简单局部状态保留在组件内。Markdown 工具栏、旧文档转换和上传适配集中在 `features/editor`，编辑器代码按路由拆包，不进入公开文章页的客户端包。

### 设计变量与视觉规范

- 样式采用 Tailwind CSS 4 与 SCSS。`tailwind.css` 只加载 Tailwind 与 `tokens.css`，使 `@theme` 与工具类在同一编译入口；`app.scss` 按功能加载局部 partial。两份入口按此顺序在根路由静态导入，由 TanStack Start 资源清单输出首屏 stylesheet 链接，业务规则始终覆盖工具类基础层；禁止手动使用 `?url` 拼接样式链接，避免 SSR 与客户端独立编译时产生不同哈希、导致首屏 CSS 404。`tokens.css` 集中定义纸张底色、灰阶、单点暖色信号、字体与 600px 网格 / 552px 阅读栏。组件优先使用语义变量和 Tailwind 工具类。
- 视觉规则对照 [cali.so dev 分支](https://github.com/CaliCastle/cali.so/tree/dev) 的 `docs/design-language.md`、`app/_views/home-page.tsx`、`app/_views/blog-index-page.tsx`、`components/post-row.tsx`：小号无衬线排版、窄栏、虚线列导轨、编号排线小节、标题与日期组成的点线目录行、底部固定导航。本站不使用参考站作者的肖像或文章素材。
- `styles/base` 和 `styles/shared` 承载基础元素、通用控件与动效；`layout`、`article`、`contact`、`editor`、`home`、`admin` 按界面职责维护规则。局部页面布局仍使用 Tailwind。彩色像素只在每页标题附近出现一次，其余文字、边框和按钮保持中性灰阶。
- 公开页使用 552px 阅读栏；管理端独立于公开页，常规页面采用全屏侧栏与内容区。文章工作区隐藏侧栏并占满视口，桌面端左右等宽：左侧 Markdown 原文，右侧使用前台共用的文章组件实时预览；窄屏切换源码和预览。移动端给正文保留 16px 边距，并为公开页固定导航预留底部空间。
- 所有交互组件覆盖默认、悬停、焦点、禁用、加载和错误状态；表单具备标签、错误文本和键盘操作。图标按钮带可访问名称及提示文字。
- PC 端优先设计内容扫描和编辑效率；移动端重排布局和工具栏，不让文字溢出、控件重叠或内容被固定区域遮挡。
- 设计变量、基础组件和页面模式先在一页内部样例中验证，再扩展到公开页与管理端；首版不单独发布组件库。

### 前端质量门禁

- 使用 Oxc 生态的 `oxlint` 做 lint、`oxfmt` 做格式化；同时执行 TypeScript 类型检查。规则覆盖 React Hook、可访问性、未使用代码与导入质量。
- 关键 UI 流程使用浏览器测试验证：公开文章 SSR、搜索、评论提交、管理员登录、文章编辑与素材上传。组件测试集中在复杂交互，避免只复述实现的快照测试。
- 前端依赖锁定在 `frontend` 的锁文件中，开发与构建脚本固定命令；上线构建产物与 Rust API 按同一版本部署。

## 5. 核心数据模型

| 实体 | 主要字段或关系 | 约束与用途 |
| --- | --- | --- |
| `admin_users` | ID、用户名、Argon2 密码摘要、创建时间 | 首版只允许一个管理员；不提供公开注册 |
| `articles` | 内部 ID、外部 UUID、slug、标题、摘要、Markdown JSON（兼容旧 Tiptap JSON）、净化后的 HTML、封面素材 ID、状态、发布时间、更新时间 | `draft` / `published`；外部 UUID 与 slug 分别唯一；公开查询只返回已发布内容 |
| `article_revisions` | 文章 ID、文档 JSON、保存时间 | 保留可恢复的编辑历史；限制保留数量 |
| `categories` / `tags` | 名称、slug | 分类与标签分别管理，slug 唯一 |
| `article_categories` / `article_tags` | 文章 ID、分类或标签 ID | 使用关联实体维护关系 |
| `comments` | 内部 ID、外部 UUID、文章 ID、父评论 ID、昵称、邮箱标识、正文、状态、创建时间 | 匿名提交；邮箱不公开；头像由邮箱标识生成；待审核评论不进入公开查询 |
| `assets` | 内部 ID、外部 UUID、provider ID、对象键、MIME、大小、尺寸、可见性、上传时间 | 素材与存储提供商绑定，文章内部引用素材 ID，对外地址使用 UUID |
| `storage_providers` | 稳定 ID、类型、公开域名、端点、空间、区域、加密凭据、启用状态 | 凭据在服务端认证加密，旧记录仍可用环境变量 |
| `agent_providers` / `agent_bindings` | 模型协议、能力、模型 ID、加密 API Key、任务映射 | Agent 配置与公开站点信息分离，按任务选择模型 |
| `site_settings` | 站点名称、简介、基础 URL、当前上传提供商等 | 管理端可修改非敏感配置 |
| `search_jobs` | 文章 ID、动作、状态、重试次数、更新时间 | 与文章变更在同一数据库事务内写入；不对文章建外键，以便删除文章后继续清理索引 |

首版文章修订、分类和标签的具体字段在迁移实施前再定，但上述身份、关系和一致性边界保持稳定。评论邮箱提交后仅保存规范化邮箱的带密钥摘要，用于稳定头像与滥用控制；不保存或展示原邮箱，因此也不提供邮件通知。头像由服务端基于该标识生成几何图案并缓存，不调用第三方头像接口。

文章、素材和评论保留内部 `bigint` 主键以连接数据；对外 DTO 只返回 UUIDv4 `public_id`，不序列化内部主键或外键。页面文章地址仍可使用 slug。UUID 只用于隐藏顺序与稳定定位，管理员鉴权和素材可见性检查仍由 Axum 执行。

## 6. 关键流程

AI 写作、FIM、配图和审核 Agent 的后续实施边界见 [AI 写作设计](ai-assistant.md)。当前独立 Agent 领域层定义任务及能力，`agent_providers` 注册模型，`agent_bindings` 指定写作、摘要、图像、对话模型。Skill 采用独立文件目录存储，通过 ZIP、文件夹和 GitHub 安装完整包，按需读取核心文档与配套资源，不进入数据库；实现与兼容性边界见 [文件型 Agent Skills](agent-skills.md)。脚本执行、通用工具授权与 MCP 后续独立扩展。公开模块配置通过 `site_settings.presentation` 保存，模型凭据不进入公开站点配置。

### 文章编辑与发布

图像模型按 `image` 任务绑定，通过管理员生图接口返回私有 OSS 素材；发布面板可采用为主图，正文编辑器可插入为 Markdown 图片。首页文章列表从公开素材批量读取主图 URL，写作归档承载分类与标签入口，首页不再展开分类区。

1. 管理员会话通过 Redis 保存，浏览器只持有 `HttpOnly`、`Secure`、`SameSite` Cookie；Axum 写接口验证 CSRF。TanStack Start SSR 请求按请求转发 Cookie。
2. 编辑器提交 `{ "type": "markdown", "source": "..." }`。服务端限制文档大小，解析 Markdown、将原生 HTML 当作文本处理并净化生成的 HTML；旧版 Tiptap JSON 继续支持读取和渲染。
3. 保存文章、关联关系、修订记录和搜索任务于同一 PostgreSQL 事务。发布时设置发布时间并失效相关 Redis 缓存。
4. 后台任务消费 `search_jobs`，对 Tantivy 执行幂等更新；失败保留任务并重试。搜索可能短暂落后于数据库，但任务不会因进程重启而丢失。

### 中文分词搜索

- 只索引已发布文章的标题、摘要、正文纯文本、分类和标签；标题权重最高。
- 写入和查询使用同一 jieba-rs 分词规则；支持中文词语与英文关键词。
- 搜索结果按相关性排序并分页，摘要高亮使用安全转义后的文本。
- 提供 `reindex` 管理命令：从 PostgreSQL 全量重建索引，再切换到新索引目录。索引损坏或版本升级后可恢复，不以索引作为内容备份。

### 素材上传与提供商切换

1. 上传时检查实际文件类型、大小和图片尺寸，按 `assets/images/YYYY/MM/{public_id}.{ext}` 分配对象键，调用当前 `StorageProvider`。目录使用上海时区的上传年月，UUID 避免重名覆盖；手动上传和 Notion 导入共用领域层目录规则，路径和元数据使用同一个上传时间。
2. 数据库记录素材内部 ID、外部 UUID、提供商 ID 与对象键。正文引用本站稳定地址 `/media/{public_id}`，由同源代理转发给 Axum，再根据记录解析到对应提供商。
3. 管理端切换当前提供商只影响新上传；旧素材继续由原提供商提供。不能在旧素材仍被引用时删除其配置。
4. 历史迁移放在后续阶段：复制对象、验证校验值、更新素材记录后再清理旧对象。首版不自动迁移。
5. 未发布素材只允许管理员预览；发布引用后可公开访问。上传凭据只在服务端使用。
6. 后台素材库按上传年月展示，使用公开 UUID 游标加载历史记录，并展示实际提供商与对象路径。旧 `assets/{uuid}.{ext}` 对象继续按数据库记录读取；目录规则变更不会改写历史路径。迁移时需同时备份素材元数据与对象，保留 UUID 和可见性，复制校验后才切换数据库位置。

### 写作归档

- 普通文章按首次发布时间倒序分页，每页按上海时区的年份分组，并显示月日；年份归档与主题分类独立，未分类文章同样可被浏览。
- 同一年跨越多页时，每页继续显示年份标题；总篇数使用数据库分页总数。小册章节通过小册目录组织，不混入普通写作归档。

### 匿名评论

- 访客填写昵称、邮箱和正文；邮箱只用于计算带密钥标识，不公开展示。
- 新评论和回复默认 `pending`，审核通过后才进入公开页面。首版回复层级限制为一层，以维持阅读布局。
- Redis 按来源和文章限制提交频率；服务端另行限制正文长度、链接数量并净化输出。Redis 不可用时暂停匿名提交，避免绕过限流。
- 管理员可审核、拒绝和删除评论；公开页面只展示已审核评论。

### RSS 与缓存

- `/feed.xml` 输出 RSS 2.0，包含最近发布文章的标题、摘要、发布时间和绝对永久链接；不包含草稿及评论。
- 站点 URL 从配置生成，响应提供正确的内容类型和缓存标头。
- Redis 缓存首页、文章详情及 RSS 的可复用数据；发布、撤回和修改文章后主动失效。Redis 故障时公开阅读回退到 PostgreSQL。

## 7. 路由边界

| 路由示例 | 负责人及用途 |
| --- | --- |
| `/`、`/articles/{slug}` | TanStack Start：首页和文章详情 SSR |
| `/categories/{slug}`、`/tags/{slug}`、`/archive` | TanStack Start：内容浏览 SSR |
| `/search?q=...` | TanStack Start：分词检索结果 SSR |
| `/admin/*` | TanStack Start：登录后的内容、评论、素材与设置管理 |
| `/api/v1/*` | Axum：公开与管理 JSON API |
| `/feed.xml` | Axum：RSS 订阅 |
| `/media/{public_id}` | Axum：使用 UUID 的稳定素材地址 |
| `/health/live`、`/health/ready` | Axum：存活与依赖就绪检查 |

评论查询和匿名提交走 `/api/v1/articles/{slug}/comments`；管理写操作走 `/api/v1/admin/*`。所有列表设分页上限，避免单次加载大量数据。反向代理为上述路径保留固定转发规则，其余页面请求交给 TanStack Start。

## 8. 实施顺序与验收

1. **基础骨架**：Axum API、配置、日志、SeaORM 迁移、PostgreSQL/Redis 连接；建立 TanStack Start、同源代理、设计变量、基础布局及 `oxlint`/`oxfmt`/类型检查。验收：两个进程可启动，页面可 SSR，健康检查可用，迁移可重复执行。
2. **内容管理**：管理员初始化与登录、文章、分类、标签、修订、Markdown 编辑与发布。验收：未登录不能管理内容，草稿不可公开，发布后可按永久链接阅读，刷新后文档内容不丢失。
3. **搜索**：jieba-rs + Tantivy、索引任务和重建命令。验收：中文词语能匹配，撤回文章从结果消失，重启后索引可恢复。
4. **公开体验与 RSS**：TanStack Start 首页、详情、归档、搜索页、关于页、SEO、响应式样式和 Axum `/feed.xml`。验收：无需 JavaScript 可阅读，RSS 阅读器能识别已发布文章。
5. **评论**：匿名表单、稳定头像、回复、审核和限流。验收：待审核评论不可公开，邮箱不出现在响应中。
6. **素材与云存储**：素材库、阿里云 OSS、七牛云 Kodo、切换逻辑。验收：切换后新素材使用新提供商，旧文章图片仍可访问。
7. **收尾**：缓存策略、性能与内存检查、错误页、部署配置、安全检查和关键集成测试。

每阶段完成后运行 Rust 格式检查、Clippy、相关测试，以及前端 `oxlint`、`oxfmt`、类型检查和页面验证，再进入下一阶段。部署前需要 PostgreSQL、Redis、可写的 Tantivy 索引目录、TanStack Start 运行时，以及至少一个已配置的对象存储提供商。

### 独立作品集模块

作品集使用 `/projects` 路由，首页不再嵌入项目列表。`presentation.projects.enabled` 同时控制独立页面、悬浮导航与页脚入口；关闭时页面返回未找到状态，已有条目保留。导航连续按键 `G`、`P` 仅在模块开启时可用。项目仍在站点设置中维护，数组顺序即展示顺序。
