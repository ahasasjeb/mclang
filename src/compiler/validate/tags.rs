//! 函数标签声明：名称、条目引用与循环引用检查。
//!
//! 标签里的函数条目必须是本命名空间内已声明的函数，`#标签` 条目必须是本命名
//! 空间内已声明的标签，字符串条目按资源位置校验但不做存在性检查（可能是其他
//! 数据包的资源）。循环引用会被 Minecraft 的标签加载拒绝，这里在编译期报告。

use std::collections::{HashMap, HashSet};

use crate::ast::{FunctionTagDecl, FunctionTagEntry, Program};
use crate::diagnostic::Diagnostic;

use super::Signature;
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

/// 检查标签条目的引用与循环；必须在签名表建立之后调用。
pub(super) fn validate_function_tags(
    tags: &HashMap<&str, &FunctionTagDecl>,
    signatures: &HashMap<&str, Signature>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for tag in tags.values() {
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

/// 展开标签（含嵌套标签引用）后可达的全部本命名空间函数，按声明顺序去重。
pub(super) fn reachable_functions<'a>(
    tag: &str,
    tags: &HashMap<&'a str, &'a FunctionTagDecl>,
) -> Vec<&'a str> {
    let mut reachable = Vec::new();
    let mut visited = HashSet::new();
    let mut seen_functions = HashSet::new();
    collect_functions(
        tag,
        tags,
        &mut visited,
        &mut seen_functions,
        &mut reachable,
    );
    reachable
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

fn detect_cycles(tags: &HashMap<&str, &FunctionTagDecl>, diagnostics: &mut Vec<Diagnostic>) {
    for (name, declaration) in tags {
        let mut visited = HashSet::new();
        if reaches_tag(name, name, tags, &mut visited) {
            diagnostics.push(Diagnostic::new(
                format!("函数标签 `{name}` 形成循环引用"),
                declaration.span,
            ));
        }
    }
}

fn reaches_tag<'a>(
    current: &str,
    target: &str,
    tags: &HashMap<&'a str, &'a FunctionTagDecl>,
    visited: &mut HashSet<&'a str>,
) -> bool {
    let Some(declaration) = tags.get(current) else {
        return false;
    };
    for entry in &declaration.values {
        let FunctionTagEntry::Tag(name, _) = entry else {
            continue;
        };
        let Some((&reference, _)) = tags.get_key_value(name.as_str()) else {
            continue;
        };
        if reference == target {
            return true;
        }
        if visited.insert(reference) && reaches_tag(reference, target, tags, visited) {
            return true;
        }
    }
    false
}
