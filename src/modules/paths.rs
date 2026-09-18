use std::collections::{BTreeMap, HashSet, VecDeque};
use std::path::{Component, Path};

use crate::ast::Span;
use crate::name_walk::NameRole;

/// 模块路径：`a/b/mod.mcl` 与 `a/b.mcl` 都是 `a/b`，入口是空路径。
pub(super) fn module_segments(path: &Path, root: &Path) -> Option<Vec<String>> {
    let relative = path.strip_prefix(root).ok()?;
    let mut segments = Vec::new();
    for component in relative.components() {
        let Component::Normal(part) = component else {
            return None;
        };
        segments.push(part.to_string_lossy().into_owned());
    }
    let file = segments.pop()?;
    let stem = file.strip_suffix(".mcl")?;
    if stem != "mod" {
        segments.push(stem.to_owned());
    }
    Some(segments)
}

/// 限定名：文件类名字用 `/`，内部名字用 `.`。
pub(super) fn qualify(role: NameRole, segments: &[String], name: &str) -> String {
    if segments.is_empty() {
        return name.to_owned();
    }
    if matches!(
        role,
        NameRole::Function | NameRole::Resource | NameRole::Advancement | NameRole::Tag
    ) {
        format!("{}/{}", segments.join("/"), name)
    } else {
        format!("{}.{}", segments.join("."), name)
    }
}

pub(super) fn reachable_modules(
    root: usize,
    edges: &BTreeMap<usize, Vec<(usize, Span)>>,
) -> Vec<usize> {
    let mut reachable = HashSet::new();
    let mut order = Vec::new();
    let mut queue = VecDeque::new();
    reachable.insert(root);
    order.push(root);
    queue.push_back(root);
    while let Some(index) = queue.pop_front() {
        if let Some(targets) = edges.get(&index) {
            for (target, _) in targets {
                if reachable.insert(*target) {
                    order.push(*target);
                    queue.push_back(*target);
                }
            }
        }
    }
    order
}
