//! 进度 JSON 生成：把结构化声明写为
//! `data/<命名空间>/advancement/<名称>.json`。
//!
//! 输出经过规范化：`frame`、`show_toast`、`announce_to_chat`、`hidden`
//! 只在偏离原版默认值时写出，`requirements` 总是显式生成，因此同一份
//! 声明的产物稳定可复现。

use std::collections::HashMap;

use serde_json::{Map, Value, json};

use crate::ast::*;

use super::emit::reference_id;

/// 生成一份进度文件内容。
pub(super) fn advancement_json(
    namespace: &str,
    advancement: &AdvancementDecl,
    item_stacks: &HashMap<&str, &ItemStackDecl>,
) -> String {
    let mut root = Map::new();
    if let Some(parent) = &advancement.parent {
        root.insert(
            "parent".to_owned(),
            Value::String(reference_id(namespace, parent)),
        );
    }
    if let Some(display) = &advancement.display {
        root.insert("display".to_owned(), display_json(display, item_stacks));
    }

    let mut criteria = Map::new();
    for criterion in &advancement.criteria {
        let mut entry = Map::new();
        entry.insert(
            "trigger".to_owned(),
            Value::String(format!("minecraft:{}", trigger_name(&criterion.trigger))),
        );
        if let Some(conditions) = &criterion.conditions {
            let crate::ast::AdvancementConditions::Parsed(value) = conditions else {
                unreachable!("semantic validation guarantees valid conditions JSON");
            };
            entry.insert("conditions".to_owned(), value.clone());
        }
        criteria.insert(criterion.name.clone(), Value::Object(entry));
    }
    root.insert("criteria".to_owned(), Value::Object(criteria));

    let requirements = match advancement.requirements {
        // Minecraft ANDs the outer requirement groups and ORs the criterion
        // names inside each group. `all` therefore needs one singleton group
        // per criterion, while `any` needs one group containing every name.
        AdvancementRequirements::All => advancement
            .criteria
            .iter()
            .map(|criterion| vec![criterion.name.clone()])
            .collect(),
        AdvancementRequirements::Any => vec![
            advancement
                .criteria
                .iter()
                .map(|criterion| criterion.name.clone())
                .collect::<Vec<_>>(),
        ],
    };
    root.insert(
        "requirements".to_owned(),
        serde_json::to_value(requirements).expect("criterion names serialize as strings"),
    );

    if let Some(reward) = &advancement.reward {
        let mut object = Map::new();
        if let Some(experience) = reward.experience
            && experience != 0
        {
            object.insert("experience".to_owned(), json!(experience));
        }
        if let Some(function) = &reward.function {
            object.insert(
                "function".to_owned(),
                Value::String(reference_id(namespace, function)),
            );
        }
        if !reward.loot.is_empty() {
            object.insert(
                "loot".to_owned(),
                Value::Array(
                    reward
                        .loot
                        .iter()
                        .map(|reference| Value::String(reference_id(namespace, reference)))
                        .collect(),
                ),
            );
        }
        if !reward.recipes.is_empty() {
            object.insert(
                "recipes".to_owned(),
                Value::Array(
                    reward
                        .recipes
                        .iter()
                        .map(|reference| Value::String(reference_id(namespace, reference)))
                        .collect(),
                ),
            );
        }
        if !object.is_empty() {
            root.insert("rewards".to_owned(), Value::Object(object));
        }
    }

    serde_json::to_string_pretty(&Value::Object(root)).expect("a JSON value always serializes")
        + "\n"
}

/// 去掉可选的 `minecraft:` 前缀，输出统一补回。
fn trigger_name(value: &str) -> &str {
    value.strip_prefix("minecraft:").unwrap_or(value)
}

fn display_json(
    display: &AdvancementDisplay,
    item_stacks: &HashMap<&str, &ItemStackDecl>,
) -> Value {
    let item = item_stacks
        .get(display.icon.as_str())
        .copied()
        .expect("semantic validation guarantees the icon item exists");
    let mut object = Map::new();
    object.insert("icon".to_owned(), item_stack_template_json(item));
    object.insert("title".to_owned(), text_component(&display.title));
    object.insert(
        "description".to_owned(),
        text_component(&display.description),
    );
    if let Some(background) = &display.background {
        object.insert("background".to_owned(), Value::String(background.clone()));
    }
    if display.frame != AdvancementFrame::Task {
        object.insert(
            "frame".to_owned(),
            Value::String(display.frame.as_str().to_owned()),
        );
    }
    if display.show_toast == Some(false) {
        object.insert("show_toast".to_owned(), Value::Bool(false));
    }
    if display.announce_to_chat == Some(false) {
        object.insert("announce_to_chat".to_owned(), Value::Bool(false));
    }
    if display.hidden == Some(true) {
        object.insert("hidden".to_owned(), Value::Bool(true));
    }
    Value::Object(object)
}

