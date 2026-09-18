use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use super::text::string_args_in_calls;

/// 目录下排序后的 `.java` 文件。
pub(super) fn java_files(directory: &Path) -> Result<Vec<PathBuf>, String> {
    let mut files: Vec<PathBuf> = sorted_entries(directory)?
        .into_iter()
        .filter(|path| path.extension().and_then(|value| value.to_str()) == Some("java"))
        .collect();
    files.sort();
    Ok(files)
}

/// 数据包资源目录清单：`(类型, 目录, 扩展名)`。
///
/// 顶层目录直接作为类型；`worldgen` 与 `tags` 之下的子目录单独成类。
pub(super) fn data_kinds(root: &Path) -> Result<Vec<(String, PathBuf, &'static str)>, String> {
    let data = root.join("data/minecraft");
    let mut kinds = Vec::new();
    for entry in sorted_entries(&data)? {
        let name = entry
            .file_name()
            .map(|value| value.to_string_lossy().into_owned())
            .unwrap_or_default();
        if !entry.is_dir() || matches!(name.as_str(), "datapacks" | "tags") {
            continue;
        }
        if name == "worldgen" {
            for sub in sorted_entries(&entry)? {
                if sub.is_dir() {
                    let sub_name = sub
                        .file_name()
                        .map(|value| value.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    let kind = format!("worldgen/{sub_name}");
                    kinds.push((kind, sub, "json"));
                }
            }
        } else if name == "structure" {
            kinds.push((name, entry, "nbt"));
        } else {
            kinds.push((name, entry, "json"));
        }
    }
    // 后处理效果注册表的数据在客户端资源目录里。
    let post_effect = root.join("assets/minecraft/post_effect");
    if post_effect.is_dir() {
        kinds.push(("post_effect".into(), post_effect, "json"));
    }
    Ok(kinds)
}

/// 递归列出目录下的资源 id（相对路径去掉扩展名）。
pub(super) fn scan_resource_ids(
    extractor: &mut Extractor<'_>,
    directory: &Path,
    extension: &str,
) -> Result<BTreeSet<String>, String> {
    let mut ids = BTreeSet::new();
    let mut stack = vec![directory.to_path_buf()];
    while let Some(current) = stack.pop() {
        for entry in sorted_entries(&current)? {
            if entry.is_dir() {
                stack.push(entry);
            } else if entry.extension().and_then(|value| value.to_str()) == Some(extension) {
                let relative = entry
                    .strip_prefix(directory)
                    .map_err(|error| error.to_string())?
                    .to_string_lossy()
                    .replace('\\', "/");
                let id = relative[..relative.len() - extension.len() - 1].to_string();
                extractor.digest_path(&entry);
                ids.insert(id);
            }
        }
    }
    Ok(ids)
}

/// 扫描 `data/minecraft/tags`，返回含文件的注册表目录清单。
pub(super) fn scan_tag_registries(
    extractor: &mut Extractor<'_>,
    root: &Path,
) -> Result<Vec<String>, String> {
    let tags = root.join("data/minecraft/tags");
    let mut found = BTreeSet::new();
    let mut stack = vec![tags.clone()];
    while let Some(current) = stack.pop() {
        let mut has_json = false;
        for entry in sorted_entries(&current)? {
            if entry.is_dir() {
                stack.push(entry);
            } else if entry.extension().and_then(|value| value.to_str()) == Some("json") {
                has_json = true;
                extractor.digest_path(&entry);
            }
        }
        if has_json {
            let relative = current
                .strip_prefix(&tags)
                .map_err(|error| error.to_string())?
                .to_string_lossy()
                .replace('\\', "/");
            found.insert(relative);
        }
    }
    Ok(found.into_iter().collect())
}

/// 槽位清单：单槽、范围槽（前缀 + 数量）与多槽集合。
#[derive(Default)]
pub(super) struct Slots {
    pub(super) single: BTreeSet<String>,
    pub(super) ranges: BTreeMap<String, u64>,
    pub(super) multi: BTreeSet<String>,
}

impl Slots {
    pub(super) fn to_value(&self) -> Value {
        json!({
            "single": self.single,
            "ranges": self.ranges,
            "multi": self.multi,
        })
    }
}

/// 读取源码并按文件内容累计摘要。
pub(super) struct Extractor<'a> {
    root: &'a Path,
    digest: u64,
}

impl<'a> Extractor<'a> {
    pub(super) fn new(root: &'a Path) -> Self {
        Self {
            root,
            digest: FNV_OFFSET,
        }
    }

    pub(super) fn read(&mut self, relative: &str) -> Result<String, String> {
        let path = self.root.join(relative);
        let text = fs::read_to_string(&path)
            .map_err(|error| format!("无法读取 {}：{error}", path.display()))?;
        self.digest = fnv_update(self.digest, relative.as_bytes());
        self.digest = fnv_update(self.digest, text.as_bytes());
        Ok(text)
    }

    pub(super) fn digest_path(&mut self, path: &Path) {
        let relative = path.strip_prefix(self.root).unwrap_or(path);
        let text = relative.to_string_lossy().replace('\\', "/");
        self.digest = fnv_update(self.digest, text.as_bytes());
    }

    /// 读取源码并提取 `call("id")` 形式的 id。
    pub(super) fn read_ids(&mut self, relative: &str, call: &str) -> Result<Vec<String>, String> {
        let text = self.read(relative)?;
        Ok(string_args_in_calls(&text, call).into_iter().collect())
    }

    pub(super) fn finish(&self) -> String {
        format!("fnv1a64:{:016x}", self.digest)
    }
}

/// 注册表插入：统一加上 `minecraft:` 前缀。
pub(super) fn insert(registries: &mut BTreeMap<String, BTreeSet<String>>, kind: &str, id: &str) {
    registries
        .entry(kind.to_string())
        .or_default()
        .insert(format!("minecraft:{id}"));
}

pub(super) const DYE_COLORS: [&str; 16] = [
    "white",
    "orange",
    "magenta",
    "light_blue",
    "yellow",
    "lime",
    "pink",
    "gray",
    "light_gray",
    "cyan",
    "purple",
    "blue",
    "brown",
    "green",
    "red",
    "black",
];

pub(super) const COPPER_PREFIXES: [&str; 8] = [
    "",
    "exposed_",
    "weathered_",
    "oxidized_",
    "waxed_",
    "waxed_exposed_",
    "waxed_weathered_",
    "waxed_oxidized_",
];

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

pub(super) fn fnv_update(mut hash: u64, bytes: &[u8]) -> u64 {
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

/// 构造稳定的两空格缩进 JSON。
pub(super) fn render_json(value: &Value) -> Result<String, String> {
    let mut text = serde_json::to_string_pretty(value).map_err(|error| error.to_string())?;
    text.push('\n');
    Ok(text)
}

pub(super) fn sorted_entries(directory: &Path) -> Result<Vec<PathBuf>, String> {
    let mut entries: Vec<PathBuf> = fs::read_dir(directory)
        .map_err(|error| format!("无法读取目录 {}：{error}", directory.display()))?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<Result<_, _>>()
        .map_err(|error| error.to_string())?;
    entries.sort();
    Ok(entries)
}
