//! boa 引擎封装：同步执行用户音源脚本（调用方负责放进 `spawn_blocking`）。
//!
//! 设计约束（`_dev/script-spec/API.md`）：
//! * 每次调用新建一个 `Context`（契约允许宿主随时重建脚本实例）；
//! * 跨语言边界一律走 JSON 字符串（前导在 JS 侧完成校验与归一化）；
//! * 网络只能从宿主函数发出，且必须记账（次数 + 墙钟）；
//! * 任何脚本异常都转成 `ScriptCallError`，不会 panic、不会影响其它音源。

use std::cell::RefCell;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use base64::Engine as _;
use boa_engine::{
    Context, JsError, JsNativeError, JsResult, JsString, JsValue, NativeFunction, Source,
};
use serde::{Deserialize, Serialize};

use super::limits::{
    script_too_large_message, MAX_HTTP_REQUESTS_PER_CALL, MAX_LOOP_ITERATIONS, MAX_RECURSION_DEPTH,
    MAX_SCRIPT_BYTES, MAX_SLEEP_MS, MAX_WALL_CLOCK_MS, TIMEOUT_MESSAGE,
};
use super::model::{ScriptCallError, ScriptSong, ScriptUrl};

const PRELUDE: &str = include_str!("prelude.js");

const BRIDGE_DESCRIBE: &str = "__hd_describe";
const BRIDGE_SEARCH: &str = "__hd_search";
const BRIDGE_GET_URL: &str = "__hd_get_url";
const PENDING_RESULT: &str = "__hd_pending_result";

const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36";
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);
const MAX_RESPONSE_BYTES: usize = 16 * 1024 * 1024;

/// `source` 的关键信息（安装校验、列表展示都要用）。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScriptDescriptor {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub qualities: Vec<String>,
    #[serde(default)]
    pub has_source: bool,
    #[serde(default)]
    pub has_search: bool,
    #[serde(default)]
    pub has_get_url: bool,
}

/// 单次脚本调用的宿主状态（墙钟 + 网络请求计数），线程局部，`spawn_blocking` 内设置。
#[derive(Debug, Clone, Copy)]
struct Invocation {
    deadline: Instant,
    requests: u32,
}

thread_local! {
    static HOST_STATE: RefCell<Option<Invocation>> = const { RefCell::new(None) };
}

struct HostScope;

impl HostScope {
    fn begin() -> Self {
        HOST_STATE.with(|slot| {
            *slot.borrow_mut() = Some(Invocation {
                deadline: Instant::now() + Duration::from_millis(MAX_WALL_CLOCK_MS),
                requests: 0,
            });
        });
        Self
    }

    /// 墙钟超限 → `TIMEOUT_MESSAGE`（确定性错误，不再重试）。
    fn check_deadline() -> Result<(), String> {
        HOST_STATE.with(|slot| match slot.borrow().as_ref() {
            Some(invocation) if Instant::now() > invocation.deadline => {
                Err(TIMEOUT_MESSAGE.to_string())
            }
            _ => Ok(()),
        })
    }

    /// 取一次网络请求额度（同时检查墙钟与单次调用上限）。
    fn take_request_slot() -> Result<(), String> {
        HOST_STATE.with(|slot| {
            let mut guard = slot.borrow_mut();
            let Some(invocation) = guard.as_mut() else {
                return Err("音源脚本宿主状态缺失".to_string());
            };
            if Instant::now() > invocation.deadline {
                return Err(TIMEOUT_MESSAGE.to_string());
            }
            if invocation.requests >= MAX_HTTP_REQUESTS_PER_CALL {
                return Err(format!(
                    "音源脚本请求过于频繁（单次调用最多 {MAX_HTTP_REQUESTS_PER_CALL} 次网络请求）"
                ));
            }
            invocation.requests += 1;
            Ok(())
        })
    }
}

impl Drop for HostScope {
    fn drop(&mut self) {
        HOST_STATE.with(|slot| {
            *slot.borrow_mut() = None;
        });
    }
}

/// 描述脚本（安装校验 / 列表展示）。
pub fn describe(script: &str) -> Result<ScriptDescriptor, ScriptCallError> {
    let _scope = HostScope::begin();
    let mut session = ScriptSession::new(script)?;
    let value = session.call_bridge(BRIDGE_DESCRIBE, &[])?;
    HostScope::check_deadline().map_err(ScriptCallError::deterministic)?;
    serde_json::from_value(value)
        .map_err(|error| ScriptCallError::deterministic(format!("脚本描述信息无法解析: {error}")))
}

