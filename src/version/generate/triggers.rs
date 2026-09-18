use std::collections::BTreeMap;
use std::path::Path;

use serde_json::{Map, Value, json};

use super::SOURCE_DIR;
use super::extract::{Extractor, render_json};
use super::text::matching_paren;

/// 提取 `advancement_triggers.json`：26.3 进度触发器的条件字段表。
///
/// 触发器名与类名取自 `CriteriaTriggers.java` 的注册调用；每个触发器类的
/// `TriggerInstance` 用 `RecordCodecBuilder` 声明条件字段，字段名取自
/// `.optionalFieldOf("…")`，字段粗类型取自紧邻的编解码表达式。没有条件
/// 字段的触发器（例如 `impossible`）不写入快照，校验时跳过。
///
/// 26.3 的战利品条件判定键由旧版本的 `condition` 改成了 `type`，这里把
/// `LootItemCondition` 字段单独分类，供校验器给出针对性的诊断。
pub fn generate_triggers(root: &Path) -> Result<String, String> {
    let mut extractor = Extractor::new(root);
    let registry = extractor.read("net/minecraft/advancements/triggers/CriteriaTriggers.java")?;
    let registered = registered_triggers(&registry);
    if registered.is_empty() {
        return Err("没有解析出任何触发器注册，触发器提取逻辑需要更新".into());
    }
    let mut triggers: BTreeMap<String, Value> = BTreeMap::new();
    let mut cache: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
    for (id, class) in registered {
        let fields = match cache.get(&class) {
            Some(fields) => fields.clone(),
            None => {
                let path = format!("net/minecraft/advancements/triggers/{class}.java");
                let text = extractor.read(&path)?;
                let fields = trigger_fields(&text);
                cache.insert(class, fields.clone());
                fields
            }
        };
        if fields.is_empty() {
            continue;
        }
        triggers.insert(
            id,
            serde_json::to_value(&fields).map_err(|error| error.to_string())?,
        );
    }
    let mut root_object = Map::new();
    root_object.insert("source".into(), json!(SOURCE_DIR));
    root_object.insert("digest".into(), json!(extractor.finish()));
    root_object.insert(
        "triggers".into(),
        Value::Object(triggers.into_iter().collect()),
    );
    render_json(&Value::Object(root_object))
}

/// `CriteriaTriggers.java` 里的 `register("id", new XxxTrigger())` 对。
pub(super) fn registered_triggers(text: &str) -> Vec<(String, String)> {
    let mut pairs = Vec::new();
    let mut rest = text;
    while let Some(index) = rest.find("register(\"") {
        let after = &rest[index + "register(\"".len()..];
        let Some(end) = after.find('"') else {
            break;
        };
        let id = after[..end].to_owned();
        // 注册调用都很短，只在紧随其后的窗口里找 `new 类名`，避免跨行误取。
        let window = &after[..after.len().min(240)];
        if let Some(new_index) = window.find("new ") {
            let class: String = window[new_index + "new ".len()..]
                .chars()
                .take_while(|character| character.is_ascii_alphanumeric() || *character == '_')
                .collect();
            if !id.is_empty() && !class.is_empty() {
                pairs.push((id, class));
            }
        }
        rest = after;
    }
    pairs
}

/// 一个触发器类里的条件字段表：字段名 → 粗类型。
///
/// 只扫描该类 `TriggerInstance` 自己的 CODEC 区域，避免把嵌套记录
/// （例如 `InventoryChangeTrigger.TriggerInstance.Slots`）的字段混进来；
/// 必填的 `fieldOf` 与可选的 `optionalFieldOf` 都算条件字段。
pub(super) fn trigger_fields(text: &str) -> BTreeMap<String, String> {
    let region = trigger_codec_region(text).unwrap_or(text);
    let mut fields = BTreeMap::new();
    for marker in [".optionalFieldOf(\"", ".fieldOf(\""] {
        let mut rest = region;
        while let Some(index) = rest.find(marker) {
            let after = &rest[index + marker.len()..];
            let Some(end) = after.find('"') else {
                break;
            };
            let name = after[..end].to_owned();
            let codec = codec_before(rest, index);
            if !name.is_empty() {
                fields.insert(name, codec_kind(codec).to_owned());
            }
            rest = after;
        }
    }
    fields
}

/// `TriggerInstance` 自己的 `RecordCodecBuilder.create(...)` 区域。
pub(super) fn trigger_codec_region(text: &str) -> Option<&str> {
    let marker = "TriggerInstance> CODEC = RecordCodecBuilder.create(";
    let start = text.find(marker)?;
    matching_paren(text, start + marker.len() - 1)
}

/// 取 `.optionalFieldOf` 之前、括号配平的编解码表达式。
///
/// 表达式可能含括号（`RegistryCodecs.holderSet(Registries.BLOCK)`），所以
/// 不能简单按逗号切分：从调用点向左回溯，跳过成对括号，在深度 0 的逗号
/// 或 `group(` 的左括号处停下。
pub(super) fn codec_before(text: &str, index: usize) -> &str {
    let bytes = text.as_bytes();
    let mut depth = 0i32;
    let mut position = index;
    while position > 0 {
        position -= 1;
        match bytes[position] {
            b')' => depth += 1,
            b'(' => {
                depth -= 1;
                if depth < 0 {
                    return text[position + 1..index].trim();
                }
            }
            b',' if depth == 0 => return text[position + 1..index].trim(),
            _ => {}
        }
    }
    text[..index].trim()
}

/// 编解码表达式的粗类型；识别不了的归为 `other`，校验时只看字段名。
pub(super) fn codec_kind(codec: &str) -> &'static str {
    if codec.contains("LootItemCondition") {
        "loot_condition"
    } else if codec.contains("ItemPredicate") {
        "item_predicate"
    } else if codec.contains("EntityPredicate") {
        "entity_predicate"
    } else if codec.contains("DamageSourcePredicate") {
        "damage_source"
    } else if codec.contains("StatePropertiesPredicate") {
        "state_properties"
    } else if codec.contains("holderSet")
        || codec.contains("RegistryCodecs.holder")
        || codec.contains("LIST_CODEC")
    {
        "id_or_list"
    } else if codec.contains("ResourceKey") || codec.contains("Identifier.CODEC") {
        "id"
    } else if codec.contains("MinMaxBounds") {
        "range"
    } else {
        "other"
    }
}
