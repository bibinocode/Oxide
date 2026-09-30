# 文件型 Agent Skills

## 纠正后的实现

Skill 是一个完整目录，不是数据库中的提示词记录。必需文件为 SKILL.md，开头使用 YAML frontmatter 声明 name 和 description；正文是技能指导。目录内可以包含 scripts、references、assets、模板、示例、许可证与任意其他普通文件。

技能包与管理状态均不存数据库；数据库只继续保存原有模型配置及任务绑定。

默认仓库为 data/agent-skills，使用 AGENT_SKILLS_DIR 可以设置其他目录。一个安装后的包位于该目录下以 name 命名的子目录。第三方包的文件内容原样保存；.state.json 在包之外记录启停与来源，不写数据库，不向模型暴露服务器绝对路径。文件目录需要持久化挂载；容器重建不能丢失此目录。管理进程按单个仓库单写者部署，不提供跨进程文件锁。

## 独立管理界面

/admin/agent 包含 Skills、模型与任务、工具三个分类。站点设置仅保留跳转入口。

- ZIP 安装：支持单技能包、带外层目录的 ZIP，以及指定子目录的多技能仓库 ZIP。
- 文件夹安装：浏览器选择完整目录，通过相对路径上传全部文件。
- GitHub 安装：公开仓库主页或 tree 链接，可设置 revision 和技能子目录。仓库主页缺省使用 main；其他默认分支须显式指定。含斜线的分支名使用仓库主页 + revision + 子目录分别指定。
- 已安装包：搜索、启停、查看来源和兼容性、完整文件列表、文本预览、二进制下载、完整 ZIP 导出与卸载。
- 编辑 SKILL.md：直接编辑原始 YAML + Markdown，保存不会改动附属文件；name 必须与安装目录一致。
- 同名包不会静默覆盖；勾选替换并确认后覆盖完整目录。新安装和替换后都默认禁用。

可直接向仓库放入标准技能目录，再在页面启用。直接改动磁盘内容应由管理员负责协调；不要在安装、导出或正在运行的模型请求期间并发改写同一目录。

## 渐进式加载

写作、编辑器 AI 对话及摘要任务使用同一文件型 Skill 服务：

1. 请求开始仅发现已启用包的名称和描述，不把技能正文或附属资源全部注入 system prompt。
2. 模型根据用途选择相关技能，调用只读工具 read_skill_file 加载 SKILL.md。
3. 需要参考资料、脚本源码或其他文本资源时，再按包内相对路径读取。读取 SKILL.md 时同时返回轻量文件目录，便于发现资源。
4. 工具支持 Unicode 字符 offset 分段读取，每段最多 16000 字符。一次请求最多 12 次读取，累计最多 64000 字符；模型调用回合最多 12。
5. 未启用、未被本次发现授权的包及越界路径不可读取。没有可用 Skill 时不注册读取工具，保留原模型调用行为。
6. 支持 disable-model-invocation: true；此类技能不自动发现，用户在编辑器指令开头明确使用 /name 时才加入本次可用范围。

模型需支持标准 tool / function calling。已启用 Skill 会引入多轮工具调用，可能增加延迟与 token 消耗。生成结果仍经过写作和摘要原有输出校验；不自动保存文章。SSE 展示工具调用与完成过程，最终候选取最后一轮的正文，不混入工具调用前的说明或文件内容。

## 明确的执行边界

第三方 scripts 和二进制 assets 会完整保存、下载和导出。当前支持包内只读文件加载以及独立授权的 webSearch 网络搜索（参见 docs/agent-tools.md），没有实现 Shell、脚本执行、依赖安装或 MCP。安装 Skill 不会自动授予任何工具权限。

allowed-tools、context、agent、model、hooks 等字段原样保留；未支持的执行扩展在管理界面告警，不假装提供 Claude Code 的 fork、hooks 或动态命令能力。动态变量替换和命令插值也尚未实现。需要执行脚本的技能可能仅部分可用，必须先完成后续工具、权限、运行目录和依赖管理模块。

## 安装与文件安全

- ZIP 最大 32 MiB，展开文件总计最大 128 MiB，单文件最大 20 MiB，每包最多 2048 个文件 / 条目；SKILL.md 最大 256 KiB。
- 发现元数据总计最多 32000 字符，避免一次启用过多技能占满上下文。
- 拒绝路径穿越、绝对路径、Windows 设备名、NTFS 数据流、符号链接、重复或大小写冲突路径。
- 安装在仓库内暂存目录完成，替换采用备份重命名和失败回退；包外管理状态原子写入。
- GitHub 只下载固定 codeload 主机，不跟随任意站点重定向，不接收访问令牌。超大仓库可改为只打包技能目录上传。
- 文件下载强制 attachment 和 nosniff，不在管理域渲染第三方 HTML。读取工具仅支持有界 UTF-8 文本，二进制资源需后续专用工具。
- 所有管理接口要求管理员会话，写操作要求 CSRF。

## 代码与验收

- src/domain/agent_skill.rs：SKILL.md 元数据解析和名称验证。
- src/infrastructure/agent_skills.rs：完整文件包、安装、替换、浏览、导出与包外状态。
- src/agent/skills.rs：技能发现、请求级范围与按需读取工具。
- src/infrastructure/agent.rs：Rig 多轮工具接入及流式正文隔离。
- src/web/admin/agent/skills.rs：认证包管理 API。
- frontend/src/features/admin/agent/SkillManager.tsx：安装与文件管理界面。

验证：cargo test --lib，cargo clippy --workspace --all-targets -- -D warnings，frontend 中 npm run check / npm run build，以及 PowerShell 7 下 scripts/smoke-agent-skills.ps1（仅临时文件包，不调用模型，结束后清理）。
