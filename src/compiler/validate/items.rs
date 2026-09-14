//! 物品堆、附魔和实体查询声明的校验。

use std::collections::HashSet;

use crate::ast::{EntityQueryDecl, ItemEnchantment, ItemStackDecl, Span};
use crate::diagnostic::Diagnostic;

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
    if !valid_resource_location(&item.item_id) {
        diagnostics.push(Diagnostic::new(
            format!("`{}` 不是有效的物品资源位置", item.item_id),
            item.span,
        ));
    }
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
        if !valid_resource_location(&enchantment.enchantment_id) {
            diagnostics.push(Diagnostic::new(
                format!("`{}` 不是有效的附魔资源位置", enchantment.enchantment_id),
                enchantment.span,
            ));
        }
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
    if !valid_resource_location(&query.entity_type) {
        diagnostics.push(Diagnostic::new(
            format!("`{}` 不是有效的实体类型资源位置", query.entity_type),
            query.span,
        ));
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
    if let Some(item) = &query.item {
        if item.slot != "contents" {
            diagnostics.push(Diagnostic::new(
                format!(
                    "Minecraft 26.3 的首版类型化物品查询只支持 contents 槽，实际为 `{}`",
                    item.slot
                ),
                item.span,
            ));
        }
        if !valid_resource_location(&item.item_id) {
            diagnostics.push(Diagnostic::new(
                format!("`{}` 不是有效的物品资源位置", item.item_id),
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
