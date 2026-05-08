use std::{
    fmt::Write as _,
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use futures_util::StreamExt;
use reqwest::{header::HeaderMap, Client, Url};
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};

pub const CHAT_COMPLETIONS_PATH: &str = "/v2/chat/completions";
pub const DEFAULT_API_BASE_URL: &str = "https://unvcoding.copilot.qq.com";
pub const DEFAULT_API_MODEL: &str = "glm-5.0";
pub const DEFAULT_REQUEST_TIMEOUT_SECS: u64 = 600;
pub const DEFAULT_MAX_TOKENS: u32 = 48000;

const TEMPLATE_AGENT_INTENT: &str = "craft";
const TEMPLATE_CONTENT_TYPE: &str = "application/json;charset=UTF-8";
const TEMPLATE_CONNECTION: &str = "keep-alive";
const TEMPLATE_ACCEPT: &str = "*/*";
const TEMPLATE_ACCEPT_LANGUAGE: &str = "*";
const TEMPLATE_SEC_FETCH_MODE: &str = "cors";
const TEMPLATE_ACCEPT_ENCODING: &str = "br, gzip, deflate";
const TEMPLATE_IDE_TYPE: &str = "JetBrains";
const TEMPLATE_IDE_NAME: &str = "JetBrainsGoLand";
const TEMPLATE_IDE_VERSION: &str = "GO-253.29346.379";
const TEMPLATE_PRODUCT_VERSION: &str = "4.2.17133064";
const TEMPLATE_ENV_ID: &str = "production";
const TEMPLATE_USER_ID: &str = "8bf9032d-e260-425a-b156-66316d141488";
const TEMPLATE_ENTERPRISE_ID: &str = "unvcoding";
const TEMPLATE_TENANT_ID: &str = "unvcoding";
const TEMPLATE_DOMAIN: &str = "unvcoding.copilot.qq.com";
const TEMPLATE_USER_AGENT: &str =
    "JetBrainsGoLand/GO-253.29346.379 unvcoding/4.2.17133064";
const TEMPLATE_PRODUCT: &str = "Cloud-Hosted";
const TEMPLATE_REASONING_EFFORT: &str = "medium";
const TEMPLATE_REASONING_SUMMARY: &str = "auto";

static REQUEST_CONTEXT_COUNTER: AtomicU64 = AtomicU64::new(1);

pub fn chat_completions_endpoint(base_url: &str) -> String {
    let trimmed = base_url.trim_end_matches('/');
    format!("{trimmed}{CHAT_COMPLETIONS_PATH}")
}

#[derive(Debug, Clone)]
pub struct ChatCompletionsRequestContext {
    pub host: String,
    pub conversation_id: String,
    pub conversation_request_id: String,
    pub conversation_message_id: String,
    pub request_trace_id: String,
    pub prompt_prepare_start_time_ms: u64,
    pub http_send_time_ms: u64,
    pub request_id: String,
    pub b3_parent_span_id: String,
    pub b3_span_id: String,
}

