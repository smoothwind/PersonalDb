//! 对象目录操作：组合方言 SQL 与驱动执行（P1 对象浏览器核心逻辑）。

use crate::error::{AppError, DbResult};

use super::driver::DatabaseDriver;
use super::objects::{DbObjectType, ObjectFolder, ObjectInfo, SchemaSnapshot};

/// 库/schema 下的对象类型文件夹（数量为 0 的类型不返回）
pub async fn list_folders(
    driver: &dyn DatabaseDriver,
    database: Option<&str>,
) -> DbResult<Vec<ObjectFolder>> {
    let dialect = driver.dialect();
    let mut out = Vec::new();
    for obj in dialect.capabilities().object_types {
        let Some(q) = dialect.catalog_count_query(obj, database) else {
            continue;
        };
        let count = driver.run_catalog_count(&q, database).await?;
        if count > 0 {
            out.push(ObjectFolder {
                object_type: obj,
                count,
            });
        }
    }
    Ok(out)
}

/// 某一对象类型下的实例列表
pub async fn list_objects(
    driver: &dyn DatabaseDriver,
    object_type: DbObjectType,
    database: Option<&str>,
) -> DbResult<Vec<ObjectInfo>> {
    let q = driver
        .dialect()
        .catalog_query(object_type, database)
        .ok_or_else(|| {
            AppError::General(format!(
                "该数据库不支持对象类型「{}」",
                object_type.display_name()
            ))
        })?;
    driver.run_catalog(object_type, &q, database).await
}

/// 元数据快照（用于 SQL 补全）：优先方言 SQL，其次 JDBC 元数据回退
pub async fn schema_snapshot(
    driver: &dyn DatabaseDriver,
    database: Option<&str>,
) -> DbResult<SchemaSnapshot> {
    use super::objects::{ColumnSnap, SchemaSnapshot, TableMeta};
    use super::types::CellValue;

    let text = |row: &[CellValue], i: usize| -> Option<String> {
        match row.get(i) {
            Some(CellValue::Text(s)) | Some(CellValue::Binary(s)) => Some(s.clone()),
            Some(CellValue::Int(n)) => Some(n.to_string()),
            Some(CellValue::UInt(n)) => Some(n.to_string()),
            _ => None,
        }
    };

    if let Some(sql) = driver.dialect().schema_snapshot_sql(database) {
        let res = driver.execute(database, &sql, 200_000).await?;
        let mut tables: Vec<TableMeta> = Vec::new();
        let mut index: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        for row in &res.rows {
            let name = text(row, 0).unwrap_or_default();
            if name.is_empty() {
                continue;
            }
            let pos = *index.entry(name.clone()).or_insert_with(|| {
                tables.push(TableMeta {
                    name: name.clone(),
                    kind: text(row, 1).unwrap_or_else(|| "TABLE".into()),
                    comment: text(row, 4).filter(|s| !s.is_empty()),
                    columns: Vec::new(),
                });
                tables.len() - 1
            });
            if let Some(col) = text(row, 2).filter(|s| !s.is_empty()) {
                tables[pos].columns.push(ColumnSnap {
                    name: col,
                    data_type: text(row, 3).unwrap_or_default(),
                    comment: text(row, 5).filter(|s| !s.is_empty()),
                });
            }
        }
        return Ok(SchemaSnapshot { tables });
    }

    // 元数据回退（通用 JDBC）：表 + 逐表列（上限 500 张以控成本）
    let tables = driver.list_tables(database).await?;
    let mut out = SchemaSnapshot { tables: Vec::new() };
    for (i, t) in tables.into_iter().enumerate() {
        if i >= 500 {
            break;
        }
        let cols = driver.describe_table(database, &t.name).await.unwrap_or_default();
        out.tables.push(TableMeta {
            name: t.name,
            kind: t.table_type,
            comment: t.comment,
            columns: cols
                .into_iter()
                .map(|c| ColumnSnap {
                    name: c.name,
                    data_type: c.data_type,
                    comment: c.comment,
                })
                .collect(),
        });
    }
    Ok(out)
}
