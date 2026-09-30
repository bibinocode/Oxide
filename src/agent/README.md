# Agent 核心

- src/domain/agent.rs 定义任务及模型能力。
- src/agent/summary.rs 与 src/agent/writing.rs 负责输入、提示词和候选输出约束。
- src/infrastructure/agent.rs 解析数据库中的模型与任务绑定，解密密钥并调用 Rig。
- src/web/admin/agent/ 提供认证、CSRF 和管理 API；生成不自动写入文章。

摘要使用已绑定的文本模型；未绑定且恰好只有一个可用模型时选用该模型。支持 OpenAI 兼容 Chat Completions，官方 DeepSeek 地址使用 Rig 的 DeepSeek 适配器。

Skill 是文件型技能包：SKILL.md 与 scripts、references、assets 等文件保存在 AGENT_SKILLS_DIR 指定的目录，缺省为 data/agent-skills，不存数据库。先向模型提供名称与描述，再使用 read_skill_file 按需加载核心指导与资源。参见 docs/agent-skills.md。

独立配置入口 /admin/agent 集中管理 Skill 包、模型与工具。支持 ZIP、完整文件夹和公开 GitHub 仓库安装，文件预览、编辑 SKILL.md、启停、导出及卸载。

目前工具包括技能包内只读加载、智谱 webSearch 网络搜索和受限的 webfetch 公开网页正文读取；联网工具独立于 Skill 安装，共用任务授权和启停，参见 docs/agent-tools.md。脚本执行、Shell、依赖安装与 MCP 尚未实现。第三方包保留这些文件及声明，但不能把安装成功等同于已提供运行环境。
