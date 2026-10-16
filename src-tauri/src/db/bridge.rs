//! Oracle / OceanBase-Oracle 桥接：与 sidecar 进程经 NDJSON over stdio 通信。
//!
//! 两类 sidecar：
//! - `OracleGo`：`oracle-bridge`（Go + go-ora，纯 Go Oracle 协议）→ 连接真实 Oracle，免 OCI
//! - `ObJdbc`：`ob-jdbc-bridge.jar`（Java + OceanBase Connector/J）→ 连接 OceanBase Oracle 模式
//!   （OceanBase 的 Oracle 模式本质是 MySQL 协议 + OB 扩展能力位，标准 Oracle TNS 客户端无法连接）
//!
//! 一个桥接进程承载一个数据库连接（与"连接 = 会话"模型一致）。

use std::path::PathBuf;
use std::process::Stdio;
use std::sync::OnceLock;

use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Lines};
use tokio::process::{Child, ChildStdin, ChildStdout};

use crate::error::{AppError, DbResult};

/// 打包后资源目录（lib.rs setup 注入）
static RESOURCE_DIR: OnceLock<PathBuf> = OnceLock::new();

pub fn set_resource_dir(path: PathBuf) {
    let _ = RESOURCE_DIR.set(path);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BridgeKind {
    /// 真实 Oracle：Go + go-ora（免 OCI）
    OracleGo,
    /// OceanBase Oracle 模式：Java + OceanBase Connector/J
    ObJdbc,
}

fn oracle_exe_name() -> &'static str {
    if cfg!(windows) {
        "oracle-bridge.exe"
    } else {
        "oracle-bridge"
    }
}

const JDBC_BRIDGE_JAR: &str = "jdbc-bridge.jar";

/// 按优先级返回候选路径：同目录 → 资源目录 → 资源/binaries → 开发模式 binaries
fn candidates(name: &str) -> Vec<PathBuf> {
    let mut v = Vec::new();
    if let Ok(exe) = std::env::current_exe()
        && let Some(dir) = exe.parent() {
            v.push(dir.join(name));
        }
    if let Some(res) = RESOURCE_DIR.get() {
        v.push(res.join(name));
        v.push(res.join("binaries").join(name));
    }
    #[cfg(debug_assertions)]
    {
        v.push(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("binaries")
                .join(name),
        );
    }
    v
}

fn which(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let p = dir.join(name);
        if p.is_file() {
            return Some(p);
        }
        #[cfg(windows)]
        {
            let pe = dir.join(format!("{name}.exe"));
            if pe.is_file() {
                return Some(pe);
            }
        }
    }
    None
}

/// oracle-bridge 可执行文件路径
pub fn oracle_go_path() -> Option<PathBuf> {
    if let Ok(custom) = std::env::var("PERSONALDB_ORACLE_BRIDGE") {
        let p = PathBuf::from(custom);
        if p.is_file() {
            return Some(p);
        }
    }
    candidates(oracle_exe_name())
        .into_iter()
        .find(|p| p.is_file())
}

/// jdbc-bridge jar 路径
pub fn jdbc_bridge_jar_path() -> Option<PathBuf> {
    if let Ok(custom) = std::env::var("PERSONALDB_JDBC_BRIDGE") {
        let p = PathBuf::from(custom);
        if p.is_file() {
            return Some(p);
        }
    }
    candidates(JDBC_BRIDGE_JAR).into_iter().find(|p| p.is_file())
}

/// java 可执行文件：JAVA_HOME → PATH
pub fn java_exe() -> Option<PathBuf> {
    if let Ok(home) = std::env::var("JAVA_HOME") {
        let bin = if cfg!(windows) { "java.exe" } else { "java" };
        let p = PathBuf::from(home).join("bin").join(bin);
        if p.is_file() {
            return Some(p);
        }
    }
    which("java")
}

/// 指定桥接是否可用
pub fn available(kind: BridgeKind) -> bool {
    match kind {
        BridgeKind::OracleGo => oracle_go_path().is_some(),
        BridgeKind::ObJdbc => jdbc_bridge_jar_path().is_some() && java_exe().is_some(),
    }
}

