//! 导出引擎：SQL（DDL + 方言 INSERT）/ CSV / Excel。
//! 文件路径由前端 save 对话框提供，Rust 端直接写文件。

use std::fs::File;
use std::io::{BufWriter, Write};
use std::time::Instant;

use serde::{Deserialize, Serialize};

use crate::error::{AppError, DbResult};

use super::dialect::DdlStrategy;
use super::pool::ConnectionManager;
use super::types::{CellValue, DbKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExportFormat {
    Sql,
    Csv,
    Excel,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportRequest {
    pub session_id: String,
    pub database: Option<String>,
    pub table: String,
    pub format: ExportFormat,
    /// 仅 Sql 格式有效
    pub include_ddl: bool,
    pub include_data: bool,
    pub where_clause: Option<String>,
    /// 0 = 使用保护上限
    pub max_rows: u32,
    /// 保存路径（前端 save 对话框返回）
    pub path: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportSummary {
    pub path: String,
    pub rows: u32,
    pub bytes: u64,
    pub duration_ms: u64,
}

/// 导出数据保护上限
const MAX_EXPORT_ROWS: u32 = 1_000_000;
/// INSERT 批大小（多值行数）
const INSERT_BATCH: usize = 500;

/// 计数写入器：BufWriter 流式落盘 + 字节数统计，
/// 避免大导出（百万行级）先把全部输出攒进内存再一次性写盘
struct CountingWriter<W: Write> {
    inner: BufWriter<W>,
    count: u64,
}

impl<W: Write> CountingWriter<W> {
    fn new(file: W) -> Self {
        Self {
            inner: BufWriter::with_capacity(256 * 1024, file),
            count: 0,
        }
    }

    fn flush(self) -> DbResult<u64> {
        self.inner
            .into_inner()
            .map_err(|e| AppError::General(format!("写入导出文件失败: {e}")))?;
        Ok(self.count)
    }
}

impl<W: Write> Write for CountingWriter<W> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let n = self.inner.write(buf)?;
        self.count += n as u64;
        Ok(n)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}

fn validate_where(w: &str) -> DbResult<()> {
    if w.contains(';') || w.contains("--") || w.contains("/*") {
        return Err(AppError::General(
            "WHERE 条件包含非法字符（分号或注释）".into(),
        ));
    }
    Ok(())
}

pub async fn run_export(
    state: &ConnectionManager,
    req: &ExportRequest,
) -> DbResult<ExportSummary> {
    let start = Instant::now();
    let session = state.get_session(&req.session_id)?;
    let driver = session.driver.clone();
    let dialect = driver.dialect();
    let db = req.database.as_deref().filter(|d| !d.is_empty());
    let max_rows = if req.max_rows == 0 {
        MAX_EXPORT_ROWS
    } else {
        req.max_rows.min(MAX_EXPORT_ROWS)
    };

    // 数据查询：方言生成预览语句；带 WHERE 时重建并按方言限制行数
    let mut data_sql = dialect.preview_rows_sql(db, &req.table, max_rows);
    if let Some(w) = req.where_clause.as_deref().map(str::trim).filter(|w| !w.is_empty()) {
        validate_where(w)?;
        let ident = match db {
            Some(s) => format!("{}.{}", dialect.quote_ident(s), dialect.quote_ident(&req.table)),
            None => dialect.quote_ident(&req.table),
        };
        let base = format!("SELECT * FROM {ident} WHERE {w}");
        data_sql = if session.config.kind == DbKind::Oracle {
            format!("SELECT * FROM ({base}) WHERE ROWNUM <= {max_rows}")
        } else {
            format!("{base} LIMIT {max_rows}")
        };
    }

    let result = driver.execute(db, &data_sql, max_rows).await?;

    // ---- 渲染 / 落盘（流式写入，边生成边落盘） ----
    let bytes: u64 = match req.format {
        ExportFormat::Sql => {
            let mut w = CountingWriter::new(File::create(&req.path)?);
            writeln!(w, "-- PersonalDb 导出")?;
            writeln!(
                w,
                "-- 表: {} | 时间: {}",
                req.table,
                chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
            )?;
            writeln!(w)?;
            write!(w, "{}", dialect.export_preamble())?;

            if req.include_ddl {
                let ddl = match dialect.table_ddl_strategy(db, &req.table) {
                    DdlStrategy::Query(q) => {
                        driver
                            .query_text(&q.sql, &q.params, q.value_index)
                            .await?
                            .trim()
                            .to_string()
                    }
                    DdlStrategy::FromColumns => {
                        let cols = driver.describe_table(db, &req.table).await?;
                        dialect.build_ddl_from_columns(db, &req.table, &cols)
                    }
                };
                if ddl.is_empty() {
                    return Err(AppError::General("获取表 DDL 失败".into()));
                }
                write!(w, "{ddl}")?;
                write!(w, "\n\n")?;
            }

            if req.include_data && !result.rows.is_empty() {
                for batch in result.rows.chunks(INSERT_BATCH) {
                    write!(
                        w,
                        "{}",
                        dialect.insert_statement(db, &req.table, &result.columns, batch)
                    )?;
                    writeln!(w)?;
                }
            }

            w.flush()?
        }
        ExportFormat::Csv => {
            let mut w = CountingWriter::new(File::create(&req.path)?);
            w.write_all(&[0xEF, 0xBB, 0xBF])?; // UTF-8 BOM，Excel 打开不乱码
            let header: Vec<String> = result.columns.iter().map(|c| c.name.clone()).collect();
            writeln!(w, "{}", csv_line(&header))?;
            for row in &result.rows {
                let cells: Vec<String> = row.iter().map(cell_to_csv).collect();
                writeln!(w, "{}", csv_line(&cells))?;
            }
            w.flush()?
        }
        ExportFormat::Excel => {
            let mut wb = rust_xlsxwriter::Workbook::new();
            let sheet = wb.add_worksheet();
            sheet
                .set_name(&req.table)
                .map_err(|e| AppError::General(format!("工作表命名失败: {e}")))?;
            let header: Vec<String> = result.columns.iter().map(|c| c.name.clone()).collect();
            sheet.write_row(0, 0, header.iter().map(|s| s.as_str()))?;
            for (r, row) in result.rows.iter().enumerate() {
                let row_idx = r as u32 + 1;
                for (c, v) in row.iter().enumerate() {
                    let col_idx = c as u16;
                    match v {
                        CellValue::Null => {
                            // 空单元格：跳过写入
                        }
                        CellValue::Bool(b) => {
                            sheet.write_boolean(row_idx, col_idx, *b)?;
                        }
                        CellValue::Int(i) => {
                            sheet.write_number(row_idx, col_idx, *i as f64)?;
                        }
                        CellValue::UInt(u) => {
                            sheet.write_number(row_idx, col_idx, *u as f64)?;
                        }
                        CellValue::Float(f) => {
                            sheet.write_number(row_idx, col_idx, *f)?;
                        }
                        CellValue::Text(s) => {
                            sheet.write_string(row_idx, col_idx, s)?;
                        }
                        CellValue::Binary(_) => {
                            sheet.write_string(row_idx, col_idx, "<binary>")?;
                        }
                    };
                }
            }
            wb.save(&req.path)
                .map_err(|e| AppError::General(format!("写入 Excel 失败: {e}")))?;
            std::fs::metadata(&req.path)?.len()
        }
    };

    Ok(ExportSummary {
        path: req.path.clone(),
        rows: result.rows.len() as u32,
        bytes,
        duration_ms: start.elapsed().as_millis() as u64,
    })
}

fn cell_to_csv(v: &CellValue) -> String {
    match v {
        CellValue::Null => String::new(),
        CellValue::Bool(b) => b.to_string(),
        CellValue::Int(i) => i.to_string(),
        CellValue::UInt(u) => u.to_string(),
        CellValue::Float(f) => f.to_string(),
        CellValue::Text(s) | CellValue::Binary(s) => s.clone(),
    }
}

/// CSV 公式注入防护（OWASP）：文本值以公式起始符开头时前缀 `'`，
/// 数值类型（Int/UInt/Float）不处理，保证数字可直接参与计算
fn csv_safe_cell(v: &CellValue) -> String {
    let s = cell_to_csv(v);
    if matches!(v, CellValue::Text(_) | CellValue::Binary(_))
        && (s.starts_with(['=', '+', '@', '\t', '\r'])
            || (s.starts_with('-')
                && s.len() > 1
                && !s[1..].chars().next().is_some_and(|c| c.is_ascii_digit())))
    {
        format!("'{s}")
    } else {
        s
    }
}

/// JSON 导出用：保持数值类型，Null → null，二进制为 base64 字符串
fn cell_to_json(v: &CellValue) -> serde_json::Value {
    match v {
        CellValue::Null => serde_json::Value::Null,
        CellValue::Bool(b) => serde_json::Value::Bool(*b),
        CellValue::Int(i) => serde_json::Value::from(*i),
        CellValue::UInt(u) => serde_json::Value::from(*u),
        CellValue::Float(f) => serde_json::Value::from(*f),
        CellValue::Text(s) | CellValue::Binary(s) => serde_json::Value::from(s.clone()),
    }
}

/// 结果集选中行导出格式
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RowsExportFormat {
    Sql,
    Csv,
    Excel,
    Txt,
    Json,
}

/// 导出列（名称 + 语义，SQL 显式日期转换用）
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportColumn {
    pub name: String,
    #[serde(default)]
    pub semantic: Option<String>,
}

/// 值转换规则：按列语义将值包裹为函数模板（{v} = 值的 SQL 字面量）
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValueRule {
    pub semantic: Option<String>,
    pub template: String,
}

/// SQL INSERT 导出选项
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SqlExportOptions {
    /// 去掉对象/字段名的引号
    #[serde(default)]
    pub unquote_ident: bool,
    /// 标识符大小写：original | upper | lower
    #[serde(default)]
    pub ident_case: Option<String>,
    /// 日期/时间按显示格式显式转换（TO_DATE/TO_TIMESTAMP 等）
    #[serde(default = "default_true")]
    pub date_convert: bool,
    /// 写入 UTF-8 BOM
    #[serde(default)]
    pub bom: bool,
    /// 每条 INSERT 语句合并的行数；1 = 每行单独一条 INSERT
    #[serde(default = "default_batch_size")]
    pub batch_size: usize,
    /// 值转换规则（TO_CLOB / NVL / COALESCE 等自定义模板）
    #[serde(default)]
    pub value_rules: Vec<ValueRule>,
}

fn default_true() -> bool {
    true
}

fn default_batch_size() -> usize {
    10
}

impl SqlExportOptions {
    fn default_opts() -> Self {
        Self {
            unquote_ident: false,
            ident_case: None,
            date_convert: true,
            bom: false,
            batch_size: default_batch_size(),
            value_rules: Vec::new(),
        }
    }

    fn effective_batch(&self) -> usize {
        self.batch_size.clamp(1, 1000)
    }
}

/// 选中行导出请求：列与行由前端直接提供（无需回库查询）
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RowsExportRequest {
    /// SQL INSERT 方言
    pub kind: DbKind,
    pub table: String,
    pub schema: Option<String>,
    /// 列（展示顺序、仅可见列）
    pub columns: Vec<ExportColumn>,
    /// 行数据（与 columns 对齐）
    pub rows: Vec<Vec<CellValue>>,
    pub format: RowsExportFormat,
    pub path: String,
    /// 仅 Sql 格式有效
    #[serde(default)]
    pub sql_options: Option<SqlExportOptions>,
}

/// 导出前端提供的行（选中行 / 过滤后结果）
pub fn export_rows(req: &RowsExportRequest) -> DbResult<ExportSummary> {
    let start = Instant::now();
    let dialect = super::dialect::build(req.kind)?;
    let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
    let col_names: Vec<String> = req.columns.iter().map(|c| c.name.clone()).collect();

    let bytes: u64 = match req.format {
        RowsExportFormat::Sql => {
            let opts = req.sql_options.clone().unwrap_or_else(SqlExportOptions::default_opts);
            let tbl_ident = format_ident(&req.table, &opts, dialect.as_ref());
            let tbl = match req.schema.as_deref().filter(|s| !s.is_empty()) {
                Some(s) => format!(
                    "{}.{tbl_ident}",
                    format_ident(s, &opts, dialect.as_ref())
                ),
                None => tbl_ident,
            };
            let col_list = req
                .columns
                .iter()
                .map(|c| format_ident(&c.name, &opts, dialect.as_ref()))
                .collect::<Vec<_>>()
                .join(", ");

            let mut w = CountingWriter::new(File::create(&req.path)?);
            if opts.bom {
                w.write_all(&[0xEF, 0xBB, 0xBF])?;
            }
            writeln!(
                w,
                "-- PersonalDb 导出（选中行） | 表: {} | 时间: {timestamp}",
                req.table
            )?;
            writeln!(w)?;
            write!(w, "{}", dialect.export_preamble())?;
            let batch = opts.effective_batch();
            for group in req.rows.chunks(batch) {
                let values = group
                    .iter()
                    .map(|row| {
                        let cells = row
                            .iter()
                            .zip(req.columns.iter())
                            .map(|(v, c)| {
                                value_literal(req.kind, dialect.as_ref(), c.semantic.as_deref(), v, &opts)
                            })
                            .collect::<Vec<_>>()
                            .join(", ");
                        format!("({cells})")
                    })
                    .collect::<Vec<_>>()
                    .join(",\n  ");
                write!(w, "INSERT INTO {tbl} ({col_list}) VALUES\n  {values};\n")?;
            }
            w.flush()?
        }
        RowsExportFormat::Csv => {
            let mut w = CountingWriter::new(File::create(&req.path)?);
            w.write_all(&[0xEF, 0xBB, 0xBF])?;
            writeln!(w, "{}", csv_line(&col_names))?;
            for row in &req.rows {
                let cells: Vec<String> = row.iter().map(csv_safe_cell).collect();
                writeln!(w, "{}", csv_line(&cells))?;
            }
            w.flush()?
        }
        RowsExportFormat::Txt => {
            let mut w = CountingWriter::new(File::create(&req.path)?);
            writeln!(w, "{}", col_names.join("\t"))?;
            for row in &req.rows {
                let cells: Vec<String> = row
                    .iter()
                    .map(|v| cell_to_csv(v).replace(['\r', '\n'], " "))
                    .collect();
                writeln!(w, "{}", cells.join("\t"))?;
            }
            w.flush()?
        }
        RowsExportFormat::Json => {
            // 对象数组：[{col: value, ...}, ...]；二进制保持 base64 字符串
            let mut arr = Vec::with_capacity(req.rows.len());
            for row in &req.rows {
                let mut obj = serde_json::Map::with_capacity(col_names.len());
                for (name, v) in col_names.iter().zip(row.iter()) {
                    obj.insert(name.clone(), cell_to_json(v));
                }
                arr.push(serde_json::Value::Object(obj));
            }
            let out = serde_json::to_vec_pretty(&arr)?;
            File::create(&req.path)?.write_all(&out)?;
            out.len() as u64
        }
        RowsExportFormat::Excel => {
            let mut wb = rust_xlsxwriter::Workbook::new();
            let sheet = wb.add_worksheet();
            sheet
                .set_name(&req.table)
                .map_err(|e| AppError::General(format!("工作表命名失败: {e}")))?;
            sheet.write_row(0, 0, col_names.iter().map(|s| s.as_str()))?;
            for (r, row) in req.rows.iter().enumerate() {
                let row_idx = r as u32 + 1;
                for (c, v) in row.iter().enumerate() {
                    let col_idx = c as u16;
                    match v {
                        CellValue::Null => {}
                        CellValue::Bool(b) => {
                            sheet.write_boolean(row_idx, col_idx, *b)?;
                        }
                        CellValue::Int(i) => {
                            sheet.write_number(row_idx, col_idx, *i as f64)?;
                        }
                        CellValue::UInt(u) => {
                            sheet.write_number(row_idx, col_idx, *u as f64)?;
                        }
                        CellValue::Float(f) => {
                            sheet.write_number(row_idx, col_idx, *f)?;
                        }
                        CellValue::Text(s) => {
                            sheet.write_string(row_idx, col_idx, s)?;
                        }
                        CellValue::Binary(_) => {
                            sheet.write_string(row_idx, col_idx, "<binary>")?;
                        }
                    };
                }
            }
            wb.save(&req.path)
                .map_err(|e| AppError::General(format!("写入 Excel 失败: {e}")))?;
            std::fs::metadata(&req.path)?.len()
        }
    };

    Ok(ExportSummary {
        path: req.path.clone(),
        rows: req.rows.len() as u32,
        bytes,
        duration_ms: start.elapsed().as_millis() as u64,
    })
}

// ---- SQL INSERT 生成辅助（标识符格式化 + 日期显式转换） ----

fn format_ident(name: &str, opts: &SqlExportOptions, dialect: &dyn super::dialect::SqlDialect) -> String {
    let mut s = name.to_string();
    match opts.ident_case.as_deref() {
        Some("upper") => s = s.to_uppercase(),
        Some("lower") => s = s.to_lowercase(),
        _ => {}
    }
    if opts.unquote_ident {
        s
    } else {
        dialect.quote_ident(&s)
    }
}

fn parse_sem(s: &str) -> crate::db::types::SemanticType {
    crate::db::types::SemanticType::from_label(s, crate::db::types::SemanticType::Text)
}

fn value_literal(
    kind: DbKind,
    dialect: &dyn super::dialect::SqlDialect,
    semantic: Option<&str>,
    v: &CellValue,
    opts: &SqlExportOptions,
) -> String {
    let sem = semantic.map(parse_sem);
    if opts.date_convert
        && let (Some(sem @ (crate::db::types::SemanticType::Date
        | crate::db::types::SemanticType::DateTime
        | crate::db::types::SemanticType::Time)), CellValue::Text(s)) = (sem, v)
            && let Some(lit) = explicit_date_literal(kind, sem, s) {
                return lit;
            }
    // 用户值转换规则（按语义匹配，仅对非空值）
    if let Some(sem) = sem
        && !matches!(v, CellValue::Null)
            && let Some(rule) = opts
                .value_rules
                .iter()
                .find(|r| r.semantic.as_deref().map(parse_sem) == Some(sem))
                && !rule.template.trim().is_empty() {
                    let lit = dialect.literal(v, sem);
                    return rule.template.replace("{v}", &lit);
                }
    dialect.literal(v, sem.unwrap_or(crate::db::types::SemanticType::Text))
}

/// 依据值的显示形态推断格式，并按方言生成显式日期/时间字面量
fn explicit_date_literal(kind: DbKind, sem: crate::db::types::SemanticType, text: &str) -> Option<String> {
    use crate::db::types::SemanticType as S;
    let t = text.trim();
    if t.is_empty() {
        return None;
    }
    let (norm, fmt) = infer_format(sem, t)?;
    let quoted = format!("'{norm}'");
    match kind {
        DbKind::Oracle | DbKind::OceanBaseOracle => Some(match sem {
            S::DateTime => format!("TO_TIMESTAMP({quoted}, '{fmt}')"),
            _ => format!("TO_DATE({quoted}, '{fmt}')"),
        }),
        DbKind::PostgreSql | DbKind::GaussDb => Some(match sem {
            S::DateTime => format!("TO_TIMESTAMP({quoted}, '{fmt}')"),
            _ => format!("TO_DATE({quoted}, '{fmt}')"),
        }),
        DbKind::MySql | DbKind::OceanBase => {
            Some(format!("STR_TO_DATE({quoted}, '{}')", mysql_fmt(&fmt)))
        }
        _ => Some(quoted),
    }
}

/// 从值的字符串形态推断（规范化值, Oracle 风格格模式）
fn infer_format(sem: crate::db::types::SemanticType, t: &str) -> Option<(String, String)> {
    use crate::db::types::SemanticType as S;
    let digits = !t.is_empty() && t.chars().all(|c| c.is_ascii_digit());
    match sem {
        S::Date => {
            if t.len() == 8 && digits {
                Some((t.into(), "YYYYMMDD".into()))
            } else if t.len() == 10 && t.as_bytes()[4] == b'-' && t.as_bytes()[7] == b'-' {
                Some((t.into(), "YYYY-MM-DD".into()))
            } else if t.len() == 10 && t.as_bytes()[4] == b'/' && t.as_bytes()[7] == b'/' {
                Some((t.into(), "YYYY/MM/DD".into()))
            } else {
                None
            }
        }
        S::DateTime => {
            if t.len() == 14 && digits {
                Some((t.into(), "YYYYMMDDHH24MISS".into()))
            } else {
                let (d, tm) = t.split_once(' ').or_else(|| t.split_once('T'))?;
                let df = date_part_fmt(d)?;
                let tf = time_part_fmt(tm)?;
                Some((format!("{d} {tm}"), format!("{df} {tf}")))
            }
        }
        S::Time => {
            let f = time_part_fmt(t)?;
            Some((t.into(), f))
        }
        _ => None,
    }
}

fn date_part_fmt(d: &str) -> Option<String> {
    if d.len() == 10 && d.as_bytes()[4] == b'-' && d.as_bytes()[7] == b'-' {
        Some("YYYY-MM-DD".into())
    } else if d.len() == 8 && d.chars().all(|c| c.is_ascii_digit()) {
        Some("YYYYMMDD".into())
    } else {
        None
    }
}

fn time_part_fmt(tm: &str) -> Option<String> {
    let (base, frac) = match tm.split_once('.') {
        Some((b, f)) => (b, Some(f)),
        None => (tm, None),
    };
    let parts: Vec<&str> = base.split(':').collect();
    if parts.len() != 3
        || !parts
            .iter()
            .all(|p| p.len() == 2 && p.chars().all(|c| c.is_ascii_digit()))
    {
        return None;
    }
    let mut fmt = "HH24:MI:SS".to_string();
    if let Some(f) = frac {
        if !f.is_empty() && f.chars().all(|c| c.is_ascii_digit()) {
            fmt = format!("{fmt}.FF");
        } else {
            return None;
        }
    }
    Some(fmt)
}

/// Oracle 风格格式 → MySQL STR_TO_DATE 格式
fn mysql_fmt(oracle_fmt: &str) -> String {
    oracle_fmt
        .replace("YYYY", "%Y")
        .replace("HH24", "%H")
        .replace("HH", "%H")
        .replace("MM", "%m")
        .replace("DD", "%d")
        .replace("MI", "%i")
        .replace("SS", "%s")
        .replace("FF", "%f")
}

/// RFC 4180 CSV 行（含引号转义）
fn csv_line(cells: &[String]) -> String {
    cells
        .iter()
        .map(|c| {
            if c.contains(',') || c.contains('"') || c.contains('\n') || c.contains('\r') {
                format!("\"{}\"", c.replace('"', "\"\""))
            } else {
                c.clone()
            }
        })
        .collect::<Vec<_>>()
        .join(",")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cell_to_json_types() {
        assert_eq!(cell_to_json(&CellValue::Null), serde_json::Value::Null);
        assert_eq!(cell_to_json(&CellValue::Bool(true)), serde_json::Value::Bool(true));
        assert_eq!(cell_to_json(&CellValue::Int(-5)), serde_json::json!(-5));
        assert_eq!(cell_to_json(&CellValue::UInt(7)), serde_json::json!(7));
        assert_eq!(cell_to_json(&CellValue::Float(1.5)), serde_json::json!(1.5));
        assert_eq!(cell_to_json(&CellValue::Text("中".into())), serde_json::json!("中"));
        // 二进制保持 base64 字符串（不解码）
        assert_eq!(cell_to_json(&CellValue::Binary("AQID".into())), serde_json::json!("AQID"));
    }

    #[test]
    fn csv_line_escapes() {
        assert_eq!(csv_line(&["a".into(), "b".into()]), "a,b");
        assert_eq!(csv_line(&["a,b".into(), "c\"d".into()]), "\"a,b\",\"c\"\"d\"");
        assert_eq!(csv_line(&["x\ny".into()]), "\"x\ny\"");
    }

    #[test]
    fn csv_injection_guard() {
        // 公式起始符 → 前缀 '
        assert_eq!(csv_safe_cell(&CellValue::Text("=cmd".into())), "'=cmd");
        assert_eq!(csv_safe_cell(&CellValue::Text("+1".into())), "'+1");
        assert_eq!(csv_safe_cell(&CellValue::Text("@sum".into())), "'@sum");
        assert_eq!(csv_safe_cell(&CellValue::Text("-cmd".into())), "'-cmd");
        // 数值类型不做防护（保证可计算）
        assert_eq!(csv_safe_cell(&CellValue::Int(-42)), "-42");
        assert_eq!(csv_safe_cell(&CellValue::Float(-1.5)), "-1.5");
        // 普通文本不动；"-数字"开头的文本按负数语义放行（非公式）
        assert_eq!(csv_safe_cell(&CellValue::Text("hello".into())), "hello");
        assert_eq!(csv_safe_cell(&CellValue::Text("-1.5abc".into())), "-1.5abc");
        // 空串/NULL 不动
        assert_eq!(csv_safe_cell(&CellValue::Text(String::new())), "");
        assert_eq!(csv_safe_cell(&CellValue::Null), "");
    }
}
