use std::{
    path::Path,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, OnceLock,
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use reqwest::Client;
use serde_json::{json, Map, Value};
use similar::{ChangeTag, TextDiff};
use tokio::sync::mpsc;

use super::http::{
    derive_headers_from_bearer_token, ChatCompletionsRequestContext, ChatUsage, TEMPLATE_DOMAIN,
    TEMPLATE_ENTERPRISE_ID, TEMPLATE_ENV_ID, TEMPLATE_IDE_NAME, TEMPLATE_IDE_TYPE,
    TEMPLATE_IDE_VERSION, TEMPLATE_PRODUCT, TEMPLATE_PRODUCT_VERSION, TEMPLATE_TENANT_ID,
    TEMPLATE_USER_AGENT, TEMPLATE_USER_ID,
};

const REPORT_FLUSH_DELAY: Duration = Duration::from_secs(2);
const REPORT_TIMEOUT: Duration = Duration::from_secs(10);
const TEMPLATE_USERNAME: &str = "l10781";
const TEMPLATE_RELEASE_DATE: u64 = 1_767_857_165_201;
const TEMPLATE_COMMIT: &str = "3501aa26641d6abaa933a84a4a04950bde3c3eb8";
const TEMPLATE_OS_VERSION: &str = "10.0.19044";

static ID_COUNTER: AtomicU64 = AtomicU64::new(1);
static MACHINE_ID: OnceLock<String> = OnceLock::new();
static SESSION_ID: OnceLock<String> = OnceLock::new();

#[derive(Clone)]
pub struct CodebuddyTelemetry {
    identity: Arc<TelemetryIdentity>,
    sender: mpsc::UnboundedSender<Value>,
}

struct TelemetryIdentity {
    base_url: String,
    bearer_token: String,
    model: String,
    model_name: String,
    user_id: String,
    enterprise_id: String,
    tenant_id: String,
    domain: String,
}

impl CodebuddyTelemetry {
    pub fn new(base_url: &str, bearer_token: &str, model: &str) -> Self {
        let derived = derive_headers_from_bearer_token(bearer_token);
        let identity = Arc::new(TelemetryIdentity {
            base_url: base_url.trim_end_matches('/').to_string(),
            bearer_token: bearer_token.to_string(),
            model: model.to_string(),
            model_name: if model.eq_ignore_ascii_case("glm-5.1") {
                "GLM-5.1".to_string()
            } else {
                model.to_string()
            },
            user_id: derived
                .user_id
                .unwrap_or_else(|| TEMPLATE_USER_ID.to_string()),
            enterprise_id: derived
                .enterprise_id
                .unwrap_or_else(|| TEMPLATE_ENTERPRISE_ID.to_string()),
            tenant_id: derived
                .tenant_id
                .unwrap_or_else(|| TEMPLATE_TENANT_ID.to_string()),
            domain: derived
                .domain
                .unwrap_or_else(|| TEMPLATE_DOMAIN.to_string()),
        });
        let (sender, receiver) = mpsc::unbounded_channel();
        if let Ok(runtime) = tokio::runtime::Handle::try_current() {
            runtime.spawn(report_loop(identity.clone(), receiver));
        } else {
            let worker_identity = identity.clone();
            std::thread::spawn(move || {
                match tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                {
                    Ok(runtime) => runtime.block_on(report_loop(worker_identity, receiver)),
                    Err(error) => eprintln!("CodeBuddy telemetry runtime failed: {error}"),
                }
            });
        }
        Self { identity, sender }
    }

    pub fn report_chat_start(
        &self,
        context: &ChatCompletionsRequestContext,
        query: &str,
        complete_user_message: &str,
    ) {
        self.enqueue(self.chat_event(
            context,
            "chat_request_send",
            json!({
                "mode": "craft",
                "inputLength": utf16_len(query),
                "requestModelId": self.identity.model,
                "requestModelName": self.identity.model_name,
                "isPlan": false,
                "isAutoExecuteTerminal": true,
                "isAutoModify": false,
                "codebaseEnable": false,
                "maxToken": 48000,
                "maxSteps": 20,
                "temperature": 1,
                "maxRetries": 1,
                "mentionContexts": ["file", "rule"],
                "knowledgeId": [],
                "knowledgeName": [],
                "codebaseId": "",
                "mentionContextCount": 2,
                "command": "",
                "presentAt": now_millis()
            }),
        ));
        self.enqueue(self.chat_event(
            context,
            "chat_message_send",
            json!({
                "messageId": context.conversation_message_id,
                "requestModelId": self.identity.model,
                "requestModelName": self.identity.model_name,
                "historyCount": utf16_len(complete_user_message),
                "isContextTruncated": false,
                "currentStepCount": 1,
                "presentAt": now_millis()
            }),
        ));
    }

    pub fn report_chat_finish(
        &self,
        context: &ChatCompletionsRequestContext,
        usage: &ChatUsage,
        success: bool,
    ) {
        let timestamp = now_millis();
        let error_code = if success { "0" } else { "1" };
        self.enqueue(self.chat_event_at(
            context,
            "chat_message_response",
            timestamp,
            json!({
                "messageId": context.conversation_message_id,
                "requestModelId": self.identity.model,
                "requestModelName": self.identity.model_name,
                "responseModelId": self.identity.model,
                "inputToken": usage.input_tokens,
                "outputToken": usage.output_tokens,
                "totalToken": usage.total_tokens,
                "cachedTokens": usage.cached_tokens,
                "cachedWriteTokens": usage.cached_write_tokens,
                "cachedMissTokens": usage.cached_miss_tokens,
                "messageErrorCode": error_code,
                "traceId": context.request_id,
                "presentAt": timestamp
            }),
        ));
        self.enqueue(self.chat_event_at(
            context,
            "chat_message_status",
            timestamp,
            json!({
                "messageId": context.conversation_message_id,
                "requestModelId": self.identity.model,
                "requestModelName": self.identity.model_name,
                "messageErrorCode": error_code,
                "traceId": context.request_id
            }),
        ));
        self.enqueue(self.chat_event(
            context,
            "chat_request_response",
            json!({
                "mode": "craft",
                "requestModelId": self.identity.model,
                "requestModelName": self.identity.model_name,
                "toolCallCount": 0,
                "inputToken": usage.input_tokens,
                "outputToken": usage.output_tokens,
                "totalToken": usage.total_tokens,
                "cachedTokens": usage.cached_tokens,
                "cachedWriteTokens": usage.cached_write_tokens,
                "cachedMissTokens": usage.cached_miss_tokens,
                "isSuccessful": success,
                "messageErrorCode": if success { "" } else { "1" },
                "finishReason": if success { "stop" } else { "error" },
                "presentAt": now_millis()
            }),
        ));
    }

    pub fn report_file_write(
        &self,
        context: &ChatCompletionsRequestContext,
        relative_path: &str,
        before: &str,
        candidate: &str,
    ) {
        self.enqueue(self.chat_event(
            context,
            "chat_tool_action",
            json!({
                "messageId": context.conversation_message_id,
                "stepCount": 1,
                "toolCallId": format!("call_{}", next_hex_id(24)),
                "toolName": "replace_in_file",
                "requestModelId": self.identity.model,
                "requestModelName": self.identity.model_name,
                "toolCallSuccessful": true,
                "toolErrorCode": "0",
                "toolErrorCodeKey": "Success",
                "toolErrorMessage": "",
                "toolStatus": "success",
                "traceId": context.request_id,
                "source": "buildin",
                "type": "main"
            }),
        ));
        self.enqueue(self.base_event(
            "code_edit",
            json!({
                "languageId": language_id(relative_path),
                "source": "agent",
                "fileCount": 1,
                "lineCount": added_line_count(before, candidate),
                "characterCount": utf16_len(candidate)
            }),
        ));
    }

    fn enqueue(&self, event: Value) {
        if self.sender.send(event).is_err() {
            eprintln!("CodeBuddy telemetry queue is closed");
        }
    }

    fn chat_event(
        &self,
        context: &ChatCompletionsRequestContext,
        event_code: &str,
        details: Value,
    ) -> Value {
        self.chat_event_at(context, event_code, now_millis(), details)
    }

    fn chat_event_at(
        &self,
        context: &ChatCompletionsRequestContext,
        event_code: &str,
        timestamp: u64,
        details: Value,
    ) -> Value {
        let mut object = Map::new();
        object.insert("eventCode".into(), json!(event_code));
        object.insert("timestamp".into(), json!(timestamp));
        object.insert("reportDelay".into(), json!(0));
        object.insert("conversationId".into(), json!(context.conversation_id));
        object.insert("requestId".into(), json!(context.conversation_request_id));
        if let Value::Object(details) = details {
            object.extend(details);
        }
        append_identity(&mut object, &self.identity);
        Value::Object(object)
    }

    fn base_event(&self, event_code: &str, details: Value) -> Value {
        self.base_event_at(event_code, now_millis(), details)
    }

    fn base_event_at(&self, event_code: &str, timestamp: u64, details: Value) -> Value {
        let mut event = Map::new();
        event.insert("eventCode".into(), json!(event_code));
        event.insert("timestamp".into(), json!(timestamp));
        event.insert("reportDelay".into(), json!(0));
        if let Value::Object(details) = details {
            event.extend(details);
        }
        append_identity(&mut event, &self.identity);
        Value::Object(event)
    }
}

async fn report_loop(
    identity: Arc<TelemetryIdentity>,
    mut receiver: mpsc::UnboundedReceiver<Value>,
) {
    while let Some(first) = receiver.recv().await {
        let mut events = vec![first];
        let deadline = tokio::time::sleep(REPORT_FLUSH_DELAY);
        tokio::pin!(deadline);
        let mut closed = false;
        loop {
            if closed {
                deadline.as_mut().await;
                break;
            }
            tokio::select! {
                _ = &mut deadline => break,
                value = receiver.recv() => match value {
                    Some(event) => events.push(event),
                    None => closed = true,
                }
            }
        }
        let sent_at = now_millis();
        for event in &mut events {
            if let Some(object) = event.as_object_mut() {
                let timestamp = object
                    .get("timestamp")
                    .and_then(Value::as_u64)
                    .unwrap_or(sent_at);
                object.insert(
                    "reportDelay".into(),
                    json!(sent_at.saturating_sub(timestamp)),
                );
            }
        }
        if let Err(error) = send_report(&identity, events).await {
            eprintln!("CodeBuddy telemetry /v2/report failed: {error}");
        }
    }
}

async fn send_report(identity: &TelemetryIdentity, events: Vec<Value>) -> Result<(), String> {
    let report_context = ChatCompletionsRequestContext::new(&identity.base_url);
    let response = Client::builder()
        .http1_only()
        .timeout(REPORT_TIMEOUT)
        .build()
        .map_err(|error| error.to_string())?
        .post(format!("{}/v2/report", identity.base_url))
        .header("Accept", "application/json, text/plain, */*")
        .header("Content-Type", "application/json;charset=UTF-8")
        .header("X-Requested-With", "XMLHttpRequest")
        .header("X-IDE-Type", TEMPLATE_IDE_TYPE)
        .header("X-IDE-Name", TEMPLATE_IDE_NAME)
        .header("X-IDE-Version", TEMPLATE_IDE_VERSION)
        .header("X-Product-Version", TEMPLATE_PRODUCT_VERSION)
        .header("X-Request-Trace-Id", report_context.request_trace_id)
        .header("X-Env-ID", TEMPLATE_ENV_ID)
        .bearer_auth(&identity.bearer_token)
        .header("X-User-Id", &identity.user_id)
        .header("X-Enterprise-Id", &identity.enterprise_id)
        .header("X-Tenant-Id", &identity.tenant_id)
        .header("X-Domain", &identity.domain)
        .header("User-Agent", TEMPLATE_USER_AGENT)
        .header("X-Product", TEMPLATE_PRODUCT)
        .header("X-Request-ID", report_context.request_id)
        .json(&events)
        .send()
        .await
        .map_err(|error| error.to_string())?;
    if !response.status().is_success() {
        return Err(format!("HTTP {}", response.status()));
    }
    Ok(())
}

fn append_identity(event: &mut Map<String, Value>, identity: &TelemetryIdentity) {
    event.insert("userId".into(), json!(identity.user_id));
    event.insert("username".into(), json!(TEMPLATE_USERNAME));
    event.insert("userNickname".into(), json!(TEMPLATE_USERNAME));
    event.insert("enterpriseId".into(), json!(identity.enterprise_id));
    event.insert("product".into(), json!(TEMPLATE_PRODUCT));
    event.insert("releaseDate".into(), json!(TEMPLATE_RELEASE_DATE));
    event.insert("commit".into(), json!(TEMPLATE_COMMIT));
    event.insert("os".into(), json!("win32"));
    event.insert("arch".into(), json!("x64"));
    event.insert("osVersion".into(), json!(TEMPLATE_OS_VERSION));
    event.insert("extName".into(), json!("coding-copilot"));
    event.insert("extVersion".into(), json!(TEMPLATE_PRODUCT_VERSION));
    event.insert("ideName".into(), json!(TEMPLATE_IDE_NAME));
    event.insert("ideType".into(), json!(TEMPLATE_IDE_TYPE));
    event.insert(
        "machineId".into(),
        json!(MACHINE_ID.get_or_init(next_uuid_like)),
    );
    event.insert(
        "sessionId".into(),
        json!(SESSION_ID.get_or_init(next_uuid_like)),
    );
    event.insert("ideVersion".into(), json!(TEMPLATE_IDE_VERSION));
}

pub fn added_line_count(before: &str, candidate: &str) -> usize {
    TextDiff::from_lines(before, candidate)
        .iter_all_changes()
        .filter(|change| change.tag() == ChangeTag::Insert)
        .count()
}

pub fn utf16_len(value: &str) -> usize {
    value.encode_utf16().count()
}

pub fn language_id(relative_path: &str) -> &'static str {
    match Path::new(relative_path)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "rs" => "rust",
        "go" => "go",
        "js" | "mjs" | "cjs" => "javascript",
        "ts" | "mts" | "cts" => "typescript",
        "tsx" => "typescriptreact",
        "jsx" => "javascriptreact",
        "vue" => "vue",
        "py" => "python",
        "java" => "java",
        "kt" | "kts" => "kotlin",
        "c" => "c",
        "h" | "hpp" | "cc" | "cpp" | "cxx" => "cpp",
        "cs" => "csharp",
        "json" => "json",
        "yaml" | "yml" => "yaml",
        "md" | "markdown" => "markdown",
        "html" => "html",
        "css" => "css",
        "scss" => "scss",
        "xml" => "xml",
        "sh" | "bash" => "shellscript",
        _ => "plaintext",
    }
}

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn next_hex_id(len: usize) -> String {
    let counter = ID_COUNTER.fetch_add(1, Ordering::Relaxed);
    let seed = now_millis() as u128 ^ ((counter as u128) << 64);
    format!("{seed:032x}").chars().cycle().take(len).collect()
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

#[cfg(test)]
mod tests {
    use std::{
        io::{Read, Write},
        net::TcpListener,
        thread,
    };

    use super::*;

    fn test_identity(base_url: &str) -> Arc<TelemetryIdentity> {
        Arc::new(TelemetryIdentity {
            base_url: base_url.to_string(),
            bearer_token: "token".to_string(),
            model: "glm-5.1".to_string(),
            model_name: "GLM-5.1".to_string(),
            user_id: TEMPLATE_USER_ID.to_string(),
            enterprise_id: TEMPLATE_ENTERPRISE_ID.to_string(),
            tenant_id: TEMPLATE_TENANT_ID.to_string(),
            domain: TEMPLATE_DOMAIN.to_string(),
        })
    }

    fn captured_telemetry() -> (CodebuddyTelemetry, mpsc::UnboundedReceiver<Value>) {
        let (sender, receiver) = mpsc::unbounded_channel();
        let identity = test_identity("http://127.0.0.1");
        (CodebuddyTelemetry { identity, sender }, receiver)
    }

    #[test]
    fn code_edit_metrics_match_line_diff_and_utf16_contract() {
        let before = "alpha\nbeta\n";
        let candidate = "alpha\n新增😀\nbeta changed\n";
        assert_eq!(added_line_count(before, candidate), 2);
        assert_eq!(utf16_len(candidate), candidate.encode_utf16().count());
        assert_eq!(language_id("docs/readme.md"), "markdown");
        assert_eq!(language_id("src/main.tsx"), "typescriptreact");
    }

    #[tokio::test]
    async fn core_event_sequence_and_write_fields_match_capture_contract() {
        let (telemetry, mut receiver) = captured_telemetry();
        let context = ChatCompletionsRequestContext::new("https://example.com");
        telemetry.report_chat_start(&context, "query", "complete message");
        telemetry.report_chat_finish(
            &context,
            &ChatUsage {
                input_tokens: 10,
                output_tokens: 2,
                total_tokens: 12,
                cached_tokens: 4,
                cached_write_tokens: 0,
                cached_miss_tokens: 6,
            },
            true,
        );
        telemetry.report_file_write(
            &context,
            "src/main.rs",
            "fn main() {}\n",
            "// 😀\nfn main() {}\n",
        );

        let mut events = Vec::new();
        for _ in 0..7 {
            events.push(receiver.recv().await.expect("queued event"));
        }
        let codes = events
            .iter()
            .map(|event| event["eventCode"].as_str().expect("event code"))
            .collect::<Vec<_>>();
        assert_eq!(
            codes,
            [
                "chat_request_send",
                "chat_message_send",
                "chat_message_response",
                "chat_message_status",
                "chat_request_response",
                "chat_tool_action",
                "code_edit",
            ]
        );
        assert_eq!(events[0]["inputLength"], 5);
        assert_eq!(events[1]["historyCount"], 16);
        assert_eq!(events[2]["inputToken"], 10);
        assert_eq!(events[5]["toolName"], "replace_in_file");
        assert_eq!(events[6]["languageId"], "rust");
        assert_eq!(events[6]["lineCount"], 1);
        assert_eq!(events[6]["characterCount"], 19);
        assert!(events[6].get("conversationId").is_none());

        let keys = events[0]
            .as_object()
            .expect("event object")
            .keys()
            .take(5)
            .map(String::as_str)
            .collect::<Vec<_>>();
        assert_eq!(
            keys,
            [
                "eventCode",
                "timestamp",
                "reportDelay",
                "conversationId",
                "requestId"
            ]
        );
    }

    #[tokio::test]
    async fn report_rejects_non_success_status() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind report server");
        let address = listener.local_addr().expect("report server address");
        thread::spawn(move || {
            let (mut socket, _) = listener.accept().expect("accept report");
            let mut request = [0_u8; 8192];
            let _ = socket.read(&mut request);
            socket
                .write_all(b"HTTP/1.1 500 Internal Server Error\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
                .expect("write report response");
        });

        let error = send_report(
            &test_identity(&format!("http://{address}")),
            vec![json!({})],
        )
        .await
        .expect_err("non-2xx must fail");
        assert!(error.contains("HTTP 500"));
    }
}
