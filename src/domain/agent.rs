//! Agent 核心的任务语义；API 和具体模型协议均依赖此边界。

/// 可绑定模型的任务，后续工具、Skill 和 MCP 能围绕任务扩展。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AgentTask {
    Writing,
    Summary,
    Image,
    Chat,
    /// 无工具权限的评论自动审核任务。
    CommentReview,
}

impl AgentTask {
    /// 数据库存储和 API 使用的稳定任务名。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Writing => "writing",
            Self::Summary => "summary",
            Self::Image => "image",
            Self::Chat => "chat",
            Self::CommentReview => "comment_review",
        }
    }

    /// 将 API 中的稳定任务名解析为领域任务。
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "writing" => Some(Self::Writing),
            "summary" => Some(Self::Summary),
            "image" => Some(Self::Image),
            "chat" => Some(Self::Chat),
            "comment_review" => Some(Self::CommentReview),
            _ => None,
        }
    }

    /// 绑定时验证模型能力，不允许图像模型承担写作任务。
    pub fn capability(self) -> &'static str {
        match self {
            Self::Image => "image",
            _ => "text",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_capabilities_are_explicit() {
        assert_eq!(AgentTask::parse("image").unwrap().capability(), "image");
        assert_eq!(AgentTask::parse("writing").unwrap().capability(), "text");
        assert!(AgentTask::parse("arbitrary").is_none());
    }
}
