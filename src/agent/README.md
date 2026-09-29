# Agent 核心

- `src/domain/agent.rs` 定义任务及模型能力。
- `src/agent/summary.rs` 负责摘要输入、提示词和结果约束。
- `src/infrastructure/agent.rs` 解析任务绑定，解密模型密钥并调用 Rig。
- `src/web/admin/agent/` 提供认证、CSRF 和管理端 API；前端仅接收候选结果。

摘要任务使用已绑定的文本模型；未绑定且恰好只有一个可用文本模型时选用该模型。当前支持 OpenAI 兼容 Chat Completions，官方 DeepSeek 地址使用 Rig 的 DeepSeek 适配器。生成不会自动写入文章。

后续扩展：工具调用、MCP、Skill、生图、写作续写及对话任务。
