//! `cargo xtask <子命令>` 的统一入口。
//!
//! 目前支持：
//!
//! - `generate-version-data`：从 `minecraft_client_26.3/` 重新生成
//!   版本数据快照到 `data/version/26.3/`；
//! - `check-version-data`：比对快照与源码，不写盘（CI 使用）。
//!
//! 子命令省略时等价于 `generate-version-data`。

use std::path::PathBuf;

use mclang::version::generate;

fn main() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let command = arguments
        .first()
        .map(String::as_str)
        .unwrap_or("generate-version-data");
    let result = match command {
        "generate-version-data" => generate::generate_command(&root).map(|paths| {
            for path in paths {
                println!("已写出 {}", path.display());
            }
        }),
        "check-version-data" => generate::check_command(&root).map(|()| {
            println!("版本数据快照与源码一致");
        }),
        other => Err(format!(
            "未知子命令 `{other}`；可用：generate-version-data、check-version-data"
        )),
    };
    if let Err(error) = result {
        eprintln!("xtask 失败：{error}");
        std::process::exit(1);
    }
}
