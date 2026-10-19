use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    build_oracle_bridge();
    build_jdbc_bridge();
    tauri_build::build()
}

fn binaries_dir() -> PathBuf {
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let dir = manifest.join("binaries");
    std::fs::create_dir_all(&dir).ok();
    dir
}

/// 构建 Oracle 无 OCI 桥接进程（Go sidecar，go-ora）。
/// 缺少 Go 工具链时跳过（运行时将回退 OCI 驱动）。
fn build_oracle_bridge() {
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let src_dir = manifest.join("oracle-bridge");
    let out_dir = binaries_dir();

    let exe_name = if cfg!(target_os = "windows") {
        "oracle-bridge.exe"
    } else {
        "oracle-bridge"
    };
    let target = out_dir.join(exe_name);

    // 监视整个源目录：新增 .go 文件也能触发构建脚本重跑（目录递归扫描）
    println!("cargo:rerun-if-changed=oracle-bridge");

    if Command::new("go").arg("version").output().is_err() {
        println!(
            "cargo:warning=未检测到 Go 工具链，跳过 oracle-bridge 构建（真实 Oracle 将回退 OCI 驱动，需安装 Oracle Instant Client）"
        );
        return;
    }

    if !sources_newer_than(&src_dir, &["main.go", "go.mod", "go.sum"], &target) {
        return;
    }

    let output = Command::new("go")
        .arg("build")
        .arg("-o")
        .arg(&target)
        .arg(".")
        .current_dir(&src_dir)
        .output();

    match output {
        Ok(o) if o.status.success() => println!("oracle-bridge built: {}", target.display()),
        Ok(o) => println!(
            "cargo:warning=oracle-bridge 构建失败：{}（运行时将回退 OCI 驱动）",
            String::from_utf8_lossy(&o.stderr)
        ),
        Err(e) => println!("cargo:warning=oracle-bridge 构建失败：{e}（运行时将回退 OCI 驱动）"),
    }
}

/// 构建通用 JDBC 桥接（Java + JDBC，内置 OceanBase 驱动，打包为 fat jar）。
/// 缺少 Maven 时跳过（Oracle/OceanBase 将回退 OCI 驱动）。
fn build_jdbc_bridge() {
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let src_dir = manifest.join("jdbc-bridge");
    let out_jar = binaries_dir().join("jdbc-bridge.jar");
    let built_jar = src_dir.join("target").join("jdbc-bridge.jar");

    // pom.xml + src/ 目录递归监视（不监视 target/，避免构建产物触发重跑）
    println!("cargo:rerun-if-changed=jdbc-bridge/pom.xml");
    println!("cargo:rerun-if-changed=jdbc-bridge/src");

    // 产物已最新则跳过（避免每次 cargo check 都调用 mvn）
    if !sources_newer_than(&src_dir, &["pom.xml", "src"], &out_jar) {
        return;
    }

    if !maven_available() {
        println!("cargo:warning=未检测到 Maven，跳过 jdbc-bridge 构建（Jdbc 连接将不可用）");
        return;
    }

    let output = run_maven_package(&src_dir);

    match output {
        Ok(o) if o.status.success() => {
            if built_jar.is_file() {
                match std::fs::copy(&built_jar, &out_jar) {
                    Ok(_) => println!("jdbc-bridge built: {}", out_jar.display()),
                    Err(e) => println!("cargo:warning=拷贝 jdbc-bridge.jar 失败：{e}"),
                }
            } else {
                println!("cargo:warning=jdbc-bridge 构建未产出 jar");
            }
        }
        Ok(o) => println!(
            "cargo:warning=jdbc-bridge 构建失败：{}（Jdbc 连接将不可用）",
            String::from_utf8_lossy(&o.stderr)
        ),
        Err(e) => println!("cargo:warning=jdbc-bridge 构建失败：{e}（Jdbc 连接将不可用）"),
    }
}

fn maven_available() -> bool {
    if cfg!(windows) {
        Command::new("cmd")
            .args(["/C", "mvn", "-v"])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    } else {
        Command::new("mvn")
            .arg("-v")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }
}

fn run_maven_package(src_dir: &Path) -> std::io::Result<std::process::Output> {
    if cfg!(windows) {
        Command::new("cmd")
            .args(["/C", "mvn", "-B", "-q", "-DskipTests", "package"])
            .current_dir(src_dir)
            .output()
    } else {
        Command::new("mvn")
            .args(["-B", "-q", "-DskipTests", "package"])
            .current_dir(src_dir)
            .output()
    }
}

/// 任一源文件（递归扫描目录）比 target 新（或 target 不存在）则返回 true（需要重新构建）
fn sources_newer_than(src_dir: &Path, names: &[&str], target: &Path) -> bool {
    let Ok(target_time) = std::fs::metadata(target).and_then(|m| m.modified()) else {
        return true;
    };
    fn newer_than(path: &Path, target_time: std::time::SystemTime) -> bool {
        let Ok(meta) = std::fs::metadata(path) else {
            return false;
        };
        if meta.is_file() {
            return meta.modified().is_ok_and(|t| t > target_time);
        }
        let Ok(entries) = std::fs::read_dir(path) else {
            return false;
        };
        entries
            .filter_map(Result::ok)
            .any(|e| newer_than(&e.path(), target_time))
    }
    names.iter().any(|name| newer_than(&src_dir.join(name), target_time))
}
