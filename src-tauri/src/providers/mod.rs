use crate::{
    error::{AppError, AppResult},
    types::ProviderProfile,
};
use reqwest::{Client, StatusCode};
use serde::Serialize;
use serde_json::Value;
use std::time::Duration;
use url::Url;

const DEEPSEEK_MAX_OUTPUT_TOKENS: u32 = 8_192;
const MAX_TRANSIENT_ATTEMPTS: usize = 3;
const RETRY_DELAYS_MS: [u64; MAX_TRANSIENT_ATTEMPTS - 1] = [250, 750];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InferenceMode {
    Fast,
    Deliberate,
}

#[derive(Debug, Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    messages: Vec<ChatMessage<'a>>,
    stream: bool,
    temperature: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    thinking: Option<ThinkingConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reasoning_effort: Option<&'static str>,
}

#[derive(Debug, Serialize)]
struct ChatMessage<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Debug, Serialize)]
struct ThinkingConfig {
    #[serde(rename = "type")]
    kind: &'static str,
}

pub fn normalize_chat_completions_url(base_url: &str) -> AppResult<Url> {
    let mut url = Url::parse(base_url).map_err(|_| {
        AppError::new(
            "NOT_FOUND",
            "Base URL 不是有效地址。",
            "请填写完整的 HTTPS 地址，例如 https://api.example.com/v1。",
        )
    })?;

    let is_local = matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "::1"));
    if url.scheme() != "https" && !(url.scheme() == "http" && is_local) {
        return Err(AppError::new(
            "NOT_FOUND",
            "远程模型服务必须使用 HTTPS。",
            "请改用 HTTPS；只有 localhost、127.0.0.1 和 ::1 可以使用 HTTP。",
        ));
    }

    url.set_query(None);
    url.set_fragment(None);
    let path = url.path().trim_end_matches('/');
    let normalized_path = if path.ends_with("/chat/completions") {
        path.to_owned()
    } else if path.is_empty() {
        "/v1/chat/completions".to_owned()
    } else {
        format!("{path}/chat/completions")
    };
    url.set_path(&normalized_path);
    Ok(url)
}

