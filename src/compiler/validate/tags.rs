//! 函数标签声明：名称、条目引用与循环引用检查。
//!
//! 标签里的函数条目必须是本命名空间内已声明的函数，`#标签` 条目必须是本命名
//! 空间内已声明的标签，字符串条目按资源位置校验但不做存在性检查（可能是其他
//! 数据包的资源）。循环引用会被 Minecraft 的标签加载拒绝，这里在编译期报告。

use std::collections::{HashMap, HashSet};

use crate::ast::{FunctionTagDecl, FunctionTagEntry, Program};
use crate::diagnostic::Diagnostic;

use super::Signature;
use super::graph::cyclic_nodes;
use super::rules::{valid_resource_location, validate_identifier};

/// 收集函数标签声明并检查名称与重复。
pub(super) fn collect_function_tags<'a>(
    program: &'a Program,
    diagnostics: &mut Vec<Diagnostic>,
) -> HashMap<&'a str, &'a FunctionTagDecl> {
    let mut tags = HashMap::new();
    for tag in &program.function_tags {
        validate_identifier("函数标签", &tag.name, tag.span, diagnostics);
        if tags.insert(tag.name.as_str(), tag).is_some() {
            diagnostics.push(Diagnostic::new(
                format!("重复声明函数标签 `{}`", tag.name),
                tag.span,
            ));
        }
    }
    tags
}

/// 按声明顺序排列标签：映射表本身无序，直接遍历会让诊断顺序随哈希种子变化。
fn tags_in_declaration_order<'a>(
    tags: &HashMap<&'a str, &'a FunctionTagDecl>,
) -> Vec<&'a FunctionTagDecl> {
    let mut declarations = tags.values().copied().collect::<Vec<_>>();
    declarations.sort_by_key(|tag| (tag.span.source, tag.span.start));
    declarations
}

/// 检查标签条目的引用与循环；必须在签名表建立之后调用。
pub(super) fn validate_function_tags(
    tags: &HashMap<&str, &FunctionTagDecl>,
    signatures: &HashMap<&str, Signature>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for tag in tags_in_declaration_order(tags) {
        for entry in &tag.values {
            match entry {
                FunctionTagEntry::Function(name, span) => {
                    if !signatures.contains_key(name.as_str()) {
                        diagnostics.push(Diagnostic::new(
                            format!("函数标签 `{}` 引用了不存在的函数 `{name}`", tag.name),
                            *span,
                        ));
                    }
                }
                FunctionTagEntry::Tag(name, span) => {
                    if !tags.contains_key(name.as_str()) {
                        diagnostics.push(Diagnostic::new(
                            format!("函数标签 `{}` 引用了不存在的标签 `{name}`", tag.name),
                            *span,
                        ));
                    }
                }
                FunctionTagEntry::External(value, span) => {
                    let location = value.strip_prefix('#').unwrap_or(value);
                    if !valid_resource_location(location) {
                        diagnostics.push(Diagnostic::new(
                            format!("`{value}` 不是有效的资源位置"),
                            *span,
                        ));
                    }
                }
            }
        }
    }
    detect_cycles(tags, diagnostics);
}

fn collect_functions<'a>(
    tag: &str,
    tags: &HashMap<&'a str, &'a FunctionTagDecl>,
    visited: &mut HashSet<&'a str>,
    seen_functions: &mut HashSet<&'a str>,
    reachable: &mut Vec<&'a str>,
) {
    let Some(declaration) = tags.get(tag) else {
        return;
    };
    for entry in &declaration.values {
        match entry {
            FunctionTagEntry::Function(name, _) => {
                let name = name.as_str();
                if seen_functions.insert(name) {
                    reachable.push(name);
                }
            }
            FunctionTagEntry::Tag(name, _) => {
                if let Some((&reference, _)) = tags.get_key_value(name.as_str())
                    && visited.insert(reference)
                {
                    collect_functions(reference, tags, visited, seen_functions, reachable);
                }
            }
            FunctionTagEntry::External(_, _) => {}
        }
    }
}