/// 调用脚本搜索。
pub fn search(
    script: &str,
    keyword: &str,
    page: u32,
    limit: u32,
) -> Result<Vec<ScriptSong>, ScriptCallError> {
    let _scope = HostScope::begin();
    let mut session = ScriptSession::new(script)?;
    let value = session.call_bridge(
        BRIDGE_SEARCH,
        &[
            JsValue::from(JsString::from(keyword)),
            JsValue::from(JsString::from(page.to_string())),
            JsValue::from(JsString::from(limit.to_string())),
        ],
    )?;
    HostScope::check_deadline().map_err(ScriptCallError::deterministic)?;
    serde_json::from_value(value)
        .map_err(|error| ScriptCallError::deterministic(format!("脚本搜索结果无法解析: {error}")))
}

/// 调用脚本取直链。
pub fn get_url(
    script: &str,
    song: &ScriptSong,
    quality: &str,
) -> Result<ScriptUrl, ScriptCallError> {
    let _scope = HostScope::begin();
    let mut session = ScriptSession::new(script)?;
    let value = session.call_bridge(
        BRIDGE_GET_URL,
        &[
            JsValue::from(JsString::from(song.to_script_json())),
            JsValue::from(JsString::from(quality)),
        ],
    )?;
    HostScope::check_deadline().map_err(ScriptCallError::deterministic)?;
    serde_json::from_value(value)
        .map_err(|error| ScriptCallError::deterministic(format!("脚本取链结果无法解析: {error}")))
}

/// 从 URL 抓取脚本文本（`install_script_source` 只给了 `url` 的情况）。
pub fn fetch_text(url: &str) -> Result<String, ScriptCallError> {
    let _scope = HostScope::begin();
    let payload = perform_request("GET", url, "{}", "").map_err(ScriptCallError::from_message)?;
    let value: serde_json::Value = serde_json::from_str(&payload)
        .map_err(|error| ScriptCallError::deterministic(format!("下载音源脚本失败: {error}")))?;
    let status = value
        .get("status")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    if status != 200 {
        return Err(ScriptCallError::deterministic(format!(
            "下载音源脚本失败: HTTP {status}"
        )));
    }
    let body = value
        .get("body")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_string();
    if body.trim().is_empty() {
        return Err(ScriptCallError::deterministic(
            "下载音源脚本失败: 响应为空".to_string(),
        ));
    }
    if body.len() > MAX_SCRIPT_BYTES {
        return Err(ScriptCallError::deterministic(script_too_large_message(
            body.len(),
        )));
    }
    Ok(body)
}

struct ScriptSession {
    context: Context,
}
impl ScriptSession {
    fn new(script: &str) -> Result<Self, ScriptCallError> {
        if script.len() > MAX_SCRIPT_BYTES {
            return Err(ScriptCallError::deterministic(script_too_large_message(
                script.len(),
            )));
        }

        let mut context = Context::default();
        {
            let limits = context.runtime_limits_mut();
            limits.set_loop_iteration_limit(MAX_LOOP_ITERATIONS);
            limits.set_recursion_limit(MAX_RECURSION_DEPTH);
        }
        register_host(
            &mut context,
            "__hd_request",
            NativeFunction::from_fn_ptr(host_request),
            4,
        )?;
        register_host(
            &mut context,
            "__hd_log",
            NativeFunction::from_fn_ptr(host_log),
            1,
        )?;
        register_host(
            &mut context,
            "__hd_sleep",
            NativeFunction::from_fn_ptr(host_sleep),
            1,
        )?;
        register_host(
            &mut context,
            "__hd_b64",
            NativeFunction::from_fn_ptr(host_b64),
            2,
        )?;

        let mut session = Self { context };
        session.eval(PRELUDE).map_err(|message| {
            ScriptCallError::deterministic(format!("脚本宿主初始化失败: {message}"))
        })?;

        let value = session
            .eval(script)
            .map_err(ScriptCallError::deterministic)?;
        // 契约允许把 source 作为脚本最后一个表达式返回。
        if value.as_object().is_some() && !session.describe_has_source()? {
            let global = session.context.global_object();
            let _ = global.set(JsString::from("source"), value, false, &mut session.context);
        }
        // 再跑一次前导：恢复被脚本遮蔽的桥接函数（前导全部走 globalThis 属性赋值，不会冲突）。
        session.eval(PRELUDE).map_err(|message| {
            ScriptCallError::deterministic(format!("脚本宿主初始化失败: {message}"))
        })?;
        Ok(session)
    }

