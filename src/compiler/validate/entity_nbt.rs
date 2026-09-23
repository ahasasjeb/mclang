//! 实体 NBT 合并语句的键校验：对照 26.3 源码提取的标签快照。
//!
//! 快照（`data/version/26.3/entity_nbt.json`）记录每个实体类型沿继承链
//! 能写出的全部键及粗类型。`spawn` 与非玩家查询的 `each` 知道具体实体类型，
//! 按该类型检查；`@non_player`/`@entity` 等只知道“有实体”，按全体键的并集检查。
//! 未知键会给出最近候选，避免 `NoAi` 这类拼写错误悄悄无效。

use crate::ast::{NbtCategory, NbtValue, NbtValueKind, Span};
use crate::diagnostic::Diagnostic;
use crate::version::entity_nbt::{EntityTagType, alias_of, catalog};

/// 校验一段实体 NBT：键存在且值类型匹配。
pub(super) fn validate_entity_nbt(
    value: &NbtValue,
    entity_type: Option<&str>,
    span: Span,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let catalog = catalog();
    let NbtValueKind::Compound(entries) = &value.kind else {
        diagnostics.push(Diagnostic::new("实体 NBT 需要复合 `{ ... }`", span));
        return;
    };
    let allowed = entity_type.and_then(|entity_type| catalog.entity_tags(entity_type));
    for entry in entries {
        let known = match allowed {
            Some(allowed) => allowed.contains(&entry.key),
            None => catalog.knows(&entry.key),
        };
        if !known {
            let hint = match catalog.suggest(&entry.key, allowed) {
                Some(candidate) => format!("；是否想写 {candidate}？"),
                None => String::new(),
            };
            let label = match alias_of(&entry.key) {
                Some(alias) => format!("`{}`（中文 `{alias}`）", entry.key),
                None => format!("`{}`", entry.key),
            };
            let scope = if allowed.is_some() {
                "该实体类型"
            } else {
                "26.3 的实体"
            };
            diagnostics.push(Diagnostic::new(
                format!("{scope}没有名为 {label} 的 NBT 标签{hint}"),
                entry.key_span,
            ));
            continue;
        }
        if let Some(expected) = catalog.tag_type(&entry.key)
            && !compatible(expected, &entry.value)
        {
            let label = match alias_of(&entry.key) {
                Some(alias) => format!("`{}`（中文 `{alias}`）", entry.key),
                None => format!("`{}`", entry.key),
            };
            diagnostics.push(Diagnostic::new(
                format!("{label} 需要{}", expected.label()),
                entry.value.span,
            ));
        }
    }
}

/// 值是否满足标签的期望类型。
fn compatible(expected: EntityTagType, value: &NbtValue) -> bool {
    match expected {
        EntityTagType::Any => true,
        EntityTagType::Bool | EntityTagType::Number => value.category() == NbtCategory::Numeric,
        EntityTagType::String => value.category() == NbtCategory::String,
        EntityTagType::Component => matches!(
            value.category(),
            NbtCategory::String | NbtCategory::Compound | NbtCategory::List
        ),
        EntityTagType::List => value.category() == NbtCategory::List,
        EntityTagType::NumericList => match &value.kind {
            NbtValueKind::List(values) => values
                .iter()
                .all(|element| element.category() == NbtCategory::Numeric),
            _ => false,
        },
        EntityTagType::StringList => match &value.kind {
            NbtValueKind::List(values) => values
                .iter()
                .all(|element| element.category() == NbtCategory::String),
            _ => false,
        },
        EntityTagType::Compound => value.category() == NbtCategory::Compound,
        EntityTagType::IntArray => value.category() == NbtCategory::NumericArray,
    }
}