impl ChatCompletionsRequestContext {
    pub fn new(base_url: &str) -> Self {
        let prompt_prepare_start_time_ms = current_time_millis();
        Self {
            host: request_host(base_url),
            conversation_id: next_hex_id(32),
            conversation_request_id: next_hex_id(32),
            conversation_message_id: next_hex_id(32),
            request_trace_id: next_uuid_like(),
            prompt_prepare_start_time_ms,
            http_send_time_ms: prompt_prepare_start_time_ms.saturating_add(291),
            request_id: next_hex_id(32),
            b3_parent_span_id: next_hex_id(16),
            b3_span_id: next_hex_id(16),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ChatCompletionsRequest {
    pub base_url: String,
    pub bearer_token: String,
    pub model: String,
    pub system_prompt: String,
    pub user_prompt: String,
    pub max_tokens: u32,
    pub temperature: f32,
    pub timeout_secs: u64,
    pub context: ChatCompletionsRequestContext,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HttpDebugHeader {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChatCompletionsRequestDebug {
    pub endpoint: String,
    pub headers: Vec<HttpDebugHeader>,
    pub body: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChatCompletionsResponseDebug {
    pub status: Option<u16>,
    pub headers: Vec<HttpDebugHeader>,
    pub body: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChatCompletionsDebugTrace {
    pub request: ChatCompletionsRequestDebug,
    pub response: ChatCompletionsResponseDebug,
}

#[derive(Debug, Clone)]
pub struct ChatCompletionsCallOutcome {
    pub result: Result<String, String>,
    pub debug: ChatCompletionsDebugTrace,
}

#[derive(Debug, Clone)]
struct AccumulatedSseStream {
    content: String,
    raw_body: String,
    error: Option<String>,
}

#[derive(Debug, Default, Clone)]
struct TokenDerivedHeaders {
    user_id: Option<String>,
    enterprise_id: Option<String>,
    tenant_id: Option<String>,
    domain: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AuthorizationHeaderMode {
    Raw,
    Redacted,
}

#[derive(Debug, Serialize)]
struct RequestMessage<'a> {
    role: &'static str,
    content: &'a str,
}

#[derive(Debug, Serialize)]
#[allow(non_snake_case)]
struct ChatCompletionsRequestBody<'a> {
    model: &'a str,
    max_tokens: u32,
    temperature: f64,
    reasoningEffort: &'static str,
    reasoning_summary: &'static str,
    reasoning_effort: &'static str,
    stream: bool,
    messages: [RequestMessage<'a>; 2],
}

pub fn build_chat_completions_request_debug(
    request: &ChatCompletionsRequest,
) -> ChatCompletionsRequestDebug {
    let body_text = build_request_body_text(request);
    ChatCompletionsRequestDebug {
        endpoint: chat_completions_endpoint(&request.base_url),
        headers: request_headers(request, &body_text, AuthorizationHeaderMode::Redacted),
        body: body_text,
    }
}

pub async fn call_chat_completions(request: ChatCompletionsRequest) -> Result<String, String> {
    call_chat_completions_with_observer(request, |_| {}).await
}

pub async fn call_chat_completions_with_observer<F>(
    request: ChatCompletionsRequest,
    observer: F,
) -> Result<String, String>
where
    F: FnMut(&str),
{
    call_chat_completions_with_debug(request, observer)
        .await
        .result
}

pub async fn call_chat_completions_with_debug<F>(
    request: ChatCompletionsRequest,
    observer: F,
) -> ChatCompletionsCallOutcome
where
    F: FnMut(&str),
{
    let mut debug = initialize_debug_trace(&request);
    if request.bearer_token.trim().is_empty() {
        return ChatCompletionsCallOutcome {
            result: Err("bearer token is empty".to_string()),
            debug,
        };
    }

    let endpoint = debug.request.endpoint.clone();
    let body_text = debug.request.body.clone();
    let request_headers = request_headers(&request, &body_text, AuthorizationHeaderMode::Raw);

    let client = Client::builder()
        .http1_only()
        .timeout(Duration::from_secs(request.timeout_secs.max(30)))
        .build()
        .map_err(|error| format!("http client build failed: {error}"));
    let client = match client {
        Ok(value) => value,
        Err(error) => {
            return ChatCompletionsCallOutcome {
                result: Err(error),
                debug,
            }
        }
    };

    let response = apply_request_headers(client.post(&endpoint), &request_headers)
        .body(body_text)
        .send()
        .await;
    let response = match response {
        Ok(value) => value,
        Err(error) => {
            return ChatCompletionsCallOutcome {
                result: Err(format!("http request failed: {error}")),
                debug,
            }
        }
    };

    let status = response.status();
    debug.response.status = Some(status.as_u16());
    debug.response.headers = header_entries(response.headers());
    if !status.is_success() {
        let snippet = response.text().await.unwrap_or_default();
        let truncated: String = snippet.chars().take(512).collect();
        debug.response.body = snippet;
        return ChatCompletionsCallOutcome {
            result: Err(format!("chat completions returned {}: {}", status, truncated)),
            debug,
        };
    }

    let stream = accumulate_sse_stream(response, observer).await;
    debug.response.body = stream.raw_body;
    ChatCompletionsCallOutcome {
        result: match stream.error {
            Some(error) => Err(error),
            None => Ok(stream.content),
        },
        debug,
    }
}

fn initialize_debug_trace(request: &ChatCompletionsRequest) -> ChatCompletionsDebugTrace {
    ChatCompletionsDebugTrace {
        request: build_chat_completions_request_debug(request),
        response: ChatCompletionsResponseDebug {
            status: None,
            headers: Vec::new(),
            body: String::new(),
        },
    }
}

fn build_request_body(request: &ChatCompletionsRequest) -> Value {
    serde_json::to_value(build_request_body_payload(request)).unwrap_or(Value::Null)
}

fn build_request_body_payload(request: &ChatCompletionsRequest) -> ChatCompletionsRequestBody<'_> {
    ChatCompletionsRequestBody {
        model: &request.model,
        max_tokens: request.max_tokens,
        temperature: round_temperature(request.temperature),
        reasoningEffort: TEMPLATE_REASONING_EFFORT,
        reasoning_summary: TEMPLATE_REASONING_SUMMARY,
        reasoning_effort: TEMPLATE_REASONING_EFFORT,
        stream: true,
        messages: [
            RequestMessage {
                role: "system",
                content: &request.system_prompt,
            },
            RequestMessage {
                role: "user",
                content: &request.user_prompt,
            },
        ],
    }
}

fn build_request_body_text(request: &ChatCompletionsRequest) -> String {
    serde_json::to_string(&build_request_body_payload(request))
        .unwrap_or_else(|_| build_request_body(request).to_string())
}

fn request_headers(
    request: &ChatCompletionsRequest,
    body_text: &str,
    authorization_mode: AuthorizationHeaderMode,
) -> Vec<HttpDebugHeader> {
    let derived_headers = derive_headers_from_bearer_token(&request.bearer_token);
    let authorization = match authorization_mode {
        AuthorizationHeaderMode::Raw => format!("Bearer {}", request.bearer_token.trim()),
        AuthorizationHeaderMode::Redacted => {
            format!("Bearer {}", redact_bearer_value(&request.bearer_token))
        }
    };
    let b3 = format!(
        "{}-{}-1-{}",
        request.context.request_id, request.context.b3_span_id, request.context.b3_parent_span_id
    );

    vec![
        header("host", request.context.host.clone()),
        header("connection", TEMPLATE_CONNECTION),
        header("Content-Type", TEMPLATE_CONTENT_TYPE),
        header("Authorization", authorization),
        header("X-Agent-Intent", TEMPLATE_AGENT_INTENT),
        header("X-Conversation-ID", request.context.conversation_id.clone()),
        header(
            "X-Conversation-Request-ID",
            request.context.conversation_request_id.clone(),
        ),
        header(
            "X-Conversation-Message-ID",
            request.context.conversation_message_id.clone(),
        ),
        header("X-Requested-With", "XMLHttpRequest"),
        header("X-IDE-Type", TEMPLATE_IDE_TYPE),
        header("X-IDE-Name", TEMPLATE_IDE_NAME),
        header("X-IDE-Version", TEMPLATE_IDE_VERSION),
        header("X-Product-Version", TEMPLATE_PRODUCT_VERSION),
        header("X-Request-Trace-Id", request.context.request_trace_id.clone()),
        header("X-Env-ID", TEMPLATE_ENV_ID),
        header(
            "X-User-Id",
            derived_headers
                .user_id
                .clone()
                .unwrap_or_else(|| TEMPLATE_USER_ID.to_string()),
        ),
        header(
            "X-Enterprise-Id",
            derived_headers
                .enterprise_id
                .clone()
                .unwrap_or_else(|| TEMPLATE_ENTERPRISE_ID.to_string()),
        ),
        header(
            "X-Tenant-Id",
            derived_headers
                .tenant_id
                .clone()
                .unwrap_or_else(|| TEMPLATE_TENANT_ID.to_string()),
        ),
        header(
            "X-Domain",
            derived_headers
                .domain
                .clone()
                .unwrap_or_else(|| TEMPLATE_DOMAIN.to_string()),
        ),
        header("User-Agent", TEMPLATE_USER_AGENT),
        header("X-Product", TEMPLATE_PRODUCT),
        header(
            "monitor_promptPrepareStartTime",
            request.context.prompt_prepare_start_time_ms.to_string(),
        ),
        header(
            "monitor_httpSendTime",
            request.context.http_send_time_ms.to_string(),
        ),
        header("X-Request-ID", request.context.request_id.clone()),
        header("b3", b3),
        header("X-B3-TraceId", request.context.request_id.clone()),
        header(
            "X-B3-ParentSpanId",
            request.context.b3_parent_span_id.clone(),
        ),
        header("X-B3-SpanId", request.context.b3_span_id.clone()),
        header("X-B3-Sampled", "1"),
        header("accept", TEMPLATE_ACCEPT),
        header("accept-language", TEMPLATE_ACCEPT_LANGUAGE),
        header("sec-fetch-mode", TEMPLATE_SEC_FETCH_MODE),
        header("accept-encoding", TEMPLATE_ACCEPT_ENCODING),
        header("content-length", body_text.as_bytes().len().to_string()),
    ]
}

fn apply_request_headers(
    mut builder: reqwest::RequestBuilder,
    headers: &[HttpDebugHeader],
) -> reqwest::RequestBuilder {
    for header in headers {
        if header.name.eq_ignore_ascii_case("content-length") {
            continue;
        }
        builder = builder.header(header.name.as_str(), header.value.as_str());
    }
    builder
}

fn header_entries(headers: &HeaderMap) -> Vec<HttpDebugHeader> {
    headers
        .iter()
        .map(|(name, value)| HttpDebugHeader {
            name: name.as_str().to_string(),
            value: value
                .to_str()
                .map(|text| text.to_string())
                .unwrap_or_else(|_| "<binary>".to_string()),
        })
        .collect()
}

fn header(name: &str, value: impl Into<String>) -> HttpDebugHeader {
    HttpDebugHeader {
        name: name.to_string(),
        value: value.into(),
    }
}

fn redact_bearer_value(token: &str) -> String {
    let trimmed = token.trim();
    if trimmed.is_empty() {
        return "<empty>".to_string();
    }

    let chars = trimmed.chars().collect::<Vec<_>>();
    let len = chars.len();
    if len <= 10 {
        return format!("<redacted len={len}>");
    }

    let prefix = chars.iter().take(4).collect::<String>();
    let suffix = chars[len.saturating_sub(4)..].iter().collect::<String>();
    format!("<redacted len={len} preview={prefix}...{suffix}>")
}

fn round_temperature(temperature: f32) -> f64 {
    ((temperature as f64) * 1000.0).round() / 1000.0
}

fn derive_headers_from_bearer_token(token: &str) -> TokenDerivedHeaders {
    let Some(payload) = decode_jwt_payload(token) else {
        return TokenDerivedHeaders::default();
    };

    TokenDerivedHeaders {
        user_id: claim_string(
            &payload,
            &["userId", "user_id", "uid", "sub", "user-id", "userid"],
        ),
        enterprise_id: claim_string(
            &payload,
            &[
                "enterpriseId",
                "enterprise_id",
                "enterprise",
                "enterprise-id",
            ],
        ),
        tenant_id: claim_string(&payload, &["tenantId", "tenant_id", "tenant", "tenant-id"]),
        domain: claim_string(&payload, &["domain", "host", "aud"]),
    }
}

fn decode_jwt_payload(token: &str) -> Option<Value> {
    let payload_segment = token.split('.').nth(1)?;
    let decoded = URL_SAFE_NO_PAD.decode(payload_segment.as_bytes()).ok()?;
    serde_json::from_slice::<Value>(&decoded).ok()
}

fn claim_string(payload: &Value, names: &[&str]) -> Option<String> {
    for name in names {
        let Some(value) = payload.get(*name) else {
            continue;
        };
        if let Some(text) = value.as_str() {
            if !text.trim().is_empty() {
                return Some(text.trim().to_string());
            }
        }
    }
    None
}

fn request_host(base_url: &str) -> String {
    Url::parse(base_url)
        .ok()
        .and_then(|url| {
            let host = url.host_str()?.to_string();
            Some(match url.port() {
                Some(port) => format!("{host}:{port}"),
                None => host,
            })
        })
        .filter(|host| !host.is_empty())
        .unwrap_or_else(|| TEMPLATE_DOMAIN.to_string())
}

fn current_time_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(u128::from(u64::MAX)) as u64
}

fn next_hex_id(len: usize) -> String {
    let mut output = String::with_capacity(len);
    while output.len() < len {
        let seed = REQUEST_CONTEXT_COUNTER.fetch_add(1, Ordering::Relaxed);
        let digest = Sha256::digest(
            format!(
                "commenter-http:{}:{}:{}:{}",
                std::process::id(),
                current_time_millis(),
                seed,
                len
            )
            .as_bytes(),
        );
        output.push_str(&hex_string(&digest));
    }
    output.truncate(len);
    output
}

fn next_uuid_like() -> String {
    let hex = next_hex_id(32);
    format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    )
}

fn hex_string(bytes: &[u8]) -> String {
    let mut text = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        let _ = write!(&mut text, "{byte:02x}");
    }
    text
}

async fn accumulate_sse_stream<F>(
    response: reqwest::Response,
    mut observer: F,
) -> AccumulatedSseStream
where
    F: FnMut(&str),
{
    let mut stream = response.bytes_stream();
    let mut buffer: Vec<u8> = Vec::new();
    let mut content = String::new();
    let mut raw_body = String::new();

    while let Some(chunk) = stream.next().await {
        let chunk = match chunk {
            Ok(value) => value,
            Err(error) => {
                return AccumulatedSseStream {
                    content,
                    raw_body,
                    error: Some(format!("sse stream error: {error}")),
                }
            }
        };
        raw_body.push_str(&String::from_utf8_lossy(&chunk));
        buffer.extend_from_slice(&chunk);

        while let Some(line_end) = buffer.iter().position(|byte| *byte == b'\n') {
            let raw = buffer.drain(..=line_end).collect::<Vec<u8>>();
            let line = String::from_utf8_lossy(&raw);
            let line = line.trim_end_matches(['\r', '\n']);
            if let Some(piece) = parse_sse_line(line).unwrap_or(None) {
                observer(&piece);
                content.push_str(&piece);
            }
        }
    }

    if !buffer.is_empty() {
        let line = String::from_utf8_lossy(&buffer);
        if let Some(piece) = parse_sse_line(line.trim_end_matches(['\r', '\n'])).unwrap_or(None) {
            observer(&piece);
            content.push_str(&piece);
        }
    }

    AccumulatedSseStream {
        content,
        raw_body,
        error: None,
    }
}

fn parse_sse_line(line: &str) -> Result<Option<String>, String> {
    let Some(payload) = line
        .strip_prefix("data: ")
        .or_else(|| line.strip_prefix("data:"))
    else {
        return Ok(None);
    };
    let payload = payload.trim();
    if payload.is_empty() || payload == "[DONE]" {
        return Ok(None);
    }

    let value: Value = match serde_json::from_str(payload) {
        Ok(value) => value,
        Err(_) => return Ok(None),
    };

    if let Some(delta) = value
        .pointer("/choices/0/delta/content")
        .and_then(Value::as_str)
    {
        if !delta.is_empty() {
            return Ok(Some(delta.to_string()));
        }
    }

    Ok(None)
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
    use serde_json::json;

    use super::*;

    fn sample_request() -> ChatCompletionsRequest {
        ChatCompletionsRequest {
            base_url: "https://example.com".to_string(),
            bearer_token: "secret-token-123456".to_string(),
            model: "glm-5.0".to_string(),
            system_prompt: "system".to_string(),
            user_prompt: "user".to_string(),
            max_tokens: 100,
            temperature: 1.0,
            timeout_secs: 30,
            context: ChatCompletionsRequestContext::new("https://example.com"),
        }
    }

    fn header_map(headers: &[HttpDebugHeader]) -> HashMap<&str, &str> {
        headers
            .iter()
            .map(|header| (header.name.as_str(), header.value.as_str()))
            .collect()
    }

    fn fake_jwt(payload: Value) -> String {
        let header = URL_SAFE_NO_PAD.encode(r#"{"alg":"none","typ":"JWT"}"#);
        let payload = URL_SAFE_NO_PAD.encode(payload.to_string());
        format!("{header}.{payload}.signature")
    }

    #[test]
    fn endpoint_appends_api_path_once() {
        assert_eq!(
            chat_completions_endpoint("https://example.com/"),
            "https://example.com/v2/chat/completions"
        );
        assert_eq!(
            chat_completions_endpoint("https://example.com"),
            "https://example.com/v2/chat/completions"
        );
    }

    #[test]
    fn parse_sse_line_extracts_content_and_ignores_reasoning() {
        assert_eq!(
            parse_sse_line(r#"data: {"choices":[{"delta":{"content":"你好"}}]}"#).unwrap(),
            Some("你好".to_string())
        );
        assert_eq!(
            parse_sse_line(r#"data: {"choices":[{"delta":{"reasoning_content":"忽略"}}]}"#)
                .unwrap(),
            None
        );
        assert_eq!(parse_sse_line("data: [DONE]").unwrap(), None);
        assert_eq!(parse_sse_line("event: ping").unwrap(), None);
    }

    #[test]
    fn request_debug_redacts_bearer_token() {
        let debug = build_chat_completions_request_debug(&sample_request());

        let authorization = debug
            .headers
            .iter()
            .find(|header| header.name == "Authorization")
            .expect("authorization header");
        assert!(authorization.value.contains("<redacted"));
        assert!(!authorization.value.contains("secret-token-123456"));
    }

    #[test]
    fn request_debug_matches_template_headers() {
        let debug = build_chat_completions_request_debug(&sample_request());
        let headers = header_map(&debug.headers);

        assert_eq!(headers.get("Content-Type"), Some(&"application/json;charset=UTF-8"));
        assert_eq!(headers.get("accept"), Some(&"*/*"));
        assert_eq!(headers.get("X-Agent-Intent"), Some(&"craft"));
        assert_eq!(headers.get("X-Requested-With"), Some(&"XMLHttpRequest"));
        assert_eq!(headers.get("X-IDE-Type"), Some(&"JetBrains"));
        assert_eq!(headers.get("X-IDE-Name"), Some(&"JetBrainsGoLand"));
        assert_eq!(headers.get("X-IDE-Version"), Some(&"GO-253.29346.379"));
        assert_eq!(headers.get("X-Product-Version"), Some(&"4.2.17133064"));
        assert_eq!(headers.get("X-Env-ID"), Some(&"production"));
        assert_eq!(headers.get("X-Enterprise-Id"), Some(&"unvcoding"));
        assert_eq!(headers.get("X-Tenant-Id"), Some(&"unvcoding"));
        assert_eq!(headers.get("X-Domain"), Some(&"unvcoding.copilot.qq.com"));
        assert_eq!(headers.get("X-Product"), Some(&"Cloud-Hosted"));
        assert_eq!(
            headers.get("User-Agent"),
            Some(&"JetBrainsGoLand/GO-253.29346.379 unvcoding/4.2.17133064")
        );
        assert_eq!(headers.get("accept-language"), Some(&"*"));
        assert_eq!(headers.get("sec-fetch-mode"), Some(&"cors"));
        assert_eq!(headers.get("accept-encoding"), Some(&"br, gzip, deflate"));
        assert!(headers.contains_key("content-length"));
        assert_eq!(headers.get("connection"), Some(&"keep-alive"));
        assert_eq!(headers.get("host"), Some(&"example.com"));
        assert!(headers.contains_key("X-Conversation-ID"));
        assert!(headers.contains_key("X-Conversation-Request-ID"));
        assert!(headers.contains_key("X-Conversation-Message-ID"));
        assert!(headers.contains_key("X-Request-Trace-Id"));
        assert!(headers.contains_key("X-User-Id"));
        assert!(headers.contains_key("monitor_promptPrepareStartTime"));
        assert!(headers.contains_key("monitor_httpSendTime"));
        assert!(headers.contains_key("X-Request-ID"));
        assert!(headers.contains_key("b3"));
        assert!(headers.contains_key("X-B3-TraceId"));
        assert!(headers.contains_key("X-B3-ParentSpanId"));
        assert!(headers.contains_key("X-B3-SpanId"));
        assert_eq!(headers.get("X-B3-Sampled"), Some(&"1"));

        let conversation_id = headers
            .get("X-Conversation-ID")
            .expect("conversation id header");
        assert_eq!(conversation_id.len(), 32);
        assert!(conversation_id.chars().all(|ch| ch.is_ascii_hexdigit()));

        let request_trace_id = headers
            .get("X-Request-Trace-Id")
            .expect("request trace id header");
        assert_eq!(request_trace_id.len(), 36);
        assert_eq!(request_trace_id.chars().filter(|ch| *ch == '-').count(), 4);
    }

    #[test]
    fn request_debug_matches_template_body_shape() {
        let body = build_request_body(&sample_request());

        assert_eq!(body.get("model").and_then(Value::as_str), Some("glm-5.0"));
        assert_eq!(body.get("max_tokens").and_then(Value::as_u64), Some(100));
        assert_eq!(body.get("temperature").and_then(Value::as_f64), Some(1.0));
        assert_eq!(body.get("stream").and_then(Value::as_bool), Some(true));
        assert_eq!(
            body.get("reasoningEffort").and_then(Value::as_str),
            Some("medium")
        );
        assert_eq!(
            body.get("reasoning_summary").and_then(Value::as_str),
            Some("auto")
        );
        assert_eq!(
            body.get("reasoning_effort").and_then(Value::as_str),
            Some("medium")
        );

        let messages = body
            .get("messages")
            .and_then(Value::as_array)
            .expect("messages array");
        assert_eq!(messages.len(), 2);
        assert_eq!(
            messages[0].get("role").and_then(Value::as_str),
            Some("system")
        );
        assert_eq!(
            messages[1].get("role").and_then(Value::as_str),
            Some("user")
        );
    }

    #[test]
    fn request_debug_prefers_jwt_claim_headers_over_template_samples() {
        let mut request = sample_request();
        request.bearer_token = fake_jwt(json!({
            "userId": "user-123",
            "tenantId": "tenant-456",
            "enterpriseId": "enterprise-789",
            "domain": "custom.example.com"
        }));

        let debug = build_chat_completions_request_debug(&request);
        let headers = header_map(&debug.headers);

        assert_eq!(headers.get("X-User-Id"), Some(&"user-123"));
        assert_eq!(headers.get("X-Tenant-Id"), Some(&"tenant-456"));
        assert_eq!(headers.get("X-Enterprise-Id"), Some(&"enterprise-789"));
        assert_eq!(headers.get("X-Domain"), Some(&"custom.example.com"));
    }
}
