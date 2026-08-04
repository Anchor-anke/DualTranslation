use crate::{
    conversion, credentials,
    error::{AppError, AppResult},
    platform,
    privacy::{SensitiveScanResult, scan},
    project_context, providers,
    storage::{NewConversion, Storage},
    types::{
        AdjustmentResult, AppSettings, ConversionRequest, ConversionResponse, HistoryRecord,
        HistorySummary, OutputLanguage, ProjectContextPreview, ProjectRecord, ProviderProfile,
        ProviderProfileDraft, ProviderTestResult,
    },
};
use reqwest::Client;
use serde_json::Value;
use std::{collections::HashMap, sync::Mutex, time::Instant};
use tauri::{AppHandle, Manager, State};
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_dialog::DialogExt;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

pub struct AppState {
    pub storage: Storage,
    pub http: Client,
    pub cancellations: Mutex<HashMap<String, CancellationToken>>,
}

impl AppState {
    pub fn new(storage: Storage) -> AppResult<Self> {
        let http = Client::builder()
            .user_agent("DualTranslation/0.1")
            .build()
            .map_err(AppError::internal)?;
        Ok(Self {
            storage,
            http,
            cancellations: Mutex::new(HashMap::new()),
        })
    }
}

#[tauri::command]
pub fn show_main_window(app: AppHandle) -> AppResult<()> {
    platform::show_main_window(&app)
}

#[tauri::command]
pub fn hide_main_window(app: AppHandle) -> AppResult<()> {
    platform::hide_main_window(&app)
}

#[tauri::command]
pub fn register_global_shortcut(app: AppHandle, shortcut: String) -> AppResult<()> {
    if shortcut.trim().is_empty() || shortcut.len() > 120 {
        return Err(AppError::new(
            "SHORTCUT_CONFLICT",
            "快捷键格式无效。",
            "请使用类似 CommandOrControl+Shift+Space 的组合。",
        ));
    }
    platform::replace_shortcut(&app, shortcut.trim())
}

#[tauri::command]
pub fn unregister_global_shortcut(app: AppHandle) -> AppResult<()> {
    platform::unregister_shortcut(&app)
}

#[tauri::command]
pub fn read_clipboard_text(app: AppHandle) -> AppResult<String> {
    let text = app.clipboard().read_text().map_err(|_| {
        AppError::new(
            "CLIPBOARD_UNAVAILABLE",
            "当前剪贴板没有可读取的纯文本。",
            "请复制文本内容后再点击读取；图片、文件和富文本暂不支持。",
        )
    })?;
    if text.chars().count() > 100_000 {
        return Err(AppError::new(
            "INPUT_TOO_LONG",
            "剪贴板文本超过 100,000 字符。",
            "请分段处理后重试。",
        ));
    }
    Ok(text)
}

#[tauri::command]
pub async fn write_clipboard_text(
    app: AppHandle,
    state: State<'_, AppState>,
    text: String,
    metric_kind: Option<String>,
) -> AppResult<()> {
    app.clipboard().write_text(text).map_err(|_| {
        AppError::new(
            "CLIPBOARD_UNAVAILABLE",
            "无法写入剪贴板。",
            "请检查系统权限后重试。",
        )
    })?;
    let metric_kind = metric_kind.as_deref().unwrap_or("copy");
    if !["copy", "minor_edit_copy", "major_edit_copy", "history_copy"].contains(&metric_kind) {
        return Err(AppError::internal("invalid copy metric kind"));
    }
    state.storage.increment_metric(metric_kind).await?;
    Ok(())
}

#[tauri::command]
pub async fn scan_sensitive_text(
    state: State<'_, AppState>,
    text: String,
) -> AppResult<SensitiveScanResult> {
    let result = scan(&text);
    if !result.findings.is_empty() {
        state.storage.increment_metric("sensitive_hit").await?;
    }
    Ok(result)
}

