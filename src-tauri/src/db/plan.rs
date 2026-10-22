//! 执行计划：结构化节点 + MySQL / PostgreSQL JSON 计划解析器 + SQLite 行式计划。

use serde::Serialize;
use serde_json::Value;

use crate::error::{AppError, DbResult};
use crate::db::types::{CellValue, QueryResult};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanNode {
    /// 节点标题（表名 / 操作类型）
    pub title: String,
    /// 属性键值对（访问方式、成本、行数等）
    pub attrs: Vec<(String, String)>,
    pub cost: Option<f64>,
    pub rows: Option<u64>,
    pub children: Vec<PlanNode>,
}

impl PlanNode {
    fn leaf(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            attrs: Vec::new(),
            cost: None,
            rows: None,
            children: Vec::new(),
        }
    }
}

/// MySQL EXPLAIN FORMAT=JSON → 结构化树
pub fn parse_mysql(text: &str) -> DbResult<PlanNode> {
    let v: Value = serde_json::from_str(text)
        .map_err(|e| AppError::General(format!("执行计划 JSON 解析失败: {e}")))?;
    let mut root = PlanNode::leaf("查询");
    if let Some(qb) = v.get("query_block") {
        for child in parse_mysql_node(qb) {
            root.children.push(child);
        }
        if let Some(cost) = qb
            .get("cost_info")
            .and_then(|c| c.get("query_cost"))
            .and_then(Value::as_str)
            .and_then(|s| s.parse::<f64>().ok())
        {
            root.cost = Some(cost);
        }
    } else {
        // 结构不识别时降级：原样文本展示
        root.attrs.push(("原始".into(), text.to_string()));
    }
    Ok(root)
}

/// 递归解析 MySQL 计划节点，返回该层产生的节点列表
fn parse_mysql_node(obj: &Value) -> Vec<PlanNode> {
    let Value::Object(map) = obj else {
        return vec![];
    };
    let mut out: Vec<PlanNode> = Vec::new();

    // 表访问节点
    if let Some(t) = map.get("table") {
        let mut node = PlanNode::leaf(
            t.get("table_name")
                .and_then(Value::as_str)
                .unwrap_or("table")
                .to_string(),
        );
        for key in [
            "access_type",
            "possible_keys",
            "key",
            "ref",
            "rows_examined_per_scan",
            "rows_produced_per_join",
            "filtered",
            "using_index",
            "using_where",
            "using_temporary",
            "using_filesort",
        ] {
            if let Some(v) = t.get(key) {
                let text = match v {
                    Value::String(s) => s.clone(),
                    other => other.to_string(),
                };
                if let Some(r) = key
                    .starts_with("rows")
                    .then(|| v.as_f64())
                    .flatten()
                {
                    node.rows = Some(r as u64);
                }
                if key == "rows_examined_per_scan" {
                    node.attrs.push(("预估行数".into(), text));
                } else {
                    node.attrs.push((key.to_string(), text));
                }
            }
        }
        if let Some(cost) = t
            .get("cost_info")
            .and_then(|c| c.get("read_cost"))
            .and_then(Value::as_str)
            .and_then(|s| s.parse::<f64>().ok())
        {
            node.cost = Some(cost);
        }
        out.push(node);
    }

    // 组合 / 操作节点
    for key in [
        "grouping_operation",
        "ordering_operation",
        "duplicating_weedout",
        "materialized_from_subquery",
        "query_specification",
    ] {
        if let Some(child) = map.get(key) {
            let mut node = PlanNode::leaf(op_label(key));
            for sub in parse_mysql_node(child) {
                node.children.push(sub);
            }
            out.push(node);
        }
    }

    // nested_loop: { nested_loop: [ {table...}, {grouping_operation...} ] }
    if let Some(Value::Array(branches)) = map.get("nested_loop") {
        let mut node = PlanNode::leaf("嵌套循环连接");
        for b in branches {
            for sub in parse_mysql_node(b) {
                node.children.push(sub);
            }
        }
        out.push(node);
    }

    out
}

fn op_label(key: &str) -> &'static str {
    match key {
        "grouping_operation" => "GROUP BY 聚合",
        "ordering_operation" => "ORDER BY 排序",
        "duplicating_weedout" => "去重（weedout）",
        "materialized_from_subquery" => "物化子查询",
        "query_specification" => "子查询",
        _ => "操作",
    }
}

