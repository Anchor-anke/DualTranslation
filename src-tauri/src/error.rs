use serde::Serialize;
use std::fmt::{Display, Formatter};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppError {
    pub code: &'static str,
    pub message: String,
    pub suggested_action: String,
}

impl AppError {
    pub fn new(
        code: &'static str,
        message: impl Into<String>,
        suggested_action: impl Into<String>,
    ) -> Self {
        Self {
            code,
            message: message.into(),
            suggested_action: suggested_action.into(),
        }
    }

    pub fn internal(_context: impl Display) -> Self {
        #[cfg(debug_assertions)]
        eprintln!("DualTranslation internal error: {_context}");

        Self::new(
            "INTERNAL_ERROR",
            "本地处理发生错误。",
            "请重试；如果问题持续，请查看本地诊断信息。",
        )
    }

    pub fn schema_invalid() -> Self {
        Self::new(
            "SCHEMA_INVALID",
            "模型返回的结构不完整，自动修复后仍未通过校验。",
            "请重试，或切换到结构化输出更稳定的模型。",
        )
    }

    pub fn provider_schema_invalid() -> Self {
        Self::new(
            "SCHEMA_INVALID",
            "模型服务连续返回了不完整或无法识别的响应。",
            "请稍后重试；如果问题持续，请在设置中测试连接并检查模型服务状态。",
        )
    }
}

impl Display for AppError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}", self.code)
    }
}

impl std::error::Error for AppError {}

pub type AppResult<T> = Result<T, AppError>;
