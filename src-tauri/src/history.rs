//! 查询历史持久化（SQLite，存储目录体系 history/history.db）。
//!
//! 上限 500 条（插入后按 at 修剪），单条 SQL 截断 2000 字符；写入失败静默（不影响主流程）。

use std::sync::{Mutex, PoisonError};

use rusqlite::Connection;

use crate::error::{AppError, DbResult};
use crate::storage::{self, StorageCategory};

pub struct HistoryStore {
    conn: Mutex<Connection>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryItem {
    pub id: String,
    pub connection_name: String,
    pub kind: String,
    pub database: String,
    pub sql: String,
    pub duration_ms: i64,
    pub ok: bool,
    /// epoch ms
    pub at: i64,
}

const MAX_ROWS: i64 = 500;
const SQL_MAX: usize = 2000;

fn init_schema(conn: &Connection) -> DbResult<()> {
    conn.execute_batch(
        "PRAGMA journal_mode=WAL;
         CREATE TABLE IF NOT EXISTS history (
            id TEXT PRIMARY KEY,
            connection_name TEXT NOT NULL,
            kind TEXT NOT NULL,
            database TEXT NOT NULL DEFAULT '',
            sql TEXT NOT NULL,
            duration_ms INTEGER NOT NULL DEFAULT 0,
            ok INTEGER NOT NULL DEFAULT 1,
            at INTEGER NOT NULL
         );
         CREATE INDEX IF NOT EXISTS idx_history_at ON history(at DESC);",
    )
    .map_err(|e| AppError::General(format!("初始化历史库失败: {e}")))?;
    Ok(())
}

fn open_db() -> DbResult<Connection> {
    let path = storage::file_path(StorageCategory::History, "history.db")?;
    let conn = Connection::open(&path)
        .map_err(|e| AppError::General(format!("打开历史库失败: {e}")))?;
    init_schema(&conn)?;
    Ok(conn)
}

impl HistoryStore {
    pub fn new() -> DbResult<Self> {
        Ok(Self { conn: Mutex::new(open_db()?) })
    }

    /// 文件库不可用时的内存兜底（应用生命周期内有效，不落盘）
    pub fn in_memory() -> DbResult<Self> {
        let conn = Connection::open_in_memory()
            .map_err(|e| AppError::General(format!("内存历史库失败: {e}")))?;
        init_schema(&conn)?;
        Ok(Self { conn: Mutex::new(conn) })
    }

    pub fn add(&self, mut item: HistoryItem) {
        item.sql.truncate(SQL_MAX);
        let Ok(conn) = self.conn.lock() else { return };
        let _ = conn.execute(
            "INSERT OR REPLACE INTO history(id, connection_name, kind, database, sql, duration_ms, ok, at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            rusqlite::params![
                item.id,
                item.connection_name,
                item.kind,
                item.database,
                item.sql,
                item.duration_ms,
                item.ok as i64,
                item.at
            ],
        );
        // 修剪超出上限的最旧记录
        let _ = conn.execute(
            "DELETE FROM history WHERE id IN (
                SELECT id FROM history ORDER BY at DESC LIMIT -1 OFFSET ?1
             )",
            [MAX_ROWS],
        );
    }

    pub fn list(&self, keyword: Option<&str>, limit: i64) -> DbResult<Vec<HistoryItem>> {
        let conn = self.conn.lock().unwrap_or_else(PoisonError::into_inner);
        let kw_raw = keyword.unwrap_or("").trim().to_string();
        let like = format!("%{}%", kw_raw.replace('%', ""));
        let mut stmt = conn
            .prepare(
                "SELECT id, connection_name, kind, database, sql, duration_ms, ok, at
                 FROM history
                 WHERE (?1 = '' OR sql LIKE ?2 OR connection_name LIKE ?2 OR database LIKE ?2)
                 ORDER BY at DESC LIMIT ?3",
            )
            .map_err(|e| AppError::General(format!("查询历史失败: {e}")))?;
        let rows = stmt
            .query_map(rusqlite::params![kw_raw, like, limit], |r| {
                Ok(HistoryItem {
                    id: r.get(0)?,
                    connection_name: r.get(1)?,
                    kind: r.get(2)?,
                    database: r.get(3)?,
                    sql: r.get(4)?,
                    duration_ms: r.get(5)?,
                    ok: r.get::<_, i64>(6)? != 0,
                    at: r.get(7)?,
                })
            })
            .map_err(|e| AppError::General(format!("查询历史失败: {e}")))?;
        Ok(rows.flatten().collect())
    }

    pub fn clear(&self) -> DbResult<()> {
        let conn = self.conn.lock().unwrap_or_else(PoisonError::into_inner);
        conn.execute("DELETE FROM history", [])
            .map_err(|e| AppError::General(format!("清空历史失败: {e}")))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_list_clear_roundtrip() {
        let store = HistoryStore::new().expect("open");
        store.clear().ok();
        store.add(HistoryItem {
            id: "t1".into(),
            connection_name: "c".into(),
            kind: "mysql".into(),
            database: "db".into(),
            sql: "SELECT * FROM t WHERE x LIKE '%abc%'".into(),
            duration_ms: 12,
            ok: true,
            at: 1_700_000_000_000,
        });
        let all = store.list(None, 100).expect("list");
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].sql, "SELECT * FROM t WHERE x LIKE '%abc%'");
        let hit = store.list(Some("abc"), 100).expect("search");
        assert_eq!(hit.len(), 1);
        let miss = store.list(Some("zzz"), 100).expect("search2");
        assert!(miss.is_empty());
        store.clear().expect("clear");
        assert!(store.list(None, 100).expect("list2").is_empty());
    }
}