/// PostgreSQL EXPLAIN (FORMAT JSON) → 结构化树
pub fn parse_pg(text: &str) -> DbResult<PlanNode> {
    let v: Value = serde_json::from_str(text)
        .map_err(|e| AppError::General(format!("执行计划 JSON 解析失败: {e}")))?;
    let plan = v
        .get(0)
        .and_then(|x| x.get("Plan"))
        .ok_or_else(|| AppError::General("执行计划格式无法识别".into()))?;
    parse_pg_node(plan)
}

fn parse_pg_node(obj: &Value) -> DbResult<PlanNode> {
    let mut node = PlanNode::leaf(
        obj.get("Node Type")
            .and_then(Value::as_str)
            .unwrap_or("Plan")
            .to_string(),
    );
    for (key, label) in [
        ("Relation Name", "关系"),
        ("Scan Direction", "扫描方向"),
        ("Index Name", "索引"),
        ("Join Type", "连接类型"),
        ("Startup Cost", "启动成本"),
        ("Total Cost", "总成本"),
        ("Plan Rows", "预估行数"),
        ("Plan Width", "行宽"),
        ("Filter", "过滤条件"),
        ("Index Cond", "索引条件"),
        ("Hash Cond", "Hash 条件"),
        ("Sort Key", "排序键"),
    ] {
        if let Some(v) = obj.get(key) {
            let text = match v {
                Value::String(s) => s.clone(),
                other => other.to_string(),
            };
            if key == "Total Cost" {
                node.cost = v.as_f64();
            }
            if key == "Plan Rows" {
                node.rows = v.as_u64();
            }
            node.attrs.push((label.into(), text));
        }
    }
    if let Some(Value::Array(children)) = obj.get("Plans") {
        for c in children {
            node.children.push(parse_pg_node(c)?);
        }
    }
    Ok(node)
}

/// SQLite `EXPLAIN QUERY PLAN` 结果行 → 结构化树。
///
/// 列序固定：id, parent, notused, detail（id/parent 为计划节点编号，
/// parent = 0 为根；detail 为人类可读的访问方式描述，如
/// "SCAN t"、"SEARCH t USING INDEX idx (a=?)"）。
pub fn sqlite_plan_from_rows(result: &QueryResult) -> DbResult<PlanNode> {
    use std::collections::HashMap;

    let cell_text = |c: &CellValue| -> Option<String> {
        match c {
            CellValue::Null => None,
            CellValue::Text(s) | CellValue::Binary(s) => Some(s.clone()),
            CellValue::Int(i) => Some(i.to_string()),
            CellValue::UInt(u) => Some(u.to_string()),
            CellValue::Float(f) => Some(f.to_string()),
            CellValue::Bool(b) => Some(if *b { "1".into() } else { "0".into() }),
        }
    };
    // 行 → (id, parent, detail)
    let mut entries: Vec<(i64, i64, String)> = Vec::new();
    for row in &result.rows {
        let id = row.first().and_then(|c| match c {
            CellValue::Int(i) => Some(*i),
            CellValue::Text(s) => s.trim().parse().ok(),
            _ => None,
        });
        let parent = row.get(1).and_then(|c| match c {
            CellValue::Int(i) => Some(*i),
            CellValue::Text(s) => s.trim().parse().ok(),
            _ => None,
        });
        let detail = row.get(3).and_then(|c| cell_text(c)).filter(|s| !s.is_empty());
        if let (Some(id), Some(parent), Some(detail)) = (id, parent, detail) {
            entries.push((id, parent, detail));
        }
    }
    if entries.is_empty() {
        return Err(AppError::General("执行计划查询无输出".into()));
    }

    let mut nodes: HashMap<i64, PlanNode> = HashMap::new();
    for (id, _, detail) in &entries {
        let mut node = PlanNode::leaf(detail.clone());
        node.attrs.push(("id".into(), id.to_string()));
        nodes.insert(*id, node);
    }
    // parent 挂接（先建好所有节点再归并；逐个 remove 避免双重可变借用）
    let mut roots: Vec<PlanNode> = Vec::new();
    let mut orphans: Vec<PlanNode> = Vec::new();
    for (id, parent, _) in &entries {
        let Some(node) = nodes.remove(id) else {
            continue;
        };
        if *parent == 0 {
            roots.push(node);
        } else if let Some(p) = nodes.get_mut(parent) {
            p.children.push(node);
        } else {
            // parent 未出现在结果中（防御异常输出）：平铺到根
            orphans.push(node);
        }
    }
    let mut root = PlanNode::leaf("查询计划");
    root.children = roots;
    root.children.append(&mut orphans);
    Ok(root)
}