pub async fn send_chat(
    client: &Client,
    profile: &ProviderProfile,
    api_key: &str,
    system: &str,
    user: &str,
    inference_mode: InferenceMode,
) -> AppResult<String> {
    let url = normalize_chat_completions_url(&profile.base_url)?;
    let request = build_chat_request(profile, &url, system, user, inference_mode);
    let mut attempt = 0;

    loop {
        attempt += 1;
        let response = client
            .post(url.clone())
            .bearer_auth(api_key)
            .timeout(Duration::from_millis(profile.timeout_ms))
            .json(&request)
            .send()
            .await
            .map_err(map_reqwest_error)?;
        let status = response.status();

        if !status.is_success() {
            if status.is_server_error() && attempt < MAX_TRANSIENT_ATTEMPTS {
                retry_backoff(attempt).await;
                continue;
            }
            return Err(map_status(status));
        }

        let body = response.text().await.map_err(map_reqwest_error)?;
        match parse_chat_response(&body) {
            Ok(ChatResponseOutcome::Content(content)) => return Ok(content),
            Ok(ChatResponseOutcome::Empty(finish_reason)) => {
                let (retryable, error) = map_empty_content(finish_reason.as_deref());
                if retryable && attempt < MAX_TRANSIENT_ATTEMPTS {
                    retry_backoff(attempt).await;
                    continue;
                }
                return Err(error);
            }
            Err(error) => {
                if is_retryable_response_error(&error) && attempt < MAX_TRANSIENT_ATTEMPTS {
                    retry_backoff(attempt).await;
                    continue;
                }
                return Err(error);
            }
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
enum ChatResponseOutcome {
    Content(String),
    Empty(Option<String>),
}

fn parse_chat_response(body: &str) -> AppResult<ChatResponseOutcome> {
    let trimmed = body.trim_start_matches('\u{feff}').trim();
    if trimmed.is_empty() {
        return Err(AppError::provider_schema_invalid());
    }
    if trimmed.starts_with("data:") || trimmed.contains("\ndata:") {
        return parse_sse_chat_response(trimmed);
    }
    let payload =
        serde_json::from_str::<Value>(trimmed).map_err(|_| AppError::provider_schema_invalid())?;
    parse_chat_payload(&payload)
}

fn parse_chat_payload(payload: &Value) -> AppResult<ChatResponseOutcome> {
    if let Some(error) = payload.get("error") {
        return Err(map_provider_payload_error(error));
    }
    if payload.get("choices").is_none()
        && (payload.get("code").is_some() || payload.get("status").is_some())
    {
        return Err(map_provider_payload_error(payload));
    }
    let choices = payload
        .get("choices")
        .and_then(Value::as_array)
        .filter(|choices| !choices.is_empty())
        .ok_or_else(AppError::provider_schema_invalid)?;
    let mut finish_reason = None;
    let mut recognized_choice = false;

    for choice in choices {
        if finish_reason.is_none() {
            finish_reason = choice
                .get("finish_reason")
                .and_then(Value::as_str)
                .map(str::to_owned);
        }
        if let Some(text) = choice.get("text").and_then(Value::as_str)
            && !text.trim().is_empty()
        {
            return Ok(ChatResponseOutcome::Content(text.to_owned()));
        }
        if let Some(message) = choice.get("message").or_else(|| choice.get("delta")) {
            recognized_choice = true;
            if let Some(content) = extract_message_text(message) {
                return Ok(ChatResponseOutcome::Content(content));
            }
        }
    }

    if recognized_choice || finish_reason.is_some() {
        return Ok(ChatResponseOutcome::Empty(finish_reason));
    }
    Err(AppError::provider_schema_invalid())
}

fn parse_sse_chat_response(body: &str) -> AppResult<ChatResponseOutcome> {
    let mut content = String::new();
    let mut finish_reason = None;
    let mut saw_choice = false;

    for line in body.lines() {
        let Some(data) = line.trim().strip_prefix("data:").map(str::trim) else {
            continue;
        };
        if data.is_empty() || data == "[DONE]" {
            continue;
        }
        let payload =
            serde_json::from_str::<Value>(data).map_err(|_| AppError::provider_schema_invalid())?;
        if let Some(error) = payload.get("error") {
            return Err(map_provider_payload_error(error));
        }
        let Some(choices) = payload.get("choices").and_then(Value::as_array) else {
            continue;
        };
        for choice in choices {
            saw_choice = true;
            if let Some(reason) = choice.get("finish_reason").and_then(Value::as_str) {
                finish_reason = Some(reason.to_owned());
            }
            if let Some(message) = choice.get("delta").or_else(|| choice.get("message"))
                && let Some(part) = extract_message_text(message)
            {
                content.push_str(&part);
            }
        }
    }

    if !content.trim().is_empty() {
        return Ok(ChatResponseOutcome::Content(content));
    }
    if saw_choice {
        return Ok(ChatResponseOutcome::Empty(finish_reason));
    }
    Err(AppError::provider_schema_invalid())
}

fn extract_message_text(message: &Value) -> Option<String> {
    let content = message.get("content").unwrap_or(message);
    let text = match content {
        Value::String(text) => text.clone(),
        Value::Array(parts) => parts
            .iter()
            .filter_map(|part| match part {
                Value::String(text) => Some(text.as_str()),
                Value::Object(object) => object
                    .get("text")
                    .or_else(|| object.get("content"))
                    .and_then(Value::as_str),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join(""),
        Value::Object(object) => object
            .get("text")
            .or_else(|| object.get("content"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned(),
        _ => String::new(),
    };
    (!text.trim().is_empty()).then_some(text)
}

fn map_provider_payload_error(error: &Value) -> AppError {
    let marker = error
        .get("code")
        .or_else(|| error.get("type"))
        .or_else(|| error.get("status"))
        .map(value_marker)
        .unwrap_or_default();
    let message = error
        .get("message")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let marker = format!("{marker} {message}").to_ascii_lowercase();
    if marker.contains("model") && (marker.contains("not_found") || marker.contains("invalid")) {
        return AppError::new(
            "NOT_FOUND",
            "模型服务不支持当前模型标识。",
            "请在设置中核对模型名，然后重新测试连接。",
        );
    }
    if marker.contains("auth") || marker.contains("key") || marker.contains("permission") {
        return AppError::new(
            "AUTH_FAILED",
            "API Key 无效或没有模型权限。",
            "请检查密钥并重新保存。",
        );
    }
    if marker.contains("rate") || marker.contains("quota") {
        return AppError::new(
            "RATE_LIMITED",
            "模型服务当前限流或配额不足。",
            "请稍后重试，或检查供应商配额。",
        );
    }
    AppError::new(
        "NETWORK_FAILED",
        "模型服务返回了错误结果。",
        "请在设置中测试连接，并检查模型名和服务商配额。",
    )
}

fn value_marker(value: &Value) -> String {
    value
        .as_str()
        .map(str::to_owned)
        .unwrap_or_else(|| value.to_string())
}

fn is_retryable_response_error(error: &AppError) -> bool {
    matches!(error.code, "SCHEMA_INVALID" | "NETWORK_FAILED")
}

async fn retry_backoff(attempt: usize) {
    tokio::time::sleep(Duration::from_millis(RETRY_DELAYS_MS[attempt - 1])).await;
}

fn map_empty_content(finish_reason: Option<&str>) -> (bool, AppError) {
    match finish_reason {
        Some("insufficient_system_resource") => (
            true,
            AppError::new(
                "NETWORK_FAILED",
                "模型服务当前推理资源不足，自动重试后仍未返回内容。",
                "请稍后重试，或临时切换到其他模型。",
            ),
        ),
        Some("content_filter") => (
            false,
            AppError::new(
                "SCHEMA_INVALID",
                "模型服务因内容安全策略未返回结果。",
                "请调整输入内容后重试。",
            ),
        ),
        Some("length") => (
            false,
            AppError::new(
                "SCHEMA_INVALID",
                "模型输出达到长度限制，未返回完整内容。",
                "请缩短输入内容或拆分处理后重试。",
            ),
        ),
        Some("tool_calls") => (
            false,
            AppError::new(
                "SCHEMA_INVALID",
                "模型服务返回了工具调用，而不是文本结果。",
                "请切换到支持普通文本输出的模型配置。",
            ),
        ),
        _ => (
            true,
            AppError::new(
                "SCHEMA_INVALID",
                "模型服务自动重试后仍返回空内容。",
                "请稍后重试；如果问题持续，请切换模型服务。",
            ),
        ),
    }
}

fn build_chat_request<'a>(
    profile: &'a ProviderProfile,
    url: &Url,
    system: &'a str,
    user: &'a str,
    inference_mode: InferenceMode,
) -> ChatRequest<'a> {
    let is_official_deepseek = is_official_deepseek(profile, url);
    let (thinking, reasoning_effort) = if is_official_deepseek {
        match inference_mode {
            InferenceMode::Fast => (Some(ThinkingConfig { kind: "disabled" }), None),
            InferenceMode::Deliberate => (Some(ThinkingConfig { kind: "enabled" }), Some("low")),
        }
    } else {
        (None, None)
    };

    ChatRequest {
        model: &profile.model,
        messages: vec![
            ChatMessage {
                role: "system",
                content: system,
            },
            ChatMessage {
                role: "user",
                content: user,
            },
        ],
        stream: false,
        temperature: 0.1,
        max_tokens: is_official_deepseek.then_some(DEEPSEEK_MAX_OUTPUT_TOKENS),
        // The app validates and repairs JSON itself. DeepSeek documents that forcing its JSON
        // Output mode can occasionally yield empty content, so keep that unstable mode disabled.
        thinking,
        reasoning_effort,
    }
}

fn is_official_deepseek(profile: &ProviderProfile, url: &Url) -> bool {
    url.host_str().is_some_and(|host| {
        host.eq_ignore_ascii_case("api.deepseek.com") && profile.model.starts_with("deepseek-")
    })
}

pub async fn test_connection(
    client: &Client,
    profile: &ProviderProfile,
    api_key: &str,
) -> AppResult<()> {
    let output = send_chat(
        client,
        profile,
        api_key,
        "Return exactly one JSON object and no other text.",
        r#"Return {"ok":true}."#,
        InferenceMode::Fast,
    )
    .await?;
    let value = extract_json(&output)?;
    if value.get("ok").and_then(Value::as_bool) == Some(true) {
        return Ok(());
    }
    Err(AppError::new(
        "SCHEMA_INVALID",
        "模型可以连接，但无法稳定返回结构化 JSON。",
        "请换用支持 JSON 输出的模型，或检查模型服务的兼容设置。",
    ))
}

pub fn extract_json(content: &str) -> AppResult<Value> {
    let trimmed = content.trim();
    if let Ok(value) = serde_json::from_str(trimmed) {
        return Ok(value);
    }

    let without_fence = trimmed
        .strip_prefix("```json")
        .or_else(|| trimmed.strip_prefix("```JSON"))
        .or_else(|| trimmed.strip_prefix("```"))
        .and_then(|value| value.strip_suffix("```"))
        .map(str::trim);
    if let Some(value) = without_fence.and_then(|value| serde_json::from_str(value).ok()) {
        return Ok(value);
    }

    if let (Some(start), Some(end)) = (trimmed.find('{'), trimmed.rfind('}'))
        && start < end
        && let Ok(value) = serde_json::from_str(&trimmed[start..=end])
    {
        return Ok(value);
    }

    Err(AppError::schema_invalid())
}

fn map_reqwest_error(error: reqwest::Error) -> AppError {
    if error.is_timeout() {
        return AppError::new(
            "TIMEOUT",
            "模型响应超时。",
            "请重试、缩短输入，或在模型配置中调高超时时间。",
        );
    }
    AppError::new(
        "NETWORK_FAILED",
        "无法连接模型服务。",
        "请检查网络、代理和 Base URL。",
    )
}

fn map_status(status: StatusCode) -> AppError {
    match status.as_u16() {
        401 | 403 => AppError::new(
            "AUTH_FAILED",
            "API Key 无效或没有模型权限。",
            "请检查密钥并重新保存。",
        ),
        404 => AppError::new(
            "NOT_FOUND",
            "模型地址或模型名不存在。",
            "请检查 Base URL、路径和模型标识。",
        ),
        429 => AppError::new(
            "RATE_LIMITED",
            "模型服务当前限流。",
            "请稍后重试，或检查供应商配额。",
        ),
        500..=599 => AppError::new(
            "NETWORK_FAILED",
            "模型服务暂时不可用。",
            "请稍后重试或切换服务。",
        ),
        _ => AppError::new(
            "NETWORK_FAILED",
            format!("模型服务拒绝了请求（HTTP {}）。", status.as_u16()),
            "请检查模型服务配置。",
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        conversion,
        types::{
            ConversionMode, ConversionRequest, GenerationMode, LanguagePreference, TargetAgent,
        },
    };
    use serde_json::json;
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
        time::sleep,
    };

    async fn mock_chat_server(status: u16, body: &str, delay: Duration) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let body = body.to_owned();
        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut request = vec![0; 16 * 1024];
            let _ = stream.read(&mut request).await.unwrap();
            sleep(delay).await;
            let reason = match status {
                200 => "OK",
                401 => "Unauthorized",
                404 => "Not Found",
                429 => "Too Many Requests",
                _ => "Error",
            };
            let response = format!(
                "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = stream.write_all(response.as_bytes()).await;
        });
        format!("http://{address}/v1")
    }

    async fn mock_chat_sequence(contents: Vec<String>) -> String {
        let responses = contents
            .into_iter()
            .map(|content| {
                (
                    200,
                    json!({
                        "choices": [{
                            "finish_reason": "stop",
                            "message": { "content": content }
                        }]
                    })
                    .to_string(),
                )
            })
            .collect();
        mock_http_sequence(responses).await
    }

    async fn mock_http_sequence(responses: Vec<(u16, String)>) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move {
            for (status, body) in responses {
                let (mut stream, _) = listener.accept().await.unwrap();
                let mut request = vec![0; 16 * 1024];
                let _ = stream.read(&mut request).await.unwrap();
                let reason = if status == 200 { "OK" } else { "Error" };
                let response = format!(
                    "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                stream.write_all(response.as_bytes()).await.unwrap();
            }
        });
        format!("http://{address}/v1")
    }

    fn profile(base_url: String, timeout_ms: u64) -> ProviderProfile {
        ProviderProfile {
            id: "mock".into(),
            name: "Mock".into(),
            base_url,
            model: "mock-model".into(),
            timeout_ms,
            has_credential: true,
        }
    }

    fn serialized_request(base_url: &str, model: &str, inference_mode: InferenceMode) -> Value {
        let mut provider = profile(base_url.into(), 1_000);
        provider.model = model.into();
        let url = normalize_chat_completions_url(base_url).unwrap();
        serde_json::to_value(build_chat_request(
            &provider,
            &url,
            "Return json only.",
            "Create json output.",
            inference_mode,
        ))
        .unwrap()
    }

    fn write_request() -> ConversionRequest {
        ConversionRequest {
            schema_version: 1,
            mode: ConversionMode::Write,
            input: "添加邮箱密码登录".into(),
            target_agent: TargetAgent::Codex,
            language_preference: LanguagePreference::Zh,
            generation_mode: GenerationMode::Quick,
            clarification_answers: Vec::new(),
            project_contexts: Vec::new(),
            provider_profile_id: "mock".into(),
            save_to_history: false,
            allow_sensitive_history: false,
        }
    }

    #[test]
    fn normalizes_common_base_urls() {
        assert_eq!(
            normalize_chat_completions_url("https://api.example.com")
                .unwrap()
                .as_str(),
            "https://api.example.com/v1/chat/completions"
        );
        assert_eq!(
            normalize_chat_completions_url("https://api.example.com/v1/")
                .unwrap()
                .as_str(),
            "https://api.example.com/v1/chat/completions"
        );
        assert_eq!(
            normalize_chat_completions_url(
                "https://api.example.com/v1/chat/completions?debug=true"
            )
            .unwrap()
            .as_str(),
            "https://api.example.com/v1/chat/completions"
        );
    }

    #[test]
    fn rejects_remote_plain_http_but_allows_localhost() {
        assert!(normalize_chat_completions_url("http://api.example.com/v1").is_err());
        assert!(normalize_chat_completions_url("http://127.0.0.1:11434/v1").is_ok());
    }

    #[test]
    fn extracts_json_from_markdown_fence() {
        let value = extract_json("```json\n{\"kind\":\"ok\"}\n```").unwrap();
        assert_eq!(value["kind"], "ok");
    }

    #[test]
    fn deepseek_fast_mode_avoids_unstable_json_mode() {
        let request = serialized_request(
            "https://api.deepseek.com/v1",
            "deepseek-v4-flash",
            InferenceMode::Fast,
        );

        assert_eq!(request["thinking"]["type"], "disabled");
        assert!(request.get("response_format").is_none());
        assert_eq!(request["max_tokens"], DEEPSEEK_MAX_OUTPUT_TOKENS);
        assert!(request.get("reasoning_effort").is_none());
    }

    #[test]
    fn deepseek_deliberate_mode_uses_low_reasoning_effort() {
        let request = serialized_request(
            "https://api.deepseek.com/v1",
            "deepseek-v4-flash",
            InferenceMode::Deliberate,
        );

        assert_eq!(request["thinking"]["type"], "enabled");
        assert_eq!(request["reasoning_effort"], "low");
    }

    #[test]
    fn generic_openai_compatible_request_omits_deepseek_extensions() {
        let request = serialized_request(
            "https://api.example.com/v1",
            "generic-model",
            InferenceMode::Fast,
        );

        for field in [
            "thinking",
            "reasoning_effort",
            "response_format",
            "max_tokens",
        ] {
            assert!(request.get(field).is_none(), "unexpected field: {field}");
        }
    }

    #[tokio::test]
    async fn accepts_openai_compatible_mock_response() {
        let base_url = mock_chat_server(
            200,
            r#"{"choices":[{"message":{"content":"{\"kind\":\"ok\"}"}}]}"#,
            Duration::ZERO,
        )
        .await;
        let output = send_chat(
            &Client::new(),
            &profile(base_url, 1_000),
            "test-key",
            "system",
            "user",
            InferenceMode::Fast,
        )
        .await
        .unwrap();
        assert_eq!(output, r#"{"kind":"ok"}"#);
    }

    #[tokio::test]
    async fn accepts_text_parts_in_chat_content() {
        let base_url = mock_chat_server(
            200,
            r#"{"choices":[{"message":{"content":[{"type":"text","text":"{\"kind\":"},{"type":"text","text":"\"ok\"}"}]}}]}"#,
            Duration::ZERO,
        )
        .await;
        let output = send_chat(
            &Client::new(),
            &profile(base_url, 1_000),
            "test-key",
            "system",
            "user",
            InferenceMode::Fast,
        )
        .await
        .unwrap();
        assert_eq!(output, r#"{"kind":"ok"}"#);
    }

    #[test]
    fn accepts_sse_when_a_compatible_gateway_streams_unexpectedly() {
        let output = parse_chat_response(
            "data: {\"choices\":[{\"delta\":{\"content\":\"{\\\"kind\\\":\"}}]}\n\n\
             data: {\"choices\":[{\"delta\":{\"content\":\"\\\"ok\\\"}\"},\"finish_reason\":\"stop\"}]}\n\n\
             data: [DONE]",
        )
        .unwrap();

        assert_eq!(
            output,
            ChatResponseOutcome::Content(r#"{"kind":"ok"}"#.into())
        );
    }

    #[tokio::test]
    async fn retries_empty_and_incomplete_success_responses() {
        let success = json!({
            "choices": [{
                "finish_reason": "stop",
                "message": { "content": "{\"kind\":\"ok\"}" }
            }]
        })
        .to_string();
        let base_url = mock_http_sequence(vec![
            (200, String::new()),
            (200, r#"{"choices":[]}"#.into()),
            (200, success),
        ])
        .await;

        let output = send_chat(
            &Client::new(),
            &profile(base_url, 2_000),
            "test-key",
            "system",
            "user",
            InferenceMode::Fast,
        )
        .await
        .unwrap();

        assert_eq!(output, r#"{"kind":"ok"}"#);
    }

    #[tokio::test]
    async fn retries_empty_content_with_normal_stop_reason() {
        let empty = json!({
            "choices": [{
                "finish_reason": "stop",
                "message": { "content": null }
            }]
        })
        .to_string();
        let success = json!({
            "choices": [{
                "finish_reason": "stop",
                "message": { "content": "{\"kind\":\"ok\"}" }
            }]
        })
        .to_string();
        let base_url = mock_http_sequence(vec![(200, empty), (200, success)]).await;

        let output = send_chat(
            &Client::new(),
            &profile(base_url, 2_000),
            "test-key",
            "system",
            "user",
            InferenceMode::Fast,
        )
        .await
        .unwrap();

        assert_eq!(output, r#"{"kind":"ok"}"#);
    }

    #[tokio::test]
    async fn connection_test_validates_structured_output() {
        let valid = json!({
            "choices": [{
                "finish_reason": "stop",
                "message": { "content": "{\"ok\":true}" }
            }]
        })
        .to_string();
        let base_url = mock_http_sequence(vec![(200, valid)]).await;

        test_connection(&Client::new(), &profile(base_url, 1_000), "test-key")
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn connection_test_rejects_success_status_with_invalid_envelope() {
        let invalid = r#"{"unexpected":true}"#.to_owned();
        let base_url = mock_http_sequence(vec![
            (200, invalid.clone()),
            (200, invalid.clone()),
            (200, invalid),
        ])
        .await;
        let error = test_connection(&Client::new(), &profile(base_url, 1_000), "test-key")
            .await
            .unwrap_err();

        assert_eq!(error.code, "SCHEMA_INVALID");
        assert!(error.message.contains("连续返回"));
    }

    #[tokio::test]
    async fn maps_embedded_model_error_from_success_response() {
        let base_url = mock_chat_server(
            200,
            r#"{"error":{"type":"invalid_model","code":"model_not_found"}}"#,
            Duration::ZERO,
        )
        .await;
        let error = send_chat(
            &Client::new(),
            &profile(base_url, 1_000),
            "test-key",
            "system",
            "user",
            InferenceMode::Fast,
        )
        .await
        .unwrap_err();

        assert_eq!(error.code, "NOT_FOUND");
        assert!(error.message.contains("模型标识"));
    }

    #[tokio::test]
    async fn retries_resource_shortage_then_accepts_content() {
        let unavailable = json!({
            "choices": [{
                "finish_reason": "insufficient_system_resource",
                "message": { "content": null }
            }]
        })
        .to_string();
        let success = json!({
            "choices": [{
                "finish_reason": "stop",
                "message": { "content": "{\"kind\":\"ok\"}" }
            }]
        })
        .to_string();
        let base_url = mock_http_sequence(vec![(200, unavailable), (200, success)]).await;

        let output = send_chat(
            &Client::new(),
            &profile(base_url, 1_000),
            "test-key",
            "system",
            "user",
            InferenceMode::Fast,
        )
        .await
        .unwrap();

        assert_eq!(output, r#"{"kind":"ok"}"#);
    }

    #[tokio::test]
    async fn reports_resource_shortage_after_bounded_retries() {
        let unavailable = json!({
            "choices": [{
                "finish_reason": "insufficient_system_resource",
                "message": { "content": null }
            }]
        })
        .to_string();
        let base_url = mock_http_sequence(vec![
            (200, unavailable.clone()),
            (200, unavailable.clone()),
            (200, unavailable),
        ])
        .await;

        let error = send_chat(
            &Client::new(),
            &profile(base_url, 1_000),
            "test-key",
            "system",
            "user",
            InferenceMode::Fast,
        )
        .await
        .unwrap_err();

        assert_eq!(error.code, "NETWORK_FAILED");
        assert!(error.message.contains("自动重试"));
    }

    #[tokio::test]
    async fn retries_transient_server_error_then_accepts_content() {
        let success = json!({
            "choices": [{
                "finish_reason": "stop",
                "message": { "content": "ok" }
            }]
        })
        .to_string();
        let base_url =
            mock_http_sequence(vec![(503, r#"{"error":"busy"}"#.into()), (200, success)]).await;

        let output = send_chat(
            &Client::new(),
            &profile(base_url, 1_000),
            "test-key",
            "system",
            "user",
            InferenceMode::Fast,
        )
        .await
        .unwrap();

        assert_eq!(output, "ok");
    }

    #[tokio::test]
    async fn reports_content_filter_without_retrying() {
        let base_url = mock_chat_server(
            200,
            r#"{"choices":[{"finish_reason":"content_filter","message":{"content":null}}]}"#,
            Duration::ZERO,
        )
        .await;
        let error = send_chat(
            &Client::new(),
            &profile(base_url, 1_000),
            "test-key",
            "system",
            "user",
            InferenceMode::Fast,
        )
        .await
        .unwrap_err();

        assert_eq!(error.code, "SCHEMA_INVALID");
        assert!(error.message.contains("内容安全策略"));
    }

    #[test]
    fn maps_common_provider_statuses_to_stable_codes() {
        for (status, expected) in [
            (401, "AUTH_FAILED"),
            (403, "AUTH_FAILED"),
            (404, "NOT_FOUND"),
            (429, "RATE_LIMITED"),
            (500, "NETWORK_FAILED"),
        ] {
            let error = map_status(StatusCode::from_u16(status).unwrap());
            assert_eq!(error.code, expected);
        }
    }

    #[tokio::test]
    async fn rejects_malformed_chat_envelope() {
        let invalid = r#"{"unexpected":true}"#.to_owned();
        let base_url = mock_http_sequence(vec![
            (200, invalid.clone()),
            (200, invalid.clone()),
            (200, invalid),
        ])
        .await;
        let error = send_chat(
            &Client::new(),
            &profile(base_url, 1_000),
            "test-key",
            "system",
            "user",
            InferenceMode::Fast,
        )
        .await
        .unwrap_err();
        assert_eq!(error.code, "SCHEMA_INVALID");
    }

    #[tokio::test]
    async fn maps_slow_provider_to_timeout() {
        let base_url = mock_chat_server(
            200,
            r#"{"choices":[{"message":{"content":"ok"}}]}"#,
            Duration::from_millis(100),
        )
        .await;
        let error = send_chat(
            &Client::new(),
            &profile(base_url, 10),
            "test-key",
            "system",
            "user",
            InferenceMode::Fast,
        )
        .await
        .unwrap_err();
        assert_eq!(error.code, "TIMEOUT");
    }

    #[tokio::test]
    async fn conversion_repairs_one_invalid_model_output() {
        let task_spec: Value = serde_json::from_str(include_str!(
            "../../../tests/fixtures/canonical-task-spec.valid.json"
        ))
        .unwrap();
        let repaired = json!({
            "kind": "write_completed",
            "taskSpec": task_spec,
            "languageDecision": {
                "language": "zh",
                "reason": "用户明确指定中文。",
                "userOverride": true
            }
        })
        .to_string();
        let base_url = mock_chat_sequence(vec!["not json".into(), repaired]).await;
        let request = write_request();

        let response = conversion::execute(
            &Client::new(),
            &profile(base_url, 1_000),
            "test-key",
            &request,
            "repair-test",
        )
        .await
        .unwrap();
        assert_eq!(response.kind, "write_completed");
        assert_eq!(response.request_id, "repair-test");
    }

    #[tokio::test]
    async fn conversion_stops_after_one_failed_repair() {
        let base_url = mock_chat_sequence(vec!["not json".into(), "still not json".into()]).await;
        let error = conversion::execute(
            &Client::new(),
            &profile(base_url, 1_000),
            "test-key",
            &write_request(),
            "repair-failed",
        )
        .await
        .unwrap_err();
        assert_eq!(error.code, "SCHEMA_INVALID");
    }

    #[tokio::test]
    async fn maps_abrupt_connection_close_to_stable_transport_error() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut request = vec![0; 16 * 1024];
            let _ = stream.read(&mut request).await.unwrap();
        });
        let error = send_chat(
            &Client::new(),
            &profile(format!("http://{address}/v1"), 1_000),
            "test-key",
            "system",
            "user",
            InferenceMode::Fast,
        )
        .await
        .unwrap_err();
        assert!(matches!(error.code, "NETWORK_FAILED" | "TIMEOUT"));
    }
}
