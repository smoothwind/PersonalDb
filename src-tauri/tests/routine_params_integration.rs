//! MySQL 例程参数解析回归：FUNCTION 的返回值行（information_schema.parameters 中
//! ordinal_position=0、parameter_name 为 NULL）不得混入参数列表。
//! 历史 bug：单参函数被解析出「无名 #1（实为返回类型）+ 真参数」两行。
//! Oracle 系由 parse_routine_arguments 处理返回行，无此问题。
//!
//! 运行：$env:PDB_IT_MYSQL="host:port:user:pass:db"; cargo test --test routine_params_integration

use personal_db_lib::db::objects::DbObjectType;
use personal_db_lib::db::pool::create_driver;
use personal_db_lib::db::types::{ConnectionConfig, DbKind};

fn skip_if_unset(env: &str) -> Option<String> {
    match std::env::var(env) {
        Ok(v) if !v.is_empty() => Some(v),
        _ => {
            eprintln!("skip: {env} 未设置");
            None
        }
    }
}

fn cfg_for_mysql() -> ConnectionConfig {
    let mut cfg = ConnectionConfig {
        id: "rp".into(), name: "rp".into(), kind: DbKind::MySql, host: "127.0.0.1".into(),
        port: Some(3306), username: "root".into(), password: "root".into(),
        password_ref: None, database: Some("pdbtest".into()), params: Default::default(), group_id: None,
    };
    if let Ok(v) = std::env::var("PDB_IT_MYSQL") {
        let parts: Vec<&str> = v.split(':').collect();
        if parts.len() >= 4 {
            cfg.host = parts[0].into();
            if let Ok(port) = parts[1].parse::<u16>() {
                cfg.port = Some(port);
            }
            cfg.username = parts[2].into();
            cfg.password = parts[3].into();
            if let Some(db) = parts.get(4) {
                cfg.database = Some((*db).into());
            }
        }
    }
    cfg
}

#[tokio::test]
async fn routine_params_mysql() {
    let Some(_) = skip_if_unset("PDB_IT_MYSQL") else { return };
    let driver = create_driver(&cfg_for_mysql()).expect("create_driver");
    let db = cfg_for_mysql().database.unwrap_or_else(|| "pdbtest".into());

    // 单参函数：返回值行必须被过滤，仅剩 P_NAME 一个参数
    let _ = driver.execute(None, "DROP FUNCTION IF EXISTS it_fpy", 1).await;
    driver
        .execute(
            None,
            "CREATE FUNCTION it_fpy(P_NAME varchar(4000)) RETURNS varchar(4000) \
             DETERMINISTIC NO SQL RETURN P_NAME",
            1,
        )
        .await
        .expect("create function");

    let info = driver
        .describe_routine(Some(&db), "it_fpy", DbObjectType::Function)
        .await
        .expect("describe_routine");
    assert_eq!(info.params.len(), 1, "单参函数应解析出 1 个参数: {:?}", info.params);
    assert_eq!(info.params[0].name.as_deref(), Some("P_NAME"), "参数名应含元数据: {:?}", info.params);
    assert_eq!(info.params[0].data_type, "varchar(4000)");
    assert_eq!(info.return_type.as_deref(), Some("varchar(4000)"), "返回类型单独提取");
    eprintln!("[MySQL] 单参函数解析 1 参数 + 返回类型 ✓");

    // 多参函数：顺序与名字（防「过滤返回行」误伤真参数）
    let _ = driver.execute(None, "DROP FUNCTION IF EXISTS it_fpy2", 1).await;
    driver
        .execute(
            None,
            "CREATE FUNCTION it_fpy2(a int, b varchar(10)) RETURNS int \
             DETERMINISTIC NO SQL RETURN a",
            1,
        )
        .await
        .expect("create function 2");
    let info2 = driver
        .describe_routine(Some(&db), "it_fpy2", DbObjectType::Function)
        .await
        .expect("describe_routine 2");
    assert_eq!(info2.params.len(), 2, "双参函数应解析出 2 个参数: {:?}", info2.params);
    assert_eq!(info2.params[0].name.as_deref(), Some("a"));
    assert_eq!(info2.params[1].name.as_deref(), Some("b"));
    eprintln!("[MySQL] 双参函数解析 2 参数 ✓");

    // 清理
    let _ = driver.execute(None, "DROP FUNCTION IF EXISTS it_fpy", 1).await;
    let _ = driver.execute(None, "DROP FUNCTION IF EXISTS it_fpy2", 1).await;
    eprintln!("[MySQL] 例程参数解析回归通过");
}
