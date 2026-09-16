//! 物品堆、附魔和实体查询声明的校验。

use std::collections::HashSet;

use crate::ast::{EntityQueryDecl, EntityTypeFilter, ItemEnchantment, ItemStackDecl, Span};
use crate::diagnostic::Diagnostic;

use super::registry::validate_id;
use super::rules::{valid_entity_tag, valid_resource_location};

/// `GiveCommand` 允许的数量上限是物品最大堆叠数乘以 100。
/// 未声明 `max_stack_size` 时物品原型的最小堆叠数是 1，因此保守上限为 100。
pub(super) fn max_give_count(item: &ItemStackDecl) -> u32 {
    item.max_stack_size.unwrap_or(1).saturating_mul(100)
}

pub(super) fn validate_give_count(
    item: &ItemStackDecl,
    count: Option<u32>,
    statement_span: Span,
    count_span: Option<Span>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(count) = count else {
        return;
    };
    let limit = max_give_count(item);
    if count == 0 || count > limit {
        diagnostics.push(Diagnostic::new(
            format!(
                "给予数量必须是 1 到 {limit}（`{}` 的最大堆叠数为 {}）",
                item.name,
                item.max_stack_size.unwrap_or(1)
            ),
            count_span.unwrap_or(statement_span),
        ));
    }
}

pub(super) fn validate_item_stack(item: &ItemStackDecl, diagnostics: &mut Vec<Diagnostic>) {
    validate_id("item", "物品", &item.item_id, item.span, diagnostics);
    let count_limit = max_give_count(item);
    if item.count == 0 || item.count > count_limit {
        diagnostics.push(Diagnostic::new(
            format!("物品定义的 count 必须是 1 到 {count_limit}"),
            item.span,
        ));
    }
    if item.lore.len() > 256 {
        diagnostics.push(Diagnostic::new("物品定义最多包含 256 行 lore", item.span));
    }
    if item
        .custom_name
        .iter()
        .chain(&item.item_name)
        .chain(&item.lore)
        .any(|text| text.chars().any(char::is_control))
    {
        diagnostics.push(Diagnostic::new(
            "物品名称和 lore 不能包含控制字符",
            item.span,
        ));
    }
    validate_item_enchantments("enchantment", &item.enchantments, diagnostics);
    validate_item_enchantments("stored_enchantment", &item.stored_enchantments, diagnostics);
    if item.damage.is_some_and(|damage| damage > i32::MAX as u32) {
        diagnostics.push(Diagnostic::new(
            "物品定义的 damage 不能超过 2147483647",
            item.span,
        ));
    }
    if item
        .max_damage
        .is_some_and(|max_damage| max_damage == 0 || max_damage > i32::MAX as u32)
    {
        diagnostics.push(Diagnostic::new(
            "物品定义的 max_damage 必须是 1 到 2147483647",
            item.span,
        ));
    }
    if item
        .max_stack_size
        .is_some_and(|max_stack_size| max_stack_size == 0 || max_stack_size > 99)
    {
        diagnostics.push(Diagnostic::new(
            "物品定义的 max_stack_size 必须是 1 到 99",
            item.span,
        ));
    }
    if item.max_stack_size.is_some_and(|size| size > 1) && item.max_damage.is_some() {
        diagnostics.push(Diagnostic::new(
            "物品不能同时设置大于 1 的 max_stack_size 和 max_damage",
            item.span,
        ));
    }
    if item
        .item_model
        .as_deref()
        .is_some_and(|model| !valid_resource_location(model))
    {
        diagnostics.push(Diagnostic::new(
            format!(
                "`{}` 不是有效的物品模型资源位置",
                item.item_model.as_deref().unwrap_or_default()
            ),
            item.span,
        ));
    }
    if item.dyed_color.is_some_and(|color| color > 0x00ff_ffff) {
        diagnostics.push(Diagnostic::new(
            "物品定义的 dyed_color 必须是 0 到 16777215 的 RGB 值",
            item.span,
        ));
    }
}

