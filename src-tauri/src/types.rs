use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ConversionMode {
    Write,
    Explain,
}

impl ConversionMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Write => "write",
            Self::Explain => "explain",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TargetAgent {
    Generic,
    Cursor,
    Codex,
}

impl TargetAgent {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Generic => "generic",
            Self::Cursor => "cursor",
            Self::Codex => "codex",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum LanguagePreference {
    Auto,
    Zh,
    En,
    Bilingual,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum OutputLanguage {
    Zh,
    En,
    Bilingual,
}

impl OutputLanguage {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Zh => "zh",
            Self::En => "en",
            Self::Bilingual => "bilingual",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum GenerationMode {
    Quick,
    Negotiated,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClarificationAnswer {
    pub question_id: String,
    pub answer: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversionRequest {
    pub schema_version: u8,
    pub mode: ConversionMode,
    pub input: String,
    pub target_agent: TargetAgent,
    pub language_preference: LanguagePreference,
    pub generation_mode: GenerationMode,
    pub clarification_answers: Vec<ClarificationAnswer>,
    pub provider_profile_id: String,
    pub save_to_history: bool,
    pub allow_sensitive_history: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LanguageDecision {
    pub language: OutputLanguage,
    pub reason: String,
    pub user_override: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TaskType {
    Feature,
    Bugfix,
    Ui,
    Refactor,
    Review,
    Research,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskScope {
    pub in_scope: Vec<String>,
    pub out_of_scope: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentBehavior {
    pub inspect_before_action: bool,
    pub plan_before_action: bool,
    pub confirmation_required_for: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CanonicalTaskSpec {
    pub title: String,
    #[serde(rename = "type")]
    pub task_type: TaskType,
    pub goal: String,
    pub motivation: Option<String>,
    pub context: Vec<String>,
    pub scope: TaskScope,
    pub constraints: Vec<String>,
    pub assumptions: Vec<String>,
    pub unknowns: Vec<String>,
    pub agent_behavior: AgentBehavior,
    pub acceptance_criteria: Vec<String>,
    pub verification: Vec<String>,
    pub deliverables: Vec<String>,
    pub output_language: OutputLanguage,
    pub target_agent: TargetAgent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreservedTechnicalDetails {
    pub commands: Vec<String>,
    pub file_paths: Vec<String>,
    pub errors: Vec<String>,
    pub code_snippets: Vec<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExplanationStatus {
    Completed,
    Partial,
    Failed,
    Unclear,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExplanationSpec {
    pub summary: String,
    pub status: ExplanationStatus,
    pub actions_taken: Vec<String>,
    pub verification_results: Vec<String>,
    pub user_decisions_needed: Vec<String>,
    pub risks_and_warnings: Vec<String>,
    pub suggested_next_steps: Vec<String>,
    pub preserved_technical_details: PreservedTechnicalDetails,
    pub output_language: OutputLanguage,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClarificationQuestion {
    pub id: String,
    pub question: String,
    pub reason: String,
    pub options: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum WriteModelOutput {
    ClarificationRequired {
        questions: Vec<ClarificationQuestion>,
    },
    WriteCompleted {
        #[serde(rename = "taskSpec")]
        task_spec: Box<CanonicalTaskSpec>,
        #[serde(rename = "languageDecision")]
        language_decision: LanguageDecision,
    },
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ExplainModelOutput {
    ExplainCompleted {
        explanation: ExplanationSpec,
        #[serde(rename = "languageDecision")]
        language_decision: LanguageDecision,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversionResponse {
    pub schema_version: u8,
    pub kind: String,
    pub request_id: String,
    pub data: Value,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdjustmentResult {
    pub response: ConversionResponse,
    pub version_no: Option<u32>,
    pub changed_fields: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderProfile {
    pub id: String,
    pub name: String,
    pub base_url: String,
    pub model: String,
    pub timeout_ms: u64,
    pub has_credential: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderProfileDraft {
    pub id: Option<String>,
    pub name: String,
    pub base_url: String,
    pub model: String,
    pub timeout_ms: u64,
    pub api_key: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderTestResult {
    pub model: String,
    pub latency_ms: u128,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub shortcut: String,
    pub default_agent: TargetAgent,
    pub write_language: LanguagePreference,
    pub explain_language: LanguagePreference,
    pub history_limit: u32,
    pub always_on_top: bool,
    pub active_provider_profile_id: Option<String>,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            shortcut: "CommandOrControl+Shift+Space".into(),
            default_agent: TargetAgent::Codex,
            write_language: LanguagePreference::Auto,
            explain_language: LanguagePreference::Zh,
            history_limit: 20,
            always_on_top: true,
            active_provider_profile_id: None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistorySummary {
    pub id: String,
    pub mode: ConversionMode,
    pub preview: String,
    pub target_agent: TargetAgent,
    pub output_language: OutputLanguage,
    pub created_at: String,
    pub version_count: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryVersion {
    pub version_no: u32,
    pub rendered_text: String,
    pub structured_data: Value,
    pub adjustment_text: Option<String>,
    pub changed_fields: Vec<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryRecord {
    #[serde(flatten)]
    pub summary: HistorySummary,
    pub original_input: String,
    pub sensitive: bool,
    pub versions: Vec<HistoryVersion>,
}