/// 去除 Windows verbatim 前缀 `\\?\`（Rust 的 current_exe/resource_dir 会带此前缀，
/// 但作为命令行参数传给外部程序（如 java -jar）时不被识别）
fn clean_path(p: PathBuf) -> PathBuf {
    #[cfg(windows)]
    {
        let s = p.to_string_lossy();
        if let Some(rest) = s.strip_prefix(r"\\?\") {
            if let Some(unc) = rest.strip_prefix("UNC\\") {
                return PathBuf::from(format!(r"\\{unc}"));
            }
            return PathBuf::from(rest);
        }
    }
    p
}

/// 启动指定桥接进程（extra_jars 为附加驱动 jar，仅 Jdbc 使用）
pub async fn spawn(kind: BridgeKind, extra_jars: &[String]) -> DbResult<Bridge> {
    match kind {
        BridgeKind::OracleGo => {
            let path = clean_path(oracle_go_path().ok_or_else(|| {
                AppError::General(
                    "未找到 oracle-bridge（内置 Oracle 驱动）；请改用 OCI 方式或重新构建".into(),
                )
            })?);
            Bridge::spawn_cmd(tokio::process::Command::new(path), "oracle-bridge").await
        }
        BridgeKind::ObJdbc => {
            let jar = clean_path(jdbc_bridge_jar_path().ok_or_else(|| {
                AppError::General("未找到 jdbc-bridge.jar（JDBC 驱动）；请重新构建".into())
            })?);
            let java = clean_path(java_exe().ok_or_else(|| {
                AppError::General(
                    "未找到 Java 运行时（JRE 17+）；请安装 Java 并配置 JAVA_HOME 或 PATH".into(),
                )
            })?);
            let sep = if cfg!(windows) { ";" } else { ":" };
            let mut cp = jar.display().to_string();
            for j in extra_jars {
                if j.trim().is_empty() {
                    continue;
                }
                cp.push_str(sep);
                cp.push_str(&clean_path(PathBuf::from(j)).display().to_string());
            }
            log::info!("jdbc-bridge: java={} classpath={}", java.display(), cp);
            let mut cmd = tokio::process::Command::new(&java);
            cmd.arg("-cp").arg(&cp).arg("personaldb.JdbcBridge");
            Bridge::spawn_cmd(cmd, "jdbc-bridge").await
        }
    }
}

/// 扫描指定 jar 中注册的 JDBC 驱动类（ServiceLoader）
pub async fn scan_driver_classes(jars: &[String]) -> DbResult<Vec<String>> {
    let mut b = spawn(BridgeKind::ObJdbc, jars).await?;
    let res = b.call(serde_json::json!({ "cmd": "scanDriverClasses" })).await;
    b.shutdown().await;
    let data = res?;
    Ok(data
        .get("classes")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default())
}

/// NDJSON 桥接进程句柄
pub struct Bridge {
    child: Child,
    stdin: ChildStdin,
    lines: Lines<BufReader<ChildStdout>>,
    next_id: u64,
}

