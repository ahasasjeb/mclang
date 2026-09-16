//! 版本数据生成：`cargo run --bin generate-version-data`。
//!
//! 从仓库内的 `minecraft_client_26.3-rc-2/` 源码重新生成
//! `data/version/26.3-rc-2/entity_nbt.json`。测试会比对生成物与随附快照，
//! 修改源码或标签提取逻辑后必须重新运行本命令。

use std::path::PathBuf;

use mclang::version::entity_nbt;

fn main() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let source = root.join("minecraft_client_26.3-rc-2");
    let output = root.join("data/version/26.3-rc-2/entity_nbt.json");

    let json = entity_nbt::generate(&source).unwrap_or_else(|error| {
        eprintln!("生成失败：{error}");
        std::process::exit(1);
    });
    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent).expect("无法创建版本数据目录");
    }
    std::fs::write(&output, json).expect("无法写入版本数据");
    println!("已写出 {}", output.display());
}
