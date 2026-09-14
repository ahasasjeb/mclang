//! 把结构化 AST 片段格式化为 Minecraft 命令参数、SNBT 和 JSON 文本。

use crate::ast::{EntityQueryDecl, ItemEnchantment, ItemStackDecl, MessageTarget};

pub(super) fn entity_query_clause(query: &EntityQueryDecl) -> String {
    let mut selector = vec![format!("type={}", query.entity_type)];
    selector.extend(query.tags.iter().map(|tag| format!("tag={tag}")));
    selector.extend(query.excluded_tags.iter().map(|tag| format!("tag=!{tag}")));
    if let Some(sort) = query.sort {
        selector.push(format!("sort={}", sort.as_str()));
    }
    if let Some(limit) = query.limit {
        selector.push(format!("limit={limit}"));
    }
    if let Some(within) = query.within {
        selector.push(format!("distance=..{within}"));
    }

    let mut clause = format!("as @e[{}] at @s", selector.join(","));
    if let Some(item) = &query.item {
        let mut components = Vec::new();
        if let Some(custom_name) = &item.custom_name {
            components.push(format!(
                "minecraft:custom_name={{text:{}}}",
                snbt_string(custom_name)
            ));
        }
        if let Some(count) = item.count {
            components.push(format!("minecraft:count={count}"));
        }
        let predicate = if components.is_empty() {
            item.item_id.clone()
        } else {
            format!("{}[{}]", item.item_id, components.join(","))
        };
        clause.push_str(&format!(" if items entity @s {} {predicate}", item.slot));
    }
    clause
}

pub(super) fn item_stack_argument(item: &ItemStackDecl) -> String {
    let mut components = Vec::new();
    if let Some(custom_name) = &item.custom_name {
        components.push(format!(
            "minecraft:custom_name={{text:{}}}",
            snbt_string(custom_name)
        ));
    }
    if let Some(item_name) = &item.item_name {
        components.push(format!(
            "minecraft:item_name={{text:{}}}",
            snbt_string(item_name)
        ));
    }
    if !item.lore.is_empty() {
        let lines = item
            .lore
            .iter()
            .map(|line| format!("{{text:{}}}", snbt_string(line)))
            .collect::<Vec<_>>()
            .join(",");
        components.push(format!("minecraft:lore=[{lines}]"));
    }
    append_enchantments_component(
        &mut components,
        "minecraft:enchantments",
        &item.enchantments,
    );
    append_enchantments_component(
        &mut components,
        "minecraft:stored_enchantments",
        &item.stored_enchantments,
    );
    if let Some(damage) = item.damage {
        components.push(format!("minecraft:damage={damage}"));
    }
    if let Some(max_damage) = item.max_damage {
        components.push(format!("minecraft:max_damage={max_damage}"));
    }
    if let Some(max_stack_size) = item.max_stack_size {
        components.push(format!("minecraft:max_stack_size={max_stack_size}"));
    }
    if let Some(rarity) = item.rarity {
        components.push(format!("minecraft:rarity=\"{}\"", rarity.as_str()));
    }
    if let Some(item_model) = &item.item_model {
        components.push(format!("minecraft:item_model={}", snbt_string(item_model)));
    }
    if let Some(dyed_color) = item.dyed_color {
        components.push(format!("minecraft:dyed_color={dyed_color}"));
    }
    if let Some(glint) = item.enchantment_glint_override {
        components.push(format!(
            "minecraft:enchantment_glint_override={}",
            if glint { "true" } else { "false" }
        ));
    }
    if item.unbreakable {
        components.push("minecraft:unbreakable={}".to_owned());
    }
    if components.is_empty() {
        item.item_id.clone()
    } else {
        format!("{}[{}]", item.item_id, components.join(","))
    }
}

fn append_enchantments_component(
    components: &mut Vec<String>,
    component_id: &str,
    enchantments: &[ItemEnchantment],
) {
    if enchantments.is_empty() {
        return;
    }
    let entries = enchantments
        .iter()
        .map(|enchantment| {
            format!(
                "{}:{}",
                snbt_string(&enchantment.enchantment_id),
                enchantment.level
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    components.push(format!("{component_id}={{{entries}}}"));
}

fn snbt_string(value: &str) -> String {
    let mut escaped = String::from("\"");
    for character in value.chars() {
        match character {
            '\"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            character => escaped.push(character),
        }
    }
    escaped.push('\"');
    escaped
}

pub(super) fn compile_message(target: &MessageTarget, text: &str, color: Option<&str>) -> String {
    let selector = match target {
        MessageTarget::All => "@a".to_owned(),
        MessageTarget::SelfEntity => "@s".to_owned(),
        MessageTarget::Nearest { within } => {
            format!("@a[sort=nearest,limit=1,distance=..{within}]")
        }
    };
    let mut component = serde_json::Map::new();
    component.insert(
        "text".to_owned(),
        serde_json::Value::String(text.to_owned()),
    );
    if let Some(color) = color {
        component.insert(
            "color".to_owned(),
            serde_json::Value::String(color.to_owned()),
        );
    }
    format!(
        "tellraw {selector} {}",
        serde_json::Value::Object(component)
    )
}

pub(super) fn pack_metadata(description: &str) -> String {
    format!(
        "{{\n  \"pack\": {{\n    \"description\": \"{}\",\n    \"min_format\": [121, 0],\n    \"max_format\": [121, 0]\n  }}\n}}\n",
        json_escape(description)
    )
}

pub(super) fn tag_json(values: &[String]) -> String {
    let values = values
        .iter()
        .map(|value| format!("    \"{}\"", json_escape(value)))
        .collect::<Vec<_>>()
        .join(",\n");
    format!("{{\n  \"values\": [\n{values}\n  ]\n}}\n")
}

fn json_escape(value: &str) -> String {
    let mut escaped = String::new();
    for character in value.chars() {
        match character {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            character if character < ' ' => {
                escaped.push_str(&format!("\\u{:04x}", character as u32))
            }
            character => escaped.push(character),
        }
    }
    escaped
}
