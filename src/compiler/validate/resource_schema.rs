//! Checks for JSON resource fields whose 26.3 codecs reject malformed values.

use serde_json::{Map, Value};

use crate::ast::Span;
use crate::diagnostic::Diagnostic;

use super::rules::valid_resource_location;

pub(super) fn validate_resource(
    kind: &str,
    name: &str,
    value: &Value,
    span: Span,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match kind {
        "predicate" => validate_predicate(name, value, span, diagnostics),
        "recipe" => validate_recipe(name, value, span, diagnostics),
        "loot_table" => validate_loot_table(name, value, span, diagnostics),
        tag if tag.starts_with("tags/") => validate_tag(name, value, span, diagnostics),
        _ => {}
    }
}

fn validate_tag(name: &str, value: &Value, span: Span, diagnostics: &mut Vec<Diagnostic>) {
    let Some(object) = value.as_object() else {
        report(name, "标签资源需要 JSON 对象", span, diagnostics);
        return;
    };
    if object
        .get("replace")
        .is_some_and(|value| !value.is_boolean())
    {
        report(name, "标签 replace 必须是布尔值", span, diagnostics);
    }
    let Some(entries) = object.get("values").and_then(Value::as_array) else {
        report(name, "标签缺少 values 数组", span, diagnostics);
        return;
    };
    for (index, entry) in entries.iter().enumerate() {
        let id = if let Some(id) = entry.as_str() {
            Some(id)
        } else if let Some(entry) = entry.as_object() {
            if entry
                .get("required")
                .is_some_and(|value| !value.is_boolean())
            {
                report(
                    name,
                    &format!("标签第 {} 项的 required 必须是布尔值", index + 1),
                    span,
                    diagnostics,
                );
            }
            entry.get("id").and_then(Value::as_str)
        } else {
            None
        };
        if !id.is_some_and(|id| valid_resource_location(id.strip_prefix('#').unwrap_or(id))) {
            report(
                name,
                &format!("标签第 {} 项需要有效的 id 或 #标签引用", index + 1),
                span,
                diagnostics,
            );
        }
    }
}

fn validate_predicate(name: &str, value: &Value, span: Span, diagnostics: &mut Vec<Diagnostic>) {
    // RegistryDataLoader reads the direct codec at the resource root. Holder
    // references are only accepted in nested fields such as inverted.term.
    let Some(object) = value.as_object() else {
        report(
            name,
            "predicate 需要带 type 的对象，资源正文不能写引用字符串",
            span,
            diagnostics,
        );
        return;
    };
    let Some(kind) = resource_type(object, "loot_condition_type", name, span, diagnostics) else {
        return;
    };
    match kind.as_str() {
        "minecraft:random_chance" => {
            if !object.get("chance").is_some_and(|value| {
                value.as_f64().is_some_and(|number| number.is_finite())
                    || value.is_object()
                    || value.as_str().is_some_and(valid_json_id)
            }) {
                report(
                    name,
                    "random_chance 需要数字、浮点提供器对象或引用 chance",
                    span,
                    diagnostics,
                );
            }
        }
        "minecraft:inverted" => {
            if let Some(term) = object.get("term") {
                validate_predicate_reference(name, term, span, diagnostics);
            } else {
                report(name, "inverted 谓词缺少 term", span, diagnostics);
            }
        }
        "minecraft:any_of" | "minecraft:all_of" => match object.get("terms") {
            Some(Value::Array(terms)) => {
                for term in terms {
                    validate_predicate_reference(name, term, span, diagnostics);
                }
            }
            Some(Value::String(tag)) if tag.starts_with('#') => {
                if !valid_json_id(&tag[1..]) {
                    report(name, "组合谓词 terms 的标签引用无效", span, diagnostics);
                }
            }
            Some(term) => validate_predicate_reference(name, term, span, diagnostics),
            None => report(name, "组合谓词缺少 terms", span, diagnostics),
        },
        _ => {}
    }
}

fn validate_predicate_reference(
    name: &str,
    value: &Value,
    span: Span,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let Some(reference) = value.as_str() {
        if !valid_json_id(reference) {
            report(name, "predicate 引用需要有效资源位置", span, diagnostics);
        }
    } else {
        validate_predicate(name, value, span, diagnostics);
    }
}

