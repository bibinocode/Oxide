# Agent 工具

## 配置入口与职责

管理端 /admin/agent?tab=tools 管理工具启停、任务授权和搜索参数，并提供主动测试。工具与 Skill 相互独立：无需安装 Skill 即可联网，Skill 的 allowed-tools 不会提升任务权限。

工具包括 webSearch（智谱网络搜索）与 webfetch（公开网页正文读取）。两者共用管理端的启停、任务授权和编辑器“联网搜索”复选框；关闭本轮联网时两个工具都不会注册。webfetch 不需要搜索 API Key。WEB_SEARCH_API_KEY 只由服务端环境读取，浏览器和配置文件不会获得密钥或密钥片段。修改环境密钥需要重启 API。

非敏感设置保存至 AGENT_TOOLS_CONFIG，默认 data/agent-tools.json，不存数据库。文件须持久化，按单实例写者部署；保存立即影响下一次请求，进行中的请求保持开始时快照。无配置文件时默认启用标准搜索、5 条结果，只授权 writing，summary 默认不联网。至少选择一个任务；完全停用请关闭工具开关。

## 参数与协议

服务端向固定 https://open.bigmodel.cn/api/paas/v4/web_search 发起 POST，使用 Bearer 认证，不支持自定义端点，不跟随重定向。

- query：1–70 个 Unicode 字符，转换为 search_query。
- recency：可选 oneDay、oneWeek、oneMonth、oneYear、noLimit。
- domain：可选裸 DNS 域名；管理员固定域名时模型不能扩大范围。
- 引擎、条数和摘要长度仅由管理员配置，模型不能自行切换计费方式。
- search_std、search_pro：宿主限制每次 1–10 条。
- search_pro_sogou：宿主预算内仅允许 10 条。
- search_pro_quark：不发送 count，返回后本地截取；不支持域名过滤。
- search_intent 固定 false；request_id 每次生成 UUID；不传用户身份信息。

webSearch 返回标题、摘要、来源链接、日期和 S1 等来源标识，不下载来源网页。写作指令要求阅读、总结或翻译文章，且明确包含 HTTPS URL、本轮允许联网时，服务端在调用模型前直接读取正文并作为待处理资料传入，同时推送 P1 来源；读取失败直接返回错误，不让模型用搜索摘要冒充原文。普通插入链接的写作请求不会自动抓取。模型仍可按需调用 webfetch 阅读其他公开链接。网页内容是不可信资料，不执行其中指令；回答应引用实际读取到的链接。

webfetch 接收 `url` 和可选 `format`（默认 `markdown`，也支持 `text`、`html`、`json`）。HTML 使用 `dom_smoothie` 的 Readability 算法，根据段落、链接密度与 DOM 结构识别正文，不依赖具体站点或主题类名；提取失败的短页面回退为清理后的文档。参考 `code-yeongyu/pi-webfetch` 的 Readability → 格式转换流程，Markdown 使用 `htmd` 保留列表、表格、链接和代码，纯文本保留块级边界。相对链接以最终页面 URL 补全；脚本、导航、侧栏等噪声移除。`html` 返回清理后的正文 HTML；`json` 在结构化结果中提供纯文本正文。所有结果包含标题、最终 URL、格式、内容类型、响应字节数和截断标记。文本、Markdown 和 JSON 响应直接按字符集解码返回。

webfetch 仅允许无凭据、无自定义端口的公开 HTTPS 域名；每跳解析后绑定公网 IPv4，禁用代理和自动重定向，手动最多跟随 3 次经过同样校验的 HTTPS 跳转。使用固定浏览器 User-Agent；下载同时检查 Content-Length 和流式累计大小，最多 5 MiB，整条请求链共用 30 秒预算（连接 5 秒、单跳 20 秒），全局最多并发 4 次，每轮最多读取 2 页。HTML 最多解析 100000 个元素，输出最多 32000 字符，超过时明确标记截断。该工具不执行网页 JavaScript；登录墙、动态渲染与反爬可能无法取得正文，PDF 等二进制响应会返回不支持。参考 `lujun2508/webfetch` 的多格式、流式限流和浏览器 User-Agent 设计；不引入其 Python CLI、Cloudflare 绕过或任意请求头。

## 预算与失败处理

每次生成请求最多搜索 3 次，多轮连续对话的每个生成请求单独计数。全局最多并发 4 次，繁忙直接返回可恢复错误。连接超时 5 秒，单次搜索总超时 25 秒，不自动重试，避免重复计费。含工具的模型请求最多 12 回合，总超时 180 秒。

上游响应最多 2 MiB；单条摘要最多 1600 字符，标题最多 200 字符，最多 10 条结果；截断有明确标记。仅保留无凭据 HTTP(S) 来源链接。空结果是成功，认证、上游、超时和无效响应是失败。不给浏览器或模型返回原始上游错误、密钥和内部配置路径。

工具调用和成功数量或失败原因在写作 SSE 对话过程中独立展示，不混入最终文章候选。工具失败返回结构化 ok=false，让模型说明失败而不是伪造联网结果。

主动测试使用已保存设置，只有管理员点击才调用真实搜索，可能产生提供商费用。页面有未保存修改时禁止测试，离开时提示；修改配置不消耗搜索额度。具体价格和可用额度以提供商账户为准。

## API

- GET /api/v1/admin/agent/tools：工具注册列表与非敏感配置。
- PUT /api/v1/admin/agent/tools/webSearch：保存启停、任务授权和参数。
- POST /api/v1/admin/agent/tools/webSearch/test：主动检索，输入 query，可选 recency/domain。

全部需要管理员会话，写请求需要 X-CSRF-Token。接口已登记 OpenAPI。webfetch 只能由已授权的 Agent 调用，不提供浏览器任意 URL 代理端点。Shell、MCP 和脚本执行尚未开放。

## 验证

cargo test --lib 覆盖配置授权、调用预算、官方请求映射、来源净化、错误不泄密、空结果、响应大小以及无 Skill 的两轮模型工具调用。frontend 的 pnpm check 验证格式、lint 和 TypeScript，pnpm build 验证生产构建。