/// 纯文本组件；`tellraw` 与进度展示共用 `{"text": ...}` 形状。
fn text_component(text: &str) -> Value {
    json!({ "text": text })
}

/// 物品定义到 `ItemStackTemplate` JSON 的转换：`id`、`count` 与组件补丁。
fn item_stack_template_json(item: &ItemStackDecl) -> Value {
    let mut object = Map::new();
    object.insert("id".to_owned(), Value::String(item.item_id.clone()));
    if item.count != 1 {
        object.insert("count".to_owned(), json!(item.count));
    }
    let mut components = Map::new();
    if let Some(NbtValue {
        kind: NbtValueKind::Compound(entries),
        ..
    }) = &item.components
    {
        for entry in entries {
            components.insert(entry.key.clone(), nbt_json(&entry.value));
        }
    }
    if let Some(custom_name) = &item.custom_name {
        components.insert(
            "minecraft:custom_name".to_owned(),
            text_component(custom_name),
        );
    }
    if let Some(item_name) = &item.item_name {
        components.insert("minecraft:item_name".to_owned(), text_component(item_name));
    }
    if !item.lore.is_empty() {
        components.insert(
            "minecraft:lore".to_owned(),
            Value::Array(item.lore.iter().map(|line| text_component(line)).collect()),
        );
    }
    for (component, enchantments) in [
        ("minecraft:enchantments", &item.enchantments),
        ("minecraft:stored_enchantments", &item.stored_enchantments),
    ] {
        if enchantments.is_empty() {
            continue;
        }
        let mut levels = Map::new();
        for enchantment in enchantments {
            levels.insert(enchantment.enchantment_id.clone(), json!(enchantment.level));
        }
        components.insert(component.to_owned(), Value::Object(levels));
    }
    if let Some(damage) = item.damage {
        components.insert("minecraft:damage".to_owned(), json!(damage));
    }
    if let Some(max_damage) = item.max_damage {
        components.insert("minecraft:max_damage".to_owned(), json!(max_damage));
    }
    if let Some(max_stack_size) = item.max_stack_size {
        components.insert("minecraft:max_stack_size".to_owned(), json!(max_stack_size));
    }
    if let Some(rarity) = item.rarity {
        components.insert(
            "minecraft:rarity".to_owned(),
            Value::String(rarity.as_str().to_owned()),
        );
    }
    if let Some(item_model) = &item.item_model {
        components.insert(
            "minecraft:item_model".to_owned(),
            Value::String(item_model.clone()),
        );
    }
    if let Some(dyed_color) = item.dyed_color {
        components.insert("minecraft:dyed_color".to_owned(), json!(dyed_color));
    }
    if let Some(glint) = item.enchantment_glint_override {
        components.insert(
            "minecraft:enchantment_glint_override".to_owned(),
            Value::Bool(glint),
        );
    }
    if item.unbreakable {
        components.insert("minecraft:unbreakable".to_owned(), json!({}));
    }
    if let Some(custom_data) = &item.custom_data {
        components.insert("minecraft:custom_data".to_owned(), nbt_json(custom_data));
    }
    if !components.is_empty() {
        object.insert("components".to_owned(), Value::Object(components));
    }
    Value::Object(object)
}

/// NBT 值到 JSON 的转换：数据包 JSON 里的 `custom_data` 直接写成 JSON 结构。
pub(super) fn nbt_json(value: &NbtValue) -> Value {
    match &value.kind {
        NbtValueKind::Byte(value) => json!(value),
        NbtValueKind::Short(value) => json!(value),
        NbtValueKind::Int(value) => json!(value),
        NbtValueKind::Long(value) => json!(value),
        NbtValueKind::Float(value) => json!(value),
        NbtValueKind::Double(value) => json!(value),
        NbtValueKind::String(value) => Value::String(value.clone()),
        NbtValueKind::List(values) => Value::Array(values.iter().map(nbt_json).collect()),
        NbtValueKind::Compound(entries) => Value::Object(
            entries
                .iter()
                .map(|entry| (entry.key.clone(), nbt_json(&entry.value)))
                .collect(),
        ),
        NbtValueKind::ByteArray(values) => json!(values),
        NbtValueKind::IntArray(values) => json!(values),
        NbtValueKind::LongArray(values) => json!(values),
    }
}