#[tauri::command]
pub async fn select_project_folder(
    app: AppHandle,
    state: State<'_, AppState>,
) -> AppResult<Option<ProjectRecord>> {
    let Some(selected) = app
        .dialog()
        .file()
        .set_title("选择项目文件夹")
        .blocking_pick_folder()
    else {
        return Ok(None);
    };
    let path = selected.into_path().map_err(AppError::internal)?;
    let canonical = path.canonicalize().map_err(|_| {
        AppError::new(
            "NOT_FOUND",
            "无法打开这个项目文件夹。",
            "请检查文件夹是否存在以及当前账户是否有读取权限。",
        )
    })?;
    let scan_path = canonical.clone();
    let overview =
        tauri::async_runtime::spawn_blocking(move || project_context::inspect_overview(&scan_path))
            .await
            .map_err(AppError::internal)??;
    let path_text = canonical.to_string_lossy().into_owned();
    state
        .storage
        .upsert_project(
            &path_text,
            &overview.name,
            &overview.technologies,
            overview.file_count,
            &overview.fingerprint,
        )
        .await
        .map(Some)
}

#[tauri::command]
pub async fn list_projects(state: State<'_, AppState>) -> AppResult<Vec<ProjectRecord>> {
    state.storage.list_projects().await
}

#[tauri::command]
pub async fn set_project_pinned(
    state: State<'_, AppState>,
    project_id: String,
    pinned: bool,
) -> AppResult<()> {
    state.storage.set_project_pinned(&project_id, pinned).await
}

#[tauri::command]
pub async fn remove_project(state: State<'_, AppState>, project_id: String) -> AppResult<()> {
    state.storage.remove_project(&project_id).await
}

#[tauri::command]
pub async fn prepare_project_context(
    state: State<'_, AppState>,
    project_ids: Vec<String>,
    query: String,
) -> AppResult<Vec<ProjectContextPreview>> {
    if project_ids.is_empty()
        || project_ids.len() > 3
        || query.trim().is_empty()
        || query.chars().count() > 100_000
    {
        return Err(AppError::schema_invalid());
    }
    let unique = project_ids.iter().collect::<std::collections::HashSet<_>>();
    if unique.len() != project_ids.len() {
        return Err(AppError::schema_invalid());
    }
    let records = state.storage.get_projects(&project_ids).await?;
    let scan_records = records.clone();
    let previews = tauri::async_runtime::spawn_blocking(move || {
        scan_records
            .iter()
            .map(|record| project_context::prepare_preview(record, &query))
            .collect::<AppResult<Vec<_>>>()
    })
    .await
    .map_err(AppError::internal)??;
    for preview in &previews {
        state
            .storage
            .update_project_scan(
                &preview.project_id,
                &preview.technologies,
                preview.scanned_file_count,
                &preview.fingerprint,
            )
            .await?;
    }
    Ok(previews)
}

#[tauri::command]
pub async fn list_provider_profiles(state: State<'_, AppState>) -> AppResult<Vec<ProviderProfile>> {
    state.storage.list_provider_profiles().await
}

#[tauri::command]
pub async fn save_provider_profile(
    state: State<'_, AppState>,
    profile: ProviderProfileDraft,
) -> AppResult<ProviderProfile> {
    let name = profile.name.trim();
    let model = profile.model.trim();
    if name.is_empty() || name.chars().count() > 100 || model.is_empty() || model.len() > 200 {
        return Err(AppError::new(
            "PROVIDER_NOT_CONFIGURED",
            "模型配置名称或模型名无效。",
            "请填写 1～100 字符的名称和有效模型标识。",
        ));
    }
    if !(1_000..=300_000).contains(&profile.timeout_ms) {
        return Err(AppError::new(
            "PROVIDER_NOT_CONFIGURED",
            "超时时间必须在 1,000～300,000 毫秒之间。",
            "请调整超时时间后重试。",
        ));
    }
    let normalized = providers::normalize_chat_completions_url(profile.base_url.trim())?;
    let base_url = normalized_base_url(&normalized);
    let id = profile
        .id
        .filter(|id| !id.trim().is_empty() && id.len() <= 128)
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    credentials::save(&id, profile.api_key.trim())?;
    let saved = ProviderProfile {
        id,
        name: name.into(),
        base_url,
        model: model.into(),
        timeout_ms: profile.timeout_ms,
        has_credential: true,
    };
    state.storage.upsert_provider_profile(&saved).await?;
    Ok(saved)
}