impl Bridge {
    async fn spawn_cmd(mut cmd: tokio::process::Command, label: &str) -> DbResult<Self> {
        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        let mut child = cmd
            .spawn()
            .map_err(|e| AppError::General(format!("启动数据库桥接进程失败：{e}")))?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| AppError::General("桥接进程 stdin 不可用".into()))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| AppError::General("桥接进程 stdout 不可用".into()))?;
        // 转发 sidecar 的 stderr 到日志（诊断用）
        if let Some(stderr) = child.stderr.take() {
            let label = label.to_string();
            tokio::spawn(async move {
                let mut lines = BufReader::new(stderr).lines();
                while let Ok(Some(l)) = lines.next_line().await {
                    if !l.trim().is_empty() {
                        log::warn!("[{label}] {l}");
                        eprintln!("[{label}] {l}");
                    }
                }
            });
        }
        Ok(Self {
            child,
            stdin,
            lines: BufReader::new(stdout).lines(),
            next_id: 1,
        })
    }

    /// 发送命令并等待响应（顺序模型；120s 超时）
    pub async fn call(&mut self, mut req: Value) -> DbResult<Value> {
        let id = self.next_id;
        self.next_id += 1;
        req["id"] = Value::from(id);
        let mut line = serde_json::to_string(&req)?;
        line.push('\n');
        self.stdin
            .write_all(line.as_bytes())
            .await
            .map_err(|e| AppError::General(format!("写入桥接进程失败：{e}")))?;
        self.stdin
            .flush()
            .await
            .map_err(|e| AppError::General(format!("写入桥接进程失败：{e}")))?;

        let read = async {
            while let Some(l) = self
                .lines
                .next_line()
                .await
                .map_err(|e| AppError::General(format!("读取桥接进程失败：{e}")))?
            {
                if l.trim().is_empty() {
                    continue;
                }
                let v: Value = serde_json::from_str(&l)
                    .map_err(|e| AppError::General(format!("桥接进程响应解析失败：{e}")))?;
                if v.get("id").and_then(Value::as_u64) != Some(id) {
                    continue;
                }
                if v.get("ok").and_then(Value::as_bool) == Some(true) {
                    return Ok(v.get("data").cloned().unwrap_or(Value::Null));
                }
                let msg = v
                    .get("error")
                    .and_then(Value::as_str)
                    .unwrap_or("桥接进程执行失败")
                    .to_string();
                return Err(AppError::Db(msg));
            }
            // stdout 关闭：进程已退出，附带退出码便于诊断
            let code = self.child.try_wait().ok().flatten().and_then(|s| s.code());
            Err(AppError::Db(match code {
                Some(c) => format!("桥接进程已退出（退出码 {c}）"),
                None => "桥接进程已退出（可能是客户端库或连接异常）".into(),
            }))
        };

        tokio::time::timeout(std::time::Duration::from_secs(120), read)
            .await
            .map_err(|_| AppError::Db("桥接进程响应超时（120 秒）".into()))?
    }

    pub async fn shutdown(&mut self) {
        let _ = self.call(serde_json::json!({ "cmd": "shutdown" })).await;
        let _ = self.child.kill().await;
    }
}

impl Drop for Bridge {
    fn drop(&mut self) {
        let _ = self.child.start_kill();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn ping_roundtrip(kind: BridgeKind) {
        if !available(kind) {
            eprintln!("skip: {kind:?} 不可用");
            return;
        }
        let mut b = spawn(kind, &[]).await.expect("spawn bridge");
        let data = b
            .call(serde_json::json!({ "cmd": "ping" }))
            .await
            .expect("ping");
        assert_eq!(data.get("pong").and_then(Value::as_bool), Some(true));
        b.shutdown().await;
    }

    #[tokio::test]
    async fn oracle_go_ping_roundtrip() {
        ping_roundtrip(BridgeKind::OracleGo).await;
    }

    #[tokio::test]
    async fn ob_jdbc_ping_roundtrip() {
        ping_roundtrip(BridgeKind::ObJdbc).await;
    }

    #[tokio::test]
    #[ignore]
    async fn ob_jdbc_live_connect() {
        let Ok(password) = std::env::var("OB_TEST_PASSWORD") else {
            eprintln!("skip: 未设置 OB_TEST_PASSWORD");
            return;
        };
        let host = std::env::var("OB_TEST_HOST").unwrap_or_else(|_| "127.0.0.1".into());
        let mut b = spawn(BridgeKind::ObJdbc, &[]).await.expect("spawn");
        let r = b
            .call(serde_json::json!({
                "cmd": "connect", "host": host, "port": 2881,
                "service": "", "user": "cs_platform@cscioracle", "password": password
            }))
            .await;
        eprintln!("connect result: {r:?}");
        b.shutdown().await;
    }

    #[tokio::test]
    async fn bridge_unknown_command_errors() {
        if !available(BridgeKind::OracleGo) {
            eprintln!("skip: OracleGo 不可用");
            return;
        }
        let mut b = spawn(BridgeKind::OracleGo, &[]).await.expect("spawn bridge");
        let err = b
            .call(serde_json::json!({ "cmd": "nope" }))
            .await
            .expect_err("应返回错误");
        assert!(matches!(err, AppError::Db(_)));
        b.shutdown().await;
    }
}