fn validate_recipe(name: &str, value: &Value, span: Span, diagnostics: &mut Vec<Diagnostic>) {
    let Some(object) = value.as_object() else {
        report(name, "recipe 需要 JSON 对象", span, diagnostics);
        return;
    };
    let Some(kind) = resource_type(object, "recipe_serializer", name, span, diagnostics) else {
        return;
    };
    if kind == "minecraft:crafting_shapeless" {
        if !object
            .get("ingredients")
            .and_then(Value::as_array)
            .is_some_and(|items| {
                (1..=9).contains(&items.len()) && items.iter().all(valid_ingredient)
            })
        {
            report(
                name,
                "crafting_shapeless 需要 1 到 9 个物品 ID 或标签组成的 ingredients 数组",
                span,
                diagnostics,
            );
        }
        validate_recipe_result(name, object.get("result"), span, diagnostics);
    } else if kind == "minecraft:crafting_shaped" {
        if !object.contains_key("pattern") || !object.contains_key("key") {
            report(
                name,
                "crafting_shaped 需要 pattern 与 key",
                span,
                diagnostics,
            );
        }
        validate_recipe_result(name, object.get("result"), span, diagnostics);
    }
}

fn validate_recipe_result(
    name: &str,
    value: Option<&Value>,
    span: Span,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let valid = value.and_then(Value::as_str).is_some_and(valid_item_id)
        || value.and_then(Value::as_object).is_some_and(|result| {
            result
                .get("id")
                .and_then(Value::as_str)
                .is_some_and(valid_item_id)
                && result.get("count").is_none_or(|count| {
                    count
                        .as_u64()
                        .is_some_and(|count| (1..=99).contains(&count))
                })
        });
    if !valid {
        report(
            name,
            "配方 result 需要有效物品 id，count 可选且必须在 1 到 99 之间",
            span,
            diagnostics,
        );
    }
}

fn valid_ingredient(value: &Value) -> bool {
    value.as_str().is_some_and(|id| {
        id.strip_prefix('#')
            .map_or_else(|| valid_item_id(id), valid_json_id)
    }) || value.as_array().is_some_and(|ids| {
        !ids.is_empty() && ids.iter().all(|id| id.as_str().is_some_and(valid_item_id))
    })
}

fn valid_item_id(id: &str) -> bool {
    let id = canonical_json_id(id);
    valid_resource_location(&id)
        && id != "minecraft:air"
        && crate::version::snapshot::snapshot().registry_contains("item", &id) == Some(true)
}

fn canonical_json_id(id: &str) -> String {
    if id.contains(':') {
        id.to_owned()
    } else {
        format!("minecraft:{id}")
    }
}

fn valid_json_id(id: &str) -> bool {
    !id.is_empty() && valid_resource_location(&canonical_json_id(id))
}

fn validate_loot_table(name: &str, value: &Value, span: Span, diagnostics: &mut Vec<Diagnostic>) {
    let Some(object) = value.as_object() else {
        report(name, "loot_table 需要 JSON 对象", span, diagnostics);
        return;
    };
    if let Some(pools) = object.get("pools") {
        let Some(pools) = pools.as_array() else {
            report(name, "loot_table 的 pools 必须是数组", span, diagnostics);
            return;
        };
        for (index, pool) in pools.iter().enumerate() {
            if !pool.as_object().is_some_and(|pool| {
                pool.get("entries").is_some_and(Value::is_array)
                    && pool.get("rolls").is_some_and(|rolls| {
                        rolls.as_i64().is_some()
                            || rolls.is_object()
                            || rolls.as_str().is_some_and(valid_json_id)
                    })
            }) {
                report(
                    name,
                    &format!(
                        "loot_table 的第 {} 个池需要 entries 数组和 rolls",
                        index + 1
                    ),
                    span,
                    diagnostics,
                );
            }
        }
    }
}

fn resource_type(
    object: &Map<String, Value>,
    registry: &str,
    name: &str,
    span: Span,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<String> {
    let Some(kind) = object.get("type").and_then(Value::as_str) else {
        report(name, "资源缺少字符串 type 判别键", span, diagnostics);
        return None;
    };
    let canonical = if kind.contains(':') {
        kind.to_owned()
    } else {
        format!("minecraft:{kind}")
    };
    if crate::version::snapshot::snapshot().registry_contains(registry, &canonical) != Some(true) {
        report(
            name,
            &format!("type `{kind}` 未在 Minecraft 26.3 的 {registry} 注册表中"),
            span,
            diagnostics,
        );
        return None;
    }
    Some(canonical)
}

fn report(name: &str, detail: &str, span: Span, diagnostics: &mut Vec<Diagnostic>) {
    diagnostics.push(Diagnostic::new(format!("资源 `{name}`：{detail}"), span));
}