    fn eval(&mut self, code: &str) -> Result<JsValue, String> {
        match self.context.eval(Source::from_bytes(code)) {
            Ok(value) => Ok(value),
            Err(error) => Err(js_error_message(&mut self.context, error)),
        }
    }

    fn describe_has_source(&mut self) -> Result<bool, ScriptCallError> {
        let envelope = self.call_bridge(BRIDGE_DESCRIBE, &[])?;
        Ok(envelope
            .get("hasSource")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false))
    }

    fn call_bridge(
        &mut self,
        name: &str,
        args: &[JsValue],
    ) -> Result<serde_json::Value, ScriptCallError> {
        let global = self.context.global_object();
        let value = global
            .get(JsString::from(name), &mut self.context)
            .map_err(|error| {
                ScriptCallError::deterministic(format!(
                    "脚本宿主内部错误: {}",
                    js_error_message(&mut self.context, error)
                ))
            })?;
        let callable = value
            .as_callable()
            .ok_or_else(|| ScriptCallError::deterministic("脚本宿主内部错误: 桥接函数缺失"))?;
        let result = callable
            .call(&JsValue::undefined(), args, &mut self.context)
            .map_err(|error| {
                ScriptCallError::deterministic(js_error_message(&mut self.context, error))
            })?;
        let text = result
            .to_string(&mut self.context)
            .map(|value| value.to_std_string_escaped())
            .unwrap_or_default();
        let mut envelope = parse_envelope(&text)?;

        if envelope
            .get("pending")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false)
        {
            self.context.run_jobs();
            let pending = {
                let global = self.context.global_object();
                global
                    .get(JsString::from(PENDING_RESULT), &mut self.context)
                    .map_err(|error| {
                        ScriptCallError::deterministic(format!(
                            "脚本宿主内部错误: {}",
                            js_error_message(&mut self.context, error)
                        ))
                    })?
            };
            let text = pending
                .to_string(&mut self.context)
                .map(|value| value.to_std_string_escaped())
                .unwrap_or_default();
            if text.is_empty() || text == "null" || text == "undefined" {
                return Err(ScriptCallError::deterministic(TIMEOUT_MESSAGE));
            }
            envelope = parse_envelope(&text)?;
        }

        if envelope.get("ok").and_then(serde_json::Value::as_bool) != Some(true) {
            let message = envelope
                .get("error")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("脚本执行失败")
                .to_string();
            return Err(ScriptCallError::from_message(message));
        }
        Ok(envelope
            .get("value")
            .cloned()
            .unwrap_or(serde_json::Value::Null))
    }
}

fn parse_envelope(text: &str) -> Result<serde_json::Value, ScriptCallError> {
    serde_json::from_str(text)
        .map_err(|error| ScriptCallError::deterministic(format!("脚本返回值无法解析: {error}")))
}

fn register_host(
    context: &mut Context,
    name: &str,
    function: NativeFunction,
    length: usize,
) -> Result<(), ScriptCallError> {
    context
        .register_global_builtin_callable(JsString::from(name), length, function)
        .map_err(|error| {
            ScriptCallError::deterministic(format!(
                "脚本宿主初始化失败: {}",
                js_error_message(context, error)
            ))
        })
}

/// 把 JS 抛出的异常转成用户可读文案（优先取 `error.message`）。
fn js_error_message(context: &mut Context, error: JsError) -> String {
    let opaque = error.to_opaque(context);
    if let Some(object) = opaque.as_object() {
        if let Ok(message) = object.get(JsString::from("message"), context) {
            let text = message
                .to_string(context)
                .map(|value| value.to_std_string_escaped())
                .unwrap_or_default();
            if !text.is_empty() && text != "undefined" {
                return text;
            }
        }
    }
    opaque
        .to_string(context)
        .map(|value| value.to_std_string_escaped())
        .unwrap_or_else(|_| "脚本执行失败".to_string())
}

fn host_error(message: String) -> JsError {
    JsNativeError::typ().with_message(message).into()
}

fn argument_text(args: &[JsValue], index: usize, context: &mut Context) -> String {
    match args.get(index) {
        Some(value) => value
            .to_string(context)
            .map(|text| text.to_std_string_escaped())
            .unwrap_or_default(),
        None => String::new(),
    }
}