fn validate_item_enchantments(
    property: &str,
    enchantments: &[ItemEnchantment],
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut ids = HashSet::new();
    for enchantment in enchantments {
        validate_id(
            "enchantment",
            "附魔",
            &enchantment.enchantment_id,
            enchantment.span,
            diagnostics,
        );
        if enchantment.level == 0 || enchantment.level > 255 {
            diagnostics.push(Diagnostic::new("附魔等级必须是 1 到 255", enchantment.span));
        }
        if !ids.insert(enchantment.enchantment_id.as_str()) {
            diagnostics.push(Diagnostic::new(
                format!(
                    "物品定义重复声明 {property} `{}`",
                    enchantment.enchantment_id
                ),
                enchantment.span,
            ));
        }
    }
}

pub(super) fn validate_entity_query(query: &EntityQueryDecl, diagnostics: &mut Vec<Diagnostic>) {
    validate_entity_type(&query.entity_type, query.span, diagnostics);
    for filter in &query.type_filters {
        let (value, span) = match filter {
            EntityTypeFilter::Include(value, span) | EntityTypeFilter::Exclude(value, span) => {
                (value, *span)
            }
        };
        validate_entity_type(value, span, diagnostics);
    }
    let mut tags = HashSet::new();
    for tag in query.tags.iter().chain(&query.excluded_tags) {
        if !valid_entity_tag(tag) {
            diagnostics.push(Diagnostic::new(
                format!("`{tag}` 不是有效的实体标签"),
                query.span,
            ));
        }
        if !tags.insert(tag) {
            diagnostics.push(Diagnostic::new(
                format!("实体查询 `{}` 重复使用标签 `{tag}`", query.name),
                query.span,
            ));
        }
    }
    if query
        .limit
        .is_some_and(|limit| limit == 0 || limit > i32::MAX as u32)
    {
        diagnostics.push(Diagnostic::new(
            "查询 limit 必须是 1 到 2147483647",
            query.span,
        ));
    }
    if query
        .within
        .is_some_and(|within| within == 0 || within > 30_000_000)
    {
        diagnostics.push(Diagnostic::new(
            "查询 within 必须是 1 到 30000000",
            query.span,
        ));
    }
    if let Some(filter) = &query.name_filter
        && (filter.value.is_empty() || filter.value.chars().any(char::is_control))
    {
        diagnostics.push(Diagnostic::new(
            "查询 name 不能为空或包含控制字符",
            filter.span,
        ));
    }
    for score in &query.scores {
        if score.objective.is_empty() {
            diagnostics.push(Diagnostic::new("scores 的目标名不能为空", score.span));
        }
        if !valid_int_range(&score.range) {
            diagnostics.push(Diagnostic::new(
                format!("`{}` 不是有效的整数区间，例如 1..5、..5、5..", score.range),
                score.span,
            ));
        }
    }
    if let Some(filter) = &query.nbt_filter
        && (filter.value.is_empty() || !balanced_snbt(&filter.value))
    {
        diagnostics.push(Diagnostic::new(
            "查询 nbt 需要平衡的 SNBT 谓词或路径",
            filter.span,
        ));
    }
    if let Some(box_filter) = &query.box_filter {
        for (label, value) in [
            ("x", &box_filter.x),
            ("y", &box_filter.y),
            ("z", &box_filter.z),
        ] {
            if value.parse::<f64>().is_err() {
                diagnostics.push(Diagnostic::new(
                    format!("box {label} 需要数字"),
                    box_filter.span,
                ));
            }
        }
        for (label, value) in [
            ("dx", &box_filter.dx),
            ("dy", &box_filter.dy),
            ("dz", &box_filter.dz),
        ] {
            if !value.parse::<f64>().is_ok_and(|value| value >= 0.0) {
                diagnostics.push(Diagnostic::new(
                    format!("box {label} 需要非负数字"),
                    box_filter.span,
                ));
            }
        }
    }
    if let Some(distance) = &query.distance
        && !valid_float_range(distance)
    {
        diagnostics.push(Diagnostic::new(
            format!("`{distance}` 不是有效的距离区间，例如 3..10、..10、5.."),
            query.span,
        ));
    }
    if let Some(level) = &query.level
        && !valid_int_range(level)
    {
        diagnostics.push(Diagnostic::new(
            format!("`{level}` 不是有效的等级区间，例如 1..5、..5、5.."),
            query.span,
        ));
    }
    if let Some(gamemode) = &query.gamemode
        && !crate::version::snapshot::snapshot().enum_contains("gamemode", gamemode)
    {
        diagnostics.push(Diagnostic::new(
            format!("`{gamemode}` 不是有效的游戏模式"),
            query.span,
        ));
    }
    if let Some(filter) = &query.team_filter
        && filter.value.is_empty()
    {
        diagnostics.push(Diagnostic::new("查询 team 不能为空", filter.span));
    }
    if let Some(rotation) = &query.rotation {
        for (label, value) in [("偏航", &rotation.yaw), ("俯仰", &rotation.pitch)] {
            if !valid_float_range(value) {
                diagnostics.push(Diagnostic::new(
                    format!("rotate 的{label}区间 `{value}` 无效"),
                    rotation.span,
                ));
            }
        }
    }
    if let Some(predicate) = &query.predicate
        && !valid_resource_location(predicate)
    {
        diagnostics.push(Diagnostic::new(
            format!("`{predicate}` 不是有效的谓词资源位置"),
            query.span,
        ));
    }
    if let Some(advancements) = &query.advancements
        && (advancements.is_empty() || !balanced_snbt(advancements))
    {
        diagnostics.push(Diagnostic::new(
            "查询 advancements 需要平衡的 SNBT 谓词",
            query.span,
        ));
    }
    if let Some(item) = &query.item {
        let snapshot = crate::version::snapshot::snapshot();
        if !snapshot.slots().accepts(&item.slot) && !valid_resource_location(&item.slot) {
            diagnostics.push(Diagnostic::new(
                format!("`{}` 不是有效的槽位来源", item.slot),
                item.span,
            ));
        }
        if !valid_item_predicate(&item.item_id) {
            diagnostics.push(Diagnostic::new(
                format!(
                    "`{}` 不是有效的物品谓词（物品 id、`#标签` 或带组件过滤器的 id）",
                    item.item_id
                ),
                item.span,
            ));
        }
        if item.count.is_some_and(|count| count == 0 || count > 99) {
            diagnostics.push(Diagnostic::new("item.count 必须是 1 到 99", item.span));
        }
        if item
            .custom_name
            .as_ref()
            .is_some_and(|name| name.chars().any(char::is_control))
        {
            diagnostics.push(Diagnostic::new(
                "item.custom_name 不能包含控制字符",
                item.span,
            ));
        }
    }
}

