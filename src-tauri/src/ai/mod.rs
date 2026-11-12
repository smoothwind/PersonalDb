//! AI 能力（OpenAI 兼容流式客户端）——Tauri 命令入口与任务登记。
//!
//! 密钥经 `keyRef` 服务端从 keyring 解析（明文不过 IPC、不进日志）；
//! 流式经 `tauri::ipc::Channel` 逐 token 推送，前端按 reqId 过期丢弃
//! （runSeq 同款语义）；取消走 `ai_cancel` 直接 abort 任务。

mod client;

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use serde::Serialize;
use tauri::ipc::Channel;
use tokio::task::JoinHandle;

use crate::error::{AppError, DbResult};
use crate::secrets;

use client::{AiCallConfig, AiMessage};

/// 流式事件（Channel 逐条推送）
#[derive(Debug, Serialize, Clone)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum AiStreamEvent {
    Token { delta: String },
    Done { finish: Option<String> },
    Error { message: String },
}

fn tasks() -> &'static Mutex<HashMap<String, JoinHandle<()>>> {
    static TASKS: OnceLock<Mutex<HashMap<String, JoinHandle<()>>>> = OnceLock::new();
    TASKS.get_or_init(|| Mutex::new(HashMap::new()))
}

/// keyRef → keyring 取值；匿名端点（无 keyRef）放行。
fn resolve_key(key_ref: &Option<String>) -> DbResult<Option<String>> {
    match key_ref.as_deref().filter(|s| !s.is_empty()) {
        None => Ok(None),
        Some(id) => match secrets::get(id) {
            Some(k) if !k.is_empty() => Ok(Some(k)),
            _ => Err(AppError::General(format!(
                "AI 密钥未配置（keyring 引用 {id} 无值）——请在设置 → AI 重新保存密钥"
            ))),
        },
    }
}

/// 流式对话：命令立即返回，token 经 Channel 推送；前端凭 reqId 可 `ai_cancel` 作废。
#[tauri::command]
pub async fn ai_chat_stream(
    req_id: String,
    cfg: AiCallConfig,
    messages: Vec<AiMessage>,
    on_event: Channel<AiStreamEvent>,
) -> DbResult<()> {
    let api_key = resolve_key(&cfg.key_ref)?;
    let req_id_inner = req_id.clone();
    let handle = tokio::spawn(async move {
        let result = client::stream_chat(&cfg, api_key, &messages, |delta| {
            let _ = on_event.send(AiStreamEvent::Token { delta });
        })
        .await;
        let event = match result {
            Ok(finish) => AiStreamEvent::Done { finish },
            Err(e) => AiStreamEvent::Error { message: e.to_string() },
        };
        let _ = on_event.send(event);
        tasks().lock().unwrap().remove(&req_id_inner);
    });
    tasks().lock().unwrap().insert(req_id, handle);
    Ok(())
}

/// 作废挂起流（不依赖 Promise 复位，直接 abort 任务）。
#[tauri::command]
pub async fn ai_cancel(req_id: String) -> bool {
    match tasks().lock().unwrap().remove(&req_id) {
        Some(h) => {
            h.abort();
            true
        }
        None => false,
    }
}

/// 设置页「测试连接」：非流式短探测，返回模型回显摘要。
#[tauri::command]
pub async fn ai_test_connection(cfg: AiCallConfig) -> DbResult<String> {
    let api_key = resolve_key(&cfg.key_ref)?;
    client::test_connection(&cfg, api_key).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_key_无引用_放行匿名() {
        assert!(resolve_key(&None).unwrap().is_none());
        assert!(resolve_key(&Some(String::new())).unwrap().is_none());
    }

    #[test]
    fn resolve_key_引用无值_报可操作错误() {
        let err = resolve_key(&Some("ai:not-exist".into())).unwrap_err();
        assert!(err.to_string().contains("密钥未配置"));
    }

    /// 端到端：本地 mock SSE 端点（std TcpListener 手写响应，分三次 write 模拟跨块），
    /// 验证 stream_chat 拼接增量、捕获 finish_reason、按 [DONE] 收尾。
    #[tokio::test]
    async fn mock_sse_端到端_流式拼接与完成() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = listener.local_addr().unwrap().port();
        std::thread::spawn(move || {
            use std::io::{BufRead, BufReader, Write};
            let (sock, _) = listener.accept().expect("accept");
            let mut reader = BufReader::new(sock);
            // 吞掉请求头（到空行）
            loop {
                let mut line = String::new();
                if reader.read_line(&mut line).unwrap() == 0 || line == "\r\n" || line == "\n" {
                    break;
                }
            }
            let w = reader.get_mut();
            w.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\n\r\n").unwrap();
            let ev = |c: &str| format!("data: {{\"choices\":[{{\"delta\":{{\"content\":\"{c}\"}}}}]}}\r\n\r\n");
            w.write_all(ev("Hel").as_bytes()).unwrap();
            w.write_all(ev("lo ").as_bytes()).unwrap();
            w.write_all(ev("世界").as_bytes()).unwrap();
            w.write_all(b"data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"stop\"}]}\r\n\r\n").unwrap();
            w.write_all(b"data: [DONE]\r\n\r\n").unwrap();
            w.flush().unwrap();
        });

        let cfg = AiCallConfig {
            base_url: format!("http://127.0.0.1:{port}/v1"),
            model: "mock-model".into(),
            key_ref: None,
            temperature: None,
            max_tokens: None,
            timeout_secs: Some(10),
        };
        let mut got = String::new();
        let finish = client::stream_chat(
            &cfg,
            None,
            &[AiMessage { role: "user".into(), content: "hi".into() }],
            |d| got.push_str(&d),
        )
        .await
        .expect("stream_chat");
        assert_eq!(got, "Hello 世界");
        assert_eq!(finish.as_deref(), Some("stop"));
    }
}
