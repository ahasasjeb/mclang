//! 版本数据生成：`cargo xtask generate-version-data`。
//!
//! 保留此入口以兼容旧命令 `cargo run --bin generate-version-data`；
//! 新代码建议使用 xtask。两个入口生成完全相同的快照。

use std::path::PathBuf;

use mclang::version::generate;

fn main() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    match generate::generate_command(&root) {
        Ok(paths) => {
            for path in paths {
                println!("已写出 {}", path.display());
            }
        }
        Err(error) => {
            eprintln!("生成失败：{error}");
            std::process::exit(1);
        }
    }
}
