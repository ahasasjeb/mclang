//! 从客户端 blockstates 模型提取每个方块可观察到的属性和值。
//! 水浸等不影响模型的属性可能不在此表中，校验器单独保守处理。

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use serde_json::{Value, json};

pub fn generate_block_states(source: &Path) -> Result<String, String> {
    let directory = source.join("assets/minecraft/blockstates");
    let mut files = fs::read_dir(&directory)
        .map_err(|error| format!("无法读取 {}：{error}", directory.display()))?
        .map(|entry| {
            entry
                .map(|entry| entry.path())
                .map_err(|error| error.to_string())
        })
        .collect::<Result<Vec<_>, _>>()?;
    files.sort();
    let mut blocks = BTreeMap::new();
    for file in files {
        if file.extension().and_then(|extension| extension.to_str()) != Some("json") {
            continue;
        }
        let id = file
            .file_stem()
            .and_then(|stem| stem.to_str())
            .ok_or("方块状态文件名无效")?;
        let content = fs::read_to_string(&file)
            .map_err(|error| format!("无法读取 {}：{error}", file.display()))?;
        let document: Value = serde_json::from_str(&content)
            .map_err(|error| format!("{}：{error}", file.display()))?;
        let mut properties: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        if let Some(variants) = document.get("variants").and_then(Value::as_object) {
            for selector in variants.keys() {
                for pair in selector.split(',').filter(|pair| !pair.is_empty()) {
                    if let Some((name, value)) = pair.split_once('=') {
                        insert_values(&mut properties, name, value);
                    }
                }
            }
        }
        if let Some(parts) = document.get("multipart").and_then(Value::as_array) {
            for part in parts {
                if let Some(condition) = part.get("when") {
                    collect_conditions(condition, &mut properties);
                }
            }
        }
        blocks.insert(format!("minecraft:{id}"), properties);
    }
    let output = json!({ "blocks": blocks });
    serde_json::to_string_pretty(&output)
        .map(|text| format!("{text}\n"))
        .map_err(|error| format!("无法序列化方块状态快照：{error}"))
}

fn collect_conditions(value: &Value, properties: &mut BTreeMap<String, BTreeSet<String>>) {
    if let Some(object) = value.as_object() {
        for (key, value) in object {
            if key == "OR" || key == "AND" {
                collect_conditions(value, properties);
            } else if let Some(value) = value.as_str() {
                insert_values(properties, key, value);
            }
        }
    } else if let Some(array) = value.as_array() {
        for value in array {
            collect_conditions(value, properties);
        }
    }
}

fn insert_values(properties: &mut BTreeMap<String, BTreeSet<String>>, name: &str, value: &str) {
    for value in value.split('|') {
        properties
            .entry(name.to_owned())
            .or_default()
            .insert(value.to_owned());
    }
}