#[tauri::command]
pub async fn test_provider_profile(
    state: State<'_, AppState>,
    profile_id: String,
) -> AppResult<ProviderTestResult> {
    let profile = state.storage.get_provider_profile(&profile_id).await?;
    let key = credentials::get(&profile.id)?;
    let started = Instant::now();
    providers::test_connection(&state.http, &profile, &key).await?;
    Ok(ProviderTestResult {
        model: profile.model,
        latency_ms: started.elapsed().as_millis(),
    })
}

#[tauri::command]
pub async fn convert(
    state: State<'_, AppState>,
    request: ConversionRequest,
    request_id: Option<String>,
) -> AppResult<ConversionResponse> {
    validate_conversion_request(&request)?;
    if !request.project_contexts.is_empty() {
        let project_ids = request
            .project_contexts
            .iter()
            .map(|context| context.project_id.clone())
            .collect::<Vec<_>>();
        state.storage.get_projects(&project_ids).await?;
    }
    let profile = state
        .storage
        .get_provider_profile(&request.provider_profile_id)
        .await?;
    let api_key = credentials::get(&profile.id)?;
    let request_id = request_id
        .filter(|id| !id.trim().is_empty() && id.len() <= 128)
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    let token = CancellationToken::new();
    state
        .cancellations
        .lock()
        .map_err(AppError::internal)?
        .insert(request_id.clone(), token.clone());

    let result = tokio::select! {
        () = token.cancelled() => Err(AppError::new(
            "REQUEST_CANCELLED",
            "已取消本次转换。",
            "你可以修改输入后重新开始。",
        )),
        result = conversion::execute(&state.http, &profile, &api_key, &request, &request_id) => result,
    };
    state
        .cancellations
        .lock()
        .map_err(AppError::internal)?
        .remove(&request_id);

    match result {
        Ok(response) => {
            state.storage.increment_metric("generate").await?;
            persist_if_allowed(&state.storage, &request, &response).await?;
            Ok(response)
        }
        Err(error) => {
            if error.code != "REQUEST_CANCELLED" {
                state.storage.increment_metric("conversion_failed").await?;
            }
            Err(error)
        }
    }
}

#[tauri::command]
pub async fn cancel_conversion(state: State<'_, AppState>, request_id: String) -> AppResult<()> {
    let token = state
        .cancellations
        .lock()
        .map_err(AppError::internal)?
        .get(&request_id)
        .cloned();
    if let Some(token) = token
        && !token.is_cancelled()
    {
        token.cancel();
        state.storage.increment_metric("cancel").await?;
    }
    Ok(())
}

#[tauri::command]
pub async fn adjust_conversion(
    state: State<'_, AppState>,
    base_response: ConversionResponse,
    instruction: String,
    provider_profile_id: String,
) -> AppResult<AdjustmentResult> {
    let instruction = instruction.trim();
    if instruction.is_empty() || instruction.chars().count() > 2_000 {
        return Err(AppError::new(
            "SCHEMA_INVALID",
            "调整指令必须为 1～2,000 个字符。",
            "请精简调整要求后重试。",
        ));
    }
    if !scan(instruction).findings.is_empty() {
        return Err(AppError::new(
            "SCHEMA_INVALID",
            "调整指令包含可能的敏感信息。",
            "请删除或手动遮盖后重试。",
        ));
    }
    let profile = state
        .storage
        .get_provider_profile(&provider_profile_id)
        .await?;
    let api_key = credentials::get(&profile.id)?;
    let adjusted =
        conversion::adjust(&state.http, &profile, &api_key, &base_response, instruction).await?;
    let changed_fields = changed_fields(&base_response.data, &adjusted.data);
    let rendered = adjusted
        .data
        .get(if adjusted.kind == "write_completed" {
            "renderedPrompt"
        } else {
            "plainText"
        })
        .and_then(Value::as_str)
        .ok_or_else(AppError::schema_invalid)?;
    let version_no = state
        .storage
        .append_conversion_version(
            &adjusted.request_id,
            &adjusted,
            rendered,
            instruction,
            &changed_fields,
        )
        .await?;
    state.storage.increment_metric("generate").await?;
    Ok(AdjustmentResult {
        response: adjusted,
        version_no,
        changed_fields,
    })
}

