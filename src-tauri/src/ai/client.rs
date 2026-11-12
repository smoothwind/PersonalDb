//! AI 提供方客户端（OpenAI 兼容 chat/completions + SSE 流式）。
//!
//! 职责单一：把消息数组发到 `{baseURL}/chat/completions`，流式回吐 token。
//! Prompt 组装、上下文注入、预设模板都在前端——后端是哑管道，
//! 这样密钥只在本进程 keyring 与本模块之间流转，日志不落 key。

use std::time::Duration;

use futures_util::StreamExt;
use serde::{Deserialize, Serialize};

use crate::error::{AppError, DbResult};

/// 单条对话消息（前端组装完毕，含 system 提示）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiMessage {
    pub role: String,
    pub content: String,
}

/// 一次 AI 调用的目标配置（密钥经 keyRef 服务端解析，明文不过 IPC）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiCallConfig {
    pub base_url: String,
    pub model: String,
    /// keyring 引用（`ai:<providerId>`）；None/空 = 匿名端点（如 Ollama 本地）
    #[serde(default)]
    pub key_ref: Option<String>,
    #[serde(default)]
    pub temperature: Option<f64>,
    #[serde(default)]
    pub max_tokens: Option<u32>,
    /// 整体超时秒数（含流式读取），默认 180
    #[serde(default)]
    pub timeout_secs: Option<u64>,
}

fn http_client(timeout_secs: u64) -> DbResult<reqwest::Client> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(timeout_secs))
        .connect_timeout(Duration::from_secs(15))
        .build()
        .map_err(|e| AppError::General(format!("AI HTTP 客户端构建失败: {e}")))
}

fn endpoint(base_url: &str) -> String {
    format!("{}/chat/completions", base_url.trim_end_matches('/'))
}

fn auth_header(api_key: &Option<String>) -> Option<(String, String)> {
    api_key
        .as_ref()
        .filter(|k| !k.is_empty())
        .map(|k| ("Authorization".into(), format!("Bearer {k}")))
}

/// 从错误响应体提取一行可读摘要：优先 `error.message`，回退正文前 200 字符。
fn error_summary(body: &str) -> String {
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(body) {
        if let Some(msg) = v
            .pointer("/error/message")
            .and_then(|m| m.as_str())
            .filter(|s| !s.is_empty())
        {
            return msg.to_string();
        }
    }
    let t = body.trim();
    if t.len() > 200 {
        format!("{}…", &t[..200])
    } else {
        t.to_string()
    }
}

fn status_error(status: u16, body: &str) -> AppError {
    let hint = match status {
        401 | 403 => "（密钥无效或无权限）",
        404 => "（模型名或 baseURL 不存在——检查预设/自定义端点）",
        429 => "（限流/配额用尽）",
        500..=599 => "（提供方服务端错误）",
        _ => "",
    };
    AppError::General(format!("AI 请求失败 HTTP {status}{hint}: {}", error_summary(body)))
}

// =============================================================================
// SSE 增量解析（纯函数，单测覆盖）
// =============================================================================

/// SSE 增量解析器：逐块喂字节，产出完整事件的 `data:` 载荷。
/// 行式处理（\n 分行、容忍 \r），空行 = 事件边界；同名多 `data:` 行拼接。
#[derive(Default)]
pub struct SseParser {
    buf: String,
    data_lines: Vec<String>,
}

impl SseParser {
    pub fn feed(&mut self, chunk: &[u8]) -> Vec<String> {
        self.buf.push_str(&String::from_utf8_lossy(chunk));
        let mut out = Vec::new();
        while let Some(pos) = self.buf.find('\n') {
            let mut line = self.buf[..pos].to_string();
            self.buf.replace_range(..pos + 1, "");
            if line.ends_with('\r') {
                line.pop();
            }
            if line.is_empty() {
                if !self.data_lines.is_empty() {
                    out.push(std::mem::take(&mut self.data_lines).join("\n"));
                }
            } else if let Some(rest) = line.strip_prefix("data:") {
                self.data_lines.push(rest.strip_prefix(' ').unwrap_or(rest).to_string());
            }
            // 其他行（event:/id:/comment/心跳）忽略
        }
        out
    }
}

