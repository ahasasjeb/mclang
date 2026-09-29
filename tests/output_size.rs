//! 产物规模预算回归。
//!
//! 对 `tests/valid/**` 与 `examples/**` 的每个可编译目标记录 `.mcfunction`
//! 数量、`__mcl` 辅助函数数量、命令条数与字节数，并与提交的
//! `tests/output_budget.json` 比较：任一指标超过记录值即失败。
//!
//! 数字记录的是**上限**，用于让「优化产物规模」这类改动有可复现的对照。
//! 确认产物增长合理后可用 `MCLANG_UPDATE_OUTPUT_BUDGET=1 cargo test --test
//! output_size` 重新生成预算文件。

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use mclang::{OutputStats, check_file};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// 全部可编译语料：`tests/valid` 的每一项，以及 `examples` 下带 `main.mcl`
/// 的项目目录。`tests/invalid` 只要求报错，不参与规模统计。
fn targets() -> Vec<(String, PathBuf)> {
    let root = repo_root();
    let mut targets = Vec::new();

    let mut valid: Vec<_> = fs::read_dir(root.join("tests/valid"))
        .expect("缺少 tests/valid")
        .map(|entry| entry.expect("无法读取 tests/valid").path())
        .collect();
    valid.sort();
    for path in valid {
        let name = path
            .file_name()
            .expect("tests/valid 下的条目都有名字")
            .to_string_lossy()
            .into_owned();
        targets.push((format!("tests/valid/{name}"), path));
    }

    let mut examples: Vec<_> = fs::read_dir(root.join("examples"))
        .expect("缺少 examples")
        .map(|entry| entry.expect("无法读取 examples").path())
        .collect();
    examples.sort();
    for path in examples {
        if !path.join("main.mcl").is_file() {
            continue;
        }
        let name = path
            .file_name()
            .expect("examples 下的条目都有名字")
            .to_string_lossy()
            .into_owned();
        targets.push((format!("examples/{name}"), path));
    }

    assert!(!targets.is_empty(), "没有可统计的语料");
    targets
}

fn stats_from_json(value: &serde_json::Value, target: &str) -> OutputStats {
    let field = |name: &str| {
        value[name]
            .as_u64()
            .unwrap_or_else(|| panic!("{target} 的 `{name}` 必须是非负整数")) as usize
    };
    OutputStats {
        functions: field("functions"),
        helpers: field("helpers"),
        commands: field("commands"),
        bytes: field("bytes"),
    }
}

fn read_budget(path: &Path) -> BTreeMap<String, OutputStats> {
    let text = fs::read_to_string(path).unwrap_or_else(|error| {
        panic!(
            "无法读取 {}：{error}；用 MCLANG_UPDATE_OUTPUT_BUDGET=1 cargo test --test output_size 生成",
            path.display()
        )
    });
    let value: serde_json::Value = serde_json::from_str(&text).expect("预算文件必须是有效 JSON");
    let object = value.as_object().expect("预算文件根值必须是对象");
    object
        .iter()
        .map(|(target, entry)| (target.clone(), stats_from_json(entry, target)))
        .collect()
}

fn write_budget(path: &Path, stats: &BTreeMap<String, OutputStats>) {
    let mut object = serde_json::Map::new();
    for (target, stats) in stats {
        object.insert(
            target.clone(),
            serde_json::json!({
                "functions": stats.functions,
                "helpers": stats.helpers,
                "commands": stats.commands,
                "bytes": stats.bytes,
            }),
        );
    }
    let text = serde_json::to_string_pretty(&serde_json::Value::Object(object))
        .expect("预算可以序列化")
        + "\n";
    fs::write(path, text).expect("无法写入预算文件");
    println!("已更新 {}：{} 个语料", path.display(), stats.len());
}

#[test]
fn output_size_budget_holds() {
    let root = repo_root();
    let budget_path = root.join("tests/output_budget.json");

    let mut current = BTreeMap::new();
    for (name, path) in targets() {
        let summary =
            check_file(&path).unwrap_or_else(|error| panic!("{name} 应当编译通过：\n{error}"));
        current.insert(name, summary.output_stats);
    }

    let totals = current
        .values()
        .fold(OutputStats::default(), |mut totals, stats| {
            totals.functions += stats.functions;
            totals.helpers += stats.helpers;
            totals.commands += stats.commands;
            totals.bytes += stats.bytes;
            totals
        });
    println!(
        "产物总量：{} 个函数（其中 __mcl 辅助函数 {} 个），{} 条命令，{} 字节",
        totals.functions, totals.helpers, totals.commands, totals.bytes
    );

    if std::env::var_os("MCLANG_UPDATE_OUTPUT_BUDGET").is_some() {
        write_budget(&budget_path, &current);
        return;
    }

    let budget = read_budget(&budget_path);
    let mut failures = Vec::new();
    for (name, stats) in &current {
        let Some(limit) = budget.get(name) else {
            failures.push(format!("{name}：预算文件缺少该语料"));
            continue;
        };
        for (field, actual, allowed) in [
            ("functions", stats.functions, limit.functions),
            ("helpers", stats.helpers, limit.helpers),
            ("commands", stats.commands, limit.commands),
            ("bytes", stats.bytes, limit.bytes),
        ] {
            if actual > allowed {
                failures.push(format!("{name}：{field} {actual} 超过预算 {allowed}"));
            }
        }
    }
    for name in budget.keys() {
        if !current.contains_key(name) {
            failures.push(format!("{name}：预算文件里的语料已不存在"));
        }
    }
    assert!(
        failures.is_empty(),
        "产物规模超过预算（确认合理后用 MCLANG_UPDATE_OUTPUT_BUDGET=1 cargo test --test output_size 更新）：\n{}",
        failures.join("\n")
    );
}