#[tauri::command]
pub async fn list_history(state: State<'_, AppState>) -> AppResult<Vec<HistorySummary>> {
    state.storage.list_history().await
}

#[tauri::command]
pub async fn get_history(state: State<'_, AppState>, id: String) -> AppResult<HistoryRecord> {
    state.storage.get_history(&id).await
}

#[tauri::command]
pub async fn delete_history(state: State<'_, AppState>, id: String) -> AppResult<()> {
    state.storage.delete_history(&id).await
}

#[tauri::command]
pub async fn clear_history(state: State<'_, AppState>) -> AppResult<()> {
    state.storage.clear_history().await
}

#[tauri::command]
pub async fn get_settings(state: State<'_, AppState>) -> AppResult<AppSettings> {
    state.storage.get_settings().await
}

#[tauri::command]
pub async fn update_settings(
    app: AppHandle,
    state: State<'_, AppState>,
    settings: AppSettings,
) -> AppResult<AppSettings> {
    if settings.history_limit > 500 {
        return Err(AppError::new(
            "INTERNAL_ERROR",
            "历史保留数量必须在 0～500 之间。",
            "请调整数量后重试。",
        ));
    }
    if let Some(window) = app.get_webview_window("main") {
        window
            .set_always_on_top(settings.always_on_top)
            .map_err(AppError::internal)?;
    }
    state.storage.update_settings(&settings).await
}

#[tauri::command]
pub async fn get_local_metrics(state: State<'_, AppState>) -> AppResult<Value> {
    state.storage.get_metrics().await
}

#[tauri::command]
pub async fn clear_local_metrics(state: State<'_, AppState>) -> AppResult<()> {
    state.storage.clear_metrics().await
}

fn validate_conversion_request(request: &ConversionRequest) -> AppResult<()> {
    if request.schema_version != 1 {
        return Err(AppError::schema_invalid());
    }
    let length = request.input.chars().count();
    if length == 0 {
        return Err(AppError::new(
            "SCHEMA_INVALID",
            "输入不能为空。",
            "请输入内容后重试。",
        ));
    }
    if length > 100_000 {
        return Err(AppError::new(
            "INPUT_TOO_LONG",
            "输入超过 100,000 字符。",
            "请分段处理后重试。",
        ));
    }
    if request.clarification_answers.len() > 20 {
        return Err(AppError::schema_invalid());
    }
    if request.project_contexts.len() > 3 {
        return Err(AppError::schema_invalid());
    }
    let mut project_ids = std::collections::HashSet::new();
    let mut project_context_chars = 0_usize;
    for context in &request.project_contexts {
        if context.project_id.trim().is_empty()
            || !project_ids.insert(context.project_id.as_str())
            || context.files.len() > 8
        {
            return Err(AppError::schema_invalid());
        }
        project_context_chars = project_context_chars
            .saturating_add(context.project_name.chars().count())
            .saturating_add(
                context
                    .technologies
                    .iter()
                    .map(|value| value.chars().count())
                    .sum::<usize>(),
            )
            .saturating_add(
                context
                    .facts
                    .iter()
                    .map(|value| value.chars().count())
                    .sum::<usize>(),
            );
        for file in &context.files {
            let path = std::path::Path::new(&file.path);
            if path.is_absolute()
                || path.components().any(|component| {
                    matches!(
                        component,
                        std::path::Component::ParentDir
                            | std::path::Component::RootDir
                            | std::path::Component::Prefix(_)
                    )
                })
            {
                return Err(AppError::schema_invalid());
            }
            project_context_chars = project_context_chars
                .saturating_add(file.path.chars().count())
                .saturating_add(file.reason.chars().count())
                .saturating_add(file.excerpt.chars().count());
            if !scan(&file.excerpt).findings.is_empty() {
                return Err(AppError::new(
                    "SCHEMA_INVALID",
                    "项目上下文仍包含可能的敏感信息。",
                    "请重新扫描项目，确认敏感内容已遮盖后再试。",
                ));
            }
        }
    }
    if project_context_chars > 70_000 {
        return Err(AppError::new(
            "INPUT_TOO_LONG",
            "项目上下文内容过多。",
            "请减少选中的项目或文件后重试。",
        ));
    }
    for answer in &request.clarification_answers {
        if answer.question.trim().is_empty()
            || answer.answer.trim().is_empty()
            || answer.question.chars().count() > 4_000
            || answer.answer.chars().count() > 4_000
        {
            return Err(AppError::schema_invalid());
        }
        if !scan(&answer.question).findings.is_empty() || !scan(&answer.answer).findings.is_empty()
        {
            return Err(AppError::new(
                "SCHEMA_INVALID",
                "澄清内容包含可能的敏感信息。",
                "请删除或遮盖敏感内容后重试。",
            ));
        }
    }
    if request.allow_sensitive_history && !request.save_to_history {
        return Err(AppError::schema_invalid());
    }
    Ok(())
}