/// 从 chat.completions chunk 提取 (增量文本, finish_reason)。
pub fn extract_delta(data: &str) -> (Option<String>, Option<String>) {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(data) else {
        return (None, None);
    };
    let choice = &v["choices"][0];
    let delta = choice["delta"]["content"]
        .as_str()
        .or(choice["text"].as_str())
        .map(|s| s.to_string());
    let finish = choice["finish_reason"].as_str().map(|s| s.to_string());
    (delta, finish)
}

/// baseURL → host（日志用；不带路径与密钥）
fn host_of(base_url: &str) -> String {
    base_url
        .split("://")
        .nth(1)
        .and_then(|rest| rest.split(['/']).next())
        .unwrap_or(base_url)
        .to_string()
}

// =============================================================================
// 流式对话
// =============================================================================

/// 流式对话：逐 token 回调，返回 finish_reason（服务端正常结束 = Some）。
/// 回调返回 false = 下游 Channel 已断（前端页签关闭），立即终止流。
pub async fn stream_chat(
    cfg: &AiCallConfig,
    api_key: Option<String>,
    messages: &[AiMessage],
    mut on_token: impl FnMut(String) -> bool,
) -> DbResult<Option<String>> {
    let timeout = cfg.timeout_secs.unwrap_or(180).clamp(5, 600);
    let client = http_client(timeout)?;
    let mut body = serde_json::json!({
        "model": cfg.model,
        "messages": messages,
        "stream": true,
    });
    if let Some(t) = cfg.temperature {
        body["temperature"] = serde_json::json!(t);
    }
    if let Some(m) = cfg.max_tokens {
        body["max_tokens"] = serde_json::json!(m);
    }
    let mut req = client
        .post(endpoint(&cfg.base_url))
        .header("Content-Type", "application/json")
        .json(&body);
    if let Some(h) = auth_header(&api_key) {
        req = req.header(h.0, h.1);
    }
    let resp = req.send().await.map_err(|e| {
        // 日志红线：不落密钥与消息内容，仅 host/model/错误摘要
        let (msg, with_detail): (&str, bool) = if e.is_timeout() {
            (
                "AI 请求超时——网络不通或提供方无响应（可在设置调大超时）",
                false,
            )
        } else if e.is_connect() {
            ("AI 端点连接失败——检查 baseURL/网络/代理", false)
        } else {
            ("AI 请求失败", true)
        };
        log::warn!(
            target: "ai",
            "stream 请求失败 host={} model={} msgs={}: {msg}{}",
            host_of(&cfg.base_url),
            cfg.model,
            messages.len(),
            if with_detail { format!(" ({e})") } else { String::new() },
        );
        AppError::General(if with_detail { format!("{msg}: {e}") } else { msg.to_string() })
    })?;
    let status = resp.status().as_u16();
    if status >= 400 {
        let text = resp.text().await.unwrap_or_default();
        log::warn!(target: "ai", "stream HTTP {status} host={} model={}: {}",
            host_of(&cfg.base_url), cfg.model, error_summary(&text));
        return Err(status_error(status, &text));
    }

    let mut stream = resp.bytes_stream();
    let mut parser = SseParser::default();
    let mut finish: Option<String> = None;
    loop {
        let chunk = match stream.next().await {
            Some(Ok(b)) => b,
            Some(Err(e)) => {
                log::warn!(target: "ai", "流读取中断 host={} model={}: {e}",
                    host_of(&cfg.base_url), cfg.model);
                return Err(AppError::General(format!("AI 流读取中断: {e}")));
            }
            None => break,
        };
        for data in parser.feed(&chunk) {
            if data == "[DONE]" {
                return Ok(finish);
            }
            let (delta, fr) = extract_delta(&data);
            if let Some(fr) = fr {
                finish = Some(fr);
            }
            if let Some(d) = delta {
                if !d.is_empty() && !on_token(d) {
                    // 下游 Channel 已断（页签关闭/重载）：终止流，避免白打 API
                    log::info!(target: "ai", "下游断开，终止流 host={} model={}",
                        host_of(&cfg.base_url), cfg.model);
                    return Ok(finish);
                }
            }
        }
    }
    Ok(finish)
}