/// 为每个标签预先展开可达函数，使函数体内的每个标签调用只需查表。
pub(super) fn reachable_functions_by_tag<'a>(
    tags: &HashMap<&'a str, &'a FunctionTagDecl>,
) -> HashMap<&'a str, Vec<&'a str>> {
    let graph = tag_graph(tags);
    let cyclic = cyclic_nodes(tags.keys().copied(), &graph);

    // 沿反向边标记所有能到达环的标签。合法的剩余子图是 DAG，可从叶节点开始
    // 动态展开；有环的无效输入沿用逐根 DFS，保留原有的去重和声明顺序语义。
    let mut reverse = tags
        .keys()
        .map(|&name| (name, Vec::new()))
        .collect::<HashMap<_, _>>();
    for (&name, children) in &graph {
        for &child in children {
            reverse.entry(child).or_default().push(name);
        }
    }
    let mut reaches_cycle = cyclic.clone();
    let mut pending: Vec<&str> = cyclic.iter().copied().collect();
    while let Some(name) = pending.pop() {
        if let Some(parents) = reverse.get(name) {
            for &parent in parents {
                if reaches_cycle.insert(parent) {
                    pending.push(parent);
                }
            }
        }
    }

    let mut remaining_children = tags
        .keys()
        .filter(|name| !reaches_cycle.contains(**name))
        .map(|&name| (name, 0usize))
        .collect::<HashMap<_, _>>();
    let mut parents = HashMap::<&str, Vec<&str>>::new();
    for &name in remaining_children.keys() {
        parents.entry(name).or_default();
    }
    for (&name, children) in &graph {
        if reaches_cycle.contains(name) {
            continue;
        }
        for &child in children {
            *remaining_children
                .get_mut(name)
                .expect("acyclic tag must be indexed") += 1;
            parents.entry(child).or_default().push(name);
        }
    }

    let mut ready: Vec<&str> = remaining_children
        .iter()
        .filter_map(|(&name, &count)| (count == 0).then_some(name))
        .collect();
    let mut reachable = HashMap::with_capacity(tags.len());
    while let Some(name) = ready.pop() {
        let Some(declaration) = tags.get(name) else {
            continue;
        };
        let mut functions = Vec::new();
        let mut seen_functions = HashSet::new();
        for entry in &declaration.values {
            match entry {
                FunctionTagEntry::Function(function, _) => {
                    let function = function.as_str();
                    if seen_functions.insert(function) {
                        functions.push(function);
                    }
                }
                FunctionTagEntry::Tag(child, _) => {
                    if let Some(child_functions) = reachable.get(child.as_str()) {
                        for &function in child_functions {
                            if seen_functions.insert(function) {
                                functions.push(function);
                            }
                        }
                    }
                }
                FunctionTagEntry::External(_, _) => {}
            }
        }
        reachable.insert(name, functions);

        if let Some(dependent_tags) = parents.get(name) {
            for &parent in dependent_tags {
                let remaining = remaining_children
                    .get_mut(parent)
                    .expect("dependent tag must be indexed");
                *remaining -= 1;
                if *remaining == 0 {
                    ready.push(parent);
                }
            }
        }
    }

    for &name in &reaches_cycle {
        let mut functions = Vec::new();
        let mut visited_tags = HashSet::new();
        let mut seen_functions = HashSet::new();
        collect_functions(
            name,
            tags,
            &mut visited_tags,
            &mut seen_functions,
            &mut functions,
        );
        reachable.insert(name, functions);
    }
    reachable
}

fn tag_graph<'a>(
    tags: &HashMap<&'a str, &'a FunctionTagDecl>,
) -> HashMap<&'a str, HashSet<&'a str>> {
    let mut graph = HashMap::with_capacity(tags.len());
    for (&name, declaration) in tags {
        let children = declaration
            .values
            .iter()
            .filter_map(|entry| {
                let FunctionTagEntry::Tag(child, _) = entry else {
                    return None;
                };
                tags.get_key_value(child.as_str()).map(|(key, _)| *key)
            })
            .collect();
        graph.insert(name, children);
    }
    graph
}

fn detect_cycles(tags: &HashMap<&str, &FunctionTagDecl>, diagnostics: &mut Vec<Diagnostic>) {
    let graph = tag_graph(tags);
    let cyclic = cyclic_nodes(tags.keys().copied(), &graph);
    for declaration in tags_in_declaration_order(tags) {
        let name = declaration.name.as_str();
        if cyclic.contains(name) {
            diagnostics.push(Diagnostic::new(
                format!("函数标签 `{name}` 形成循环引用"),
                declaration.span,
            ));
        }
    }
}
