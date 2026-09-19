use super::registry::{validate_id, validate_id_or_tag};
use crate::{ast::*, diagnostic::Diagnostic};

pub(super) fn validate_predicate(predicate: &ItemPredicate, diagnostics: &mut Vec<Diagnostic>) {
    if predicate.item != "*" {
        validate_id_or_tag("item", "物品", &predicate.item, predicate.span, diagnostics);
    }
    for test in predicate.clauses.iter().flatten() {
        let registry = if matches!(test.kind, ItemComponentTestKind::Match(_)) {
            "data_component_predicate_type"
        } else {
            "data_component_type"
        };
        if test.id != "minecraft:count" {
            validate_id(registry, "组件条件", &test.id, test.span, diagnostics);
        }
    }
}

pub(super) fn validate_components(item: &ItemStackDecl, diagnostics: &mut Vec<Diagnostic>) {
    let Some(NbtValue {
        kind: NbtValueKind::Compound(entries),
        ..
    }) = &item.components
    else {
        return;
    };
    let mut seen = std::collections::HashSet::new();
    for (id, present) in [
        ("custom_name", item.custom_name.is_some()),
        ("item_name", item.item_name.is_some()),
        ("lore", !item.lore.is_empty()),
        ("enchantments", !item.enchantments.is_empty()),
        ("stored_enchantments", !item.stored_enchantments.is_empty()),
        ("damage", item.damage.is_some()),
        ("max_damage", item.max_damage.is_some()),
        ("max_stack_size", item.max_stack_size.is_some()),
        ("rarity", item.rarity.is_some()),
        ("item_model", item.item_model.is_some()),
        ("dyed_color", item.dyed_color.is_some()),
        (
            "enchantment_glint_override",
            item.enchantment_glint_override.is_some(),
        ),
        ("unbreakable", item.unbreakable),
        ("custom_data", item.custom_data.is_some()),
    ] {
        if present {
            seen.insert(format!("minecraft:{id}"));
        }
    }
    for entry in entries {
        let removed = entry.key.starts_with('!');
        let id = entry.key.trim_start_matches('!');
        validate_id(
            "data_component_type",
            "物品组件",
            id,
            entry.value.span,
            diagnostics,
        );
        if !seen.insert(id.to_owned()) {
            diagnostics.push(Diagnostic::new(
                format!("物品组件 `{id}` 重复赋值或与具名属性冲突"),
                entry.value.span,
            ));
        }
        if removed {
            if !matches!(&entry.value.kind, NbtValueKind::Compound(v) if v.is_empty()) {
                diagnostics.push(Diagnostic::new(
                    "删除组件使用 \"!minecraft:组件\": {}，不接受组件值",
                    entry.value.span,
                ));
            }
        } else {
            validate_component_value(id, &entry.value, diagnostics);
        }
    }
}

fn validate_component_value(id: &str, value: &NbtValue, diagnostics: &mut Vec<Diagnostic>) {
    let range = match id {
        "minecraft:max_stack_size" => Some((1, 99)),
        "minecraft:max_damage" => Some((1, i32::MAX)),
        "minecraft:damage" | "minecraft:repair_cost" => Some((0, i32::MAX)),
        _ => None,
    };
    if let Some((min, max)) = range
        && !matches!(value.kind, NbtValueKind::Int(v) if (min..=max).contains(&v))
    {
        diagnostics.push(Diagnostic::new(
            format!("组件 `{id}` 必须是 {min} 到 {max} 之间的整数"),
            value.span,
        ));
    }
}

pub(super) fn component_stack_size(item: &ItemStackDecl) -> Option<u32> {
    if let Some(NbtValue {
        kind: NbtValueKind::Compound(entries),
        ..
    }) = &item.components
    {
        for entry in entries {
            if entry.key == "minecraft:max_stack_size"
                && let NbtValueKind::Int(value) = entry.value.kind
            {
                return u32::try_from(value).ok();
            }
        }
    }
    item.max_stack_size
}