/// 非流式探测（设置页「测试连接」）：min 请求验证端点/模型/密钥三者通。
pub async fn test_connection(cfg: &AiCallConfig, api_key: Option<String>) -> DbResult<String> {
    let timeout = cfg.timeout_secs.unwrap_or(60).clamp(5, 120);
    let client = http_client(timeout)?;
    let mut body = serde_json::json!({
        "model": cfg.model,
        "messages": [{"role": "user", "content": "ping"}],
        "max_tokens": 8,
        "stream": false,
    });
    if let Some(m) = cfg.max_tokens {
        body["max_tokens"] = serde_json::json!(m.min(8));
    }
    let mut req = client
        .post(endpoint(&cfg.base_url))
        .header("Content-Type", "application/json")
        .json(&body);
    if let Some(h) = auth_header(&api_key) {
        req = req.header(h.0, h.1);
    }
    let resp = req.send().await.map_err(|e| {
        log::warn!(target: "ai", "测试连接不可达 host={} model={}: {e}",
            host_of(&cfg.base_url), cfg.model);
        AppError::General(format!("AI 端点不可达: {e}"))
    })?;
    let status = resp.status().as_u16();
    let text = resp.text().await.unwrap_or_default();
    if status >= 400 {
        log::warn!(target: "ai", "测试连接 HTTP {status} host={} model={}: {}",
            host_of(&cfg.base_url), cfg.model, error_summary(&text));
        return Err(status_error(status, &text));
    }
    let v: serde_json::Value = serde_json::from_str(&text).map_err(|_| {
        log::warn!(target: "ai", "测试连接响应非 JSON host={} model={}",
            host_of(&cfg.base_url), cfg.model);
        AppError::General("AI 响应非 JSON——baseURL 可能不是 OpenAI 兼容端点".into())
    })?;
    let reply = v["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("")
        .trim()
        .to_string();
    Ok(if reply.is_empty() {
        "ok".into()
    } else {
        reply
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sse_parser_按事件边界产出() {
        let mut p = SseParser::default();
        let mut out = p.feed(b"data: {\"a\":1}\n\ndata: {\"b\":2}\n\ndat");
        assert_eq!(out, vec!["{\"a\":1}", "{\"b\":2}"]);
        // 半截事件跨块续传
        out = p.feed(b"a: [DONE]\n\n");
        assert_eq!(out, vec!["[DONE]"]);
    }

    #[test]
    fn sse_parser_容忍_crlf_与无空格_data() {
        let mut p = SseParser::default();
        let out = p.feed(b"data:x1\r\n\r\ndata: x2\r\n\r\n");
        assert_eq!(out, vec!["x1", "x2"]);
    }

    #[test]
    fn sse_parser_忽略注释与非_data行() {
        let mut p = SseParser::default();
        let out = p.feed(b": keepalive\n\nevent: ping\ndata: y\n\n");
        assert_eq!(out, vec!["y"]);
    }

    #[test]
    fn extract_delta_读增量与结束原因() {
        let (d, f) = extract_delta(r#"{"choices":[{"delta":{"content":"SE"},"finish_reason":null}]}"#);
        assert_eq!(d.as_deref(), Some("SE"));
        assert_eq!(f, None);
        let (d, f) = extract_delta(r#"{"choices":[{"delta":{},"finish_reason":"stop"}]}"#);
        assert_eq!(d, None);
        assert_eq!(f.as_deref(), Some("stop"));
        // 旧式 completions 字段
        let (d, _) = extract_delta(r#"{"choices":[{"text":"x"}]}"#);
        assert_eq!(d.as_deref(), Some("x"));
    }

    #[test]
    fn error_summary_优先_error_message() {
        assert_eq!(error_summary(r#"{"error":{"message":"bad key"}}"#), "bad key");
        assert_eq!(error_summary("plain text error"), "plain text error");
        let long = "x".repeat(300);
        let s = error_summary(&long);
        // '…' 为 3 字节 UTF-8：200 字符 + 1 = 201 字符（字节 203）
        assert!(s.ends_with('…') && s.chars().count() == 201);
    }
}