/// 实体类型或 `#标签`。
fn validate_entity_type(value: &str, span: Span, diagnostics: &mut Vec<Diagnostic>) {
    if let Some(tag) = value.strip_prefix('#') {
        if !valid_resource_location(tag) {
            diagnostics.push(Diagnostic::new(
                format!("`{value}` 不是有效的实体类型标签资源位置"),
                span,
            ));
        }
    } else {
        validate_id("entity_type", "实体类型", value, span, diagnostics);
    }
}

/// 选择器区间：`5`、`..5`、`5..`、`5..10` 与小数版本。
pub(super) fn valid_int_range(text: &str) -> bool {
    valid_range(text, |value| value.parse::<i64>().is_ok())
}

pub(super) fn valid_float_range(text: &str) -> bool {
    valid_range(text, |value| value.parse::<f64>().is_ok())
}

fn valid_range(text: &str, parse: impl Fn(&str) -> bool) -> bool {
    if let Some((low, high)) = text.split_once("..") {
        let low_ok = low.is_empty() || parse(low);
        let high_ok = high.is_empty() || parse(high);
        low_ok && high_ok && !(low.is_empty() && high.is_empty())
    } else {
        parse(text)
    }
}

/// 物品谓词：物品 id、`#标签`，可带 `[组件过滤器]`。
pub(super) fn valid_item_predicate(text: &str) -> bool {
    if text.is_empty() || text.contains(char::is_whitespace) {
        return false;
    }
    let base = text.strip_prefix('#').unwrap_or(text);
    let location = base.split('[').next().unwrap_or(base);
    valid_resource_location(location) && balanced_snbt(base)
}

/// SNBT 片段：括号与引号必须配对。
fn balanced_snbt(text: &str) -> bool {
    let mut depth = 0i32;
    let mut quote: Option<char> = None;
    for character in text.chars() {
        if let Some(opening) = quote {
            if character == opening {
                quote = None;
            }
            continue;
        }
        match character {
            '"' | '\'' => quote = Some(character),
            '[' | '{' => depth += 1,
            ']' | '}' => {
                depth -= 1;
                if depth < 0 {
                    return false;
                }
            }
            _ => {}
        }
    }
    depth == 0 && quote.is_none()
}