fn http_client() -> Result<&'static reqwest::blocking::Client, String> {
    static CLIENT: OnceLock<reqwest::blocking::Client> = OnceLock::new();
    if let Some(client) = CLIENT.get() {
        return Ok(client);
    }
    let client = reqwest::blocking::Client::builder()
        .user_agent(USER_AGENT)
        .connect_timeout(CONNECT_TIMEOUT)
        .timeout(REQUEST_TIMEOUT)
        .build()
        .map_err(|error| format!("网络错误: 初始化 HTTP 客户端失败: {error}"))?;
    let _ = CLIENT.set(client);
    CLIENT
        .get()
        .ok_or_else(|| "网络错误: 初始化 HTTP 客户端失败".to_string())
}

/// 实际发请求（仅在宿主函数里同步调用）。
fn perform_request(
    method: &str,
    url: &str,
    headers_json: &str,
    body: &str,
) -> Result<String, String> {
    HostScope::take_request_slot()?;
    let client = http_client()?;

    let method = method.trim().to_ascii_uppercase();
    let method = reqwest::Method::from_bytes(method.as_bytes())
        .map_err(|_| format!("请求方法无效: {method}"))?;
    let url = reqwest::Url::parse(url.trim()).map_err(|error| format!("请求地址无效: {error}"))?;

    let mut request = client.request(method, url);
    if !headers_json.trim().is_empty() {
        let parsed: serde_json::Value =
            serde_json::from_str(headers_json).map_err(|error| format!("请求头无效: {error}"))?;
        if let Some(object) = parsed.as_object() {
            for (name, value) in object {
                let Some(text) = value.as_str() else {
                    continue;
                };
                let (Ok(name), Ok(value)) = (
                    reqwest::header::HeaderName::from_bytes(name.as_bytes()),
                    reqwest::header::HeaderValue::from_str(text),
                ) else {
                    continue;
                };
                request = request.header(name, value);
            }
        }
    }
    if !body.is_empty() {
        request = request.body(body.to_string());
    }

    let response = request
        .send()
        .map_err(|error| format!("网络错误: {error}"))?;
    let status = response.status().as_u16();
    let mut headers = serde_json::Map::new();
    for (name, value) in response.headers() {
        headers.insert(
            name.as_str().to_string(),
            serde_json::Value::String(value.to_str().unwrap_or_default().to_string()),
        );
    }
    let bytes = response
        .bytes()
        .map_err(|error| format!("读取响应失败: {error}"))?;
    if bytes.len() > MAX_RESPONSE_BYTES {
        return Err(format!("响应过大（超过 {MAX_RESPONSE_BYTES} 字节）"));
    }
    let text = String::from_utf8_lossy(&bytes).to_string();
    Ok(serde_json::json!({
        "status": status,
        "headers": headers,
        "body": text,
    })
    .to_string())
}

fn host_request(_this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let method = argument_text(args, 0, context);
    let url = argument_text(args, 1, context);
    let headers = argument_text(args, 2, context);
    let body = argument_text(args, 3, context);
    match perform_request(&method, &url, &headers, &body) {
        Ok(payload) => Ok(JsValue::from(JsString::from(payload))),
        Err(message) => Err(host_error(message)),
    }
}

fn host_log(_this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let message = argument_text(args, 0, context);
    log::info!(target: "hotdownloader::script", "音源脚本: {message}");
    Ok(JsValue::undefined())
}

fn host_sleep(_this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let requested = argument_text(args, 0, context)
        .trim()
        .parse::<u64>()
        .unwrap_or(0);
    HostScope::check_deadline().map_err(host_error)?;
    std::thread::sleep(Duration::from_millis(requested.min(MAX_SLEEP_MS)));
    HostScope::check_deadline().map_err(host_error)?;
    Ok(JsValue::undefined())
}

fn host_b64(_this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let operation = argument_text(args, 0, context);
    let text = argument_text(args, 1, context);
    match operation.trim() {
        "encode" => Ok(JsValue::from(JsString::from(
            base64::engine::general_purpose::STANDARD.encode(text.as_bytes()),
        ))),
        "decode" => match base64::engine::general_purpose::STANDARD.decode(text.as_bytes()) {
            Ok(bytes) => match String::from_utf8(bytes) {
                Ok(decoded) => Ok(JsValue::from(JsString::from(decoded))),
                Err(_) => Err(host_error("base64 解码结果不是 UTF-8 文本".to_string())),
            },
            Err(error) => Err(host_error(format!("base64 解码失败: {error}"))),
        },
        other => Err(host_error(format!("base64 不支持的操作: {other}"))),
    }
}
