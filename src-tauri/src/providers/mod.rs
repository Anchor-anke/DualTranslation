use crate::{
    error::{AppError, AppResult},
    types::ProviderProfile,
};
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
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
    response_format: Option<ResponseFormat>,
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
struct ResponseFormat {
    #[serde(rename = "type")]
    kind: &'static str,
}

#[derive(Debug, Serialize)]
struct ThinkingConfig {
    #[serde(rename = "type")]
    kind: &'static str,
}

#[derive(Debug, Deserialize)]
struct ChatResponse {
    choices: Vec<ChatChoice>,
}

#[derive(Debug, Deserialize)]
struct ChatChoice {
    #[serde(default)]
    finish_reason: Option<String>,
    message: ChatResponseMessage,
}

#[derive(Debug, Deserialize)]
struct ChatResponseMessage {
    content: Option<String>,
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

        let parsed = response.json::<ChatResponse>().await.map_err(|_| {
            AppError::new(
                "SCHEMA_INVALID",
                "模型服务返回了无法识别的响应。",
                "请确认该地址兼容 OpenAI Chat Completions 协议。",
            )
        })?;
        let Some(choice) = parsed.choices.into_iter().next() else {
            return Err(AppError::new(
                "SCHEMA_INVALID",
                "模型服务没有返回候选结果。",
                "请重试；如果问题持续，请切换模型服务。",
            ));
        };

        if let Some(content) = choice
            .message
            .content
            .filter(|content| !content.trim().is_empty())
        {
            return Ok(content);
        }

        let (retryable, error) = map_empty_content(choice.finish_reason.as_deref());
        if retryable && attempt < MAX_TRANSIENT_ATTEMPTS {
            retry_backoff(attempt).await;
            continue;
        }
        return Err(error);
    }
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
            false,
            AppError::new(
                "SCHEMA_INVALID",
                "模型服务返回了空内容。",
                "请重试；如果问题持续，请切换模型服务。",
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
        response_format: is_official_deepseek.then_some(ResponseFormat {
            kind: "json_object",
        }),
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
    let url = normalize_chat_completions_url(&profile.base_url)?;
    let request = build_connection_request(profile, &url);
    let response = client
        .post(url)
        .bearer_auth(api_key)
        .timeout(Duration::from_millis(profile.timeout_ms))
        .json(&request)
        .send()
        .await
        .map_err(map_reqwest_error)?;

    if response.status().is_success() {
        Ok(())
    } else {
        Err(map_status(response.status()))
    }
}

fn build_connection_request(profile: &ProviderProfile, url: &Url) -> Value {
    let mut request = json!({
        "model": profile.model,
        "messages": [{"role": "user", "content": "Reply with OK."}],
        "stream": false,
        "max_tokens": 4
    });
    if is_official_deepseek(profile, url) {
        request["thinking"] = json!({ "type": "disabled" });
    }
    request
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
    fn deepseek_fast_mode_disables_thinking_and_requests_json() {
        let request = serialized_request(
            "https://api.deepseek.com/v1",
            "deepseek-v4-flash",
            InferenceMode::Fast,
        );

        assert_eq!(request["thinking"]["type"], "disabled");
        assert_eq!(request["response_format"]["type"], "json_object");
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
    fn deepseek_connection_test_disables_thinking() {
        let provider = ProviderProfile {
            model: "deepseek-v4-flash".into(),
            ..profile("https://api.deepseek.com/v1".into(), 1_000)
        };
        let url = normalize_chat_completions_url(&provider.base_url).unwrap();
        let request = build_connection_request(&provider, &url);

        assert_eq!(request["thinking"]["type"], "disabled");
        assert_eq!(request["max_tokens"], 4);
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

    #[tokio::test]
    async fn maps_common_provider_statuses_to_stable_codes() {
        for (status, expected) in [
            (401, "AUTH_FAILED"),
            (403, "AUTH_FAILED"),
            (404, "NOT_FOUND"),
            (429, "RATE_LIMITED"),
            (500, "NETWORK_FAILED"),
        ] {
            let base_url = mock_chat_server(status, r#"{"error":"mock"}"#, Duration::ZERO).await;
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
            assert_eq!(error.code, expected);
        }
    }

    #[tokio::test]
    async fn rejects_malformed_chat_envelope() {
        let base_url = mock_chat_server(200, r#"{"unexpected":true}"#, Duration::ZERO).await;
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
    async fn maps_abrupt_connection_close_to_network_failed() {
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
        assert_eq!(error.code, "NETWORK_FAILED");
    }
}
