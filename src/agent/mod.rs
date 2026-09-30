//! Agent 应用层：定义任务输入与产出规则，外部模型由基础设施层调用。

pub mod context;
pub mod skills;
pub mod summary;
pub mod tools;
pub mod writing;
pub mod writing_image;

/// 文本任务的生成预算；提供商特有参数由基础设施适配器映射。
#[derive(Clone, Copy)]
pub struct TextGenerationOptions {
    pub max_tokens: u64,
    pub temperature: f64,
    pub disable_reasoning: bool,
}