async fn persist_if_allowed(
    storage: &Storage,
    request: &ConversionRequest,
    response: &ConversionResponse,
) -> AppResult<()> {
    let sensitive = !scan(&request.input).findings.is_empty();
    if !request.save_to_history || (sensitive && !request.allow_sensitive_history) {
        return Ok(());
    }
    let (rendered, language) = match response.kind.as_str() {
        "write_completed" => (
            response.data.get("renderedPrompt").and_then(Value::as_str),
            response
                .data
                .get("languageDecision")
                .and_then(|value| value.get("language"))
                .and_then(Value::as_str),
        ),
        "explain_completed" => (
            response.data.get("plainText").and_then(Value::as_str),
            response
                .data
                .get("languageDecision")
                .and_then(|value| value.get("language"))
                .and_then(Value::as_str),
        ),
        _ => return Ok(()),
    };
    let (Some(rendered), Some(language)) = (rendered, language) else {
        return Ok(());
    };
    let output_language = match language {
        "zh" => OutputLanguage::Zh,
        "en" => OutputLanguage::En,
        "bilingual" => OutputLanguage::Bilingual,
        _ => return Err(AppError::schema_invalid()),
    };
    let settings = storage.get_settings().await?;
    let project_ids = request
        .project_contexts
        .iter()
        .map(|context| context.project_id.clone())
        .collect::<Vec<_>>();
    storage
        .save_conversion(NewConversion {
            mode: &request.mode,
            original_input: &request.input,
            sensitive,
            target_agent: &request.target_agent,
            output_language: &output_language,
            response,
            rendered_text: rendered,
            history_limit: settings.history_limit,
            project_ids: &project_ids,
        })
        .await
}

fn normalized_base_url(chat_url: &url::Url) -> String {
    let mut base = chat_url.clone();
    let path = base.path().trim_end_matches("/chat/completions").to_owned();
    base.set_path(&path);
    base.to_string().trim_end_matches('/').to_owned()
}

fn changed_fields(before: &Value, after: &Value) -> Vec<String> {
    let mut fields = Vec::new();
    collect_changed_fields("data", before, after, &mut fields);
    fields.sort();
    fields.dedup();
    fields
}

fn collect_changed_fields(prefix: &str, before: &Value, after: &Value, fields: &mut Vec<String>) {
    match (before, after) {
        (Value::Object(before), Value::Object(after)) => {
            let mut keys = before.keys().chain(after.keys()).collect::<Vec<_>>();
            keys.sort();
            keys.dedup();
            for key in keys {
                let next = format!("{prefix}.{key}");
                match (before.get(key), after.get(key)) {
                    (Some(left), Some(right)) => collect_changed_fields(&next, left, right, fields),
                    _ => fields.push(next),
                }
            }
        }
        _ if before != after => fields.push(prefix.to_owned()),
        _ => {}
    }
}
