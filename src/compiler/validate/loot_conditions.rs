//! 两个入口共用 26.3 LootItemCondition 与 HolderSetCodec 的结构校验。
use super::rules::{canonical_resource_location, valid_resource_location};
use crate::diagnostic::Diagnostic;

pub(super) fn validate_list(
    label: &str,
    value: &serde_json::Value,
    span: crate::ast::Span,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(conditions) = value.as_array() else {
        diagnostics.push(Diagnostic::new(
            format!("`{label}` 需要战利品条件数组"),
            span,
        ));
        return;
    };
    for (index, condition) in conditions.iter().enumerate() {
        let item_label = format!("{label}[{}]", index + 1);
        validate_holder(&item_label, condition, span, diagnostics);
    }
}

pub(super) fn validate_holder(
    label: &str,
    value: &serde_json::Value,
    span: crate::ast::Span,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match value {
        serde_json::Value::String(reference) => {
            if !valid_resource_location(&canonical_resource_location(reference)) {
                diagnostics.push(Diagnostic::new(
                    format!("`{label}` 的谓词引用 `{reference}` 不是有效的资源位置"),
                    span,
                ));
            }
        }
        serde_json::Value::Object(condition) => {
            validate_object(label, condition, span, diagnostics);
        }
        _ => diagnostics.push(Diagnostic::new(
            format!("`{label}` 需要谓词资源字符串或带 `type` 的内联条件对象"),
            span,
        )),
    }
}

pub(super) fn validate_object(
    label: &str,
    condition: &serde_json::Map<String, serde_json::Value>,
    span: crate::ast::Span,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(raw_type) = condition.get("type").and_then(serde_json::Value::as_str) else {
        let message = if condition.contains_key("condition") {
            format!("`{label}` 使用了 `condition` 作判别键，26.3 已改为 `type`")
        } else {
            format!("`{label}` 的内联战利品条件缺少字符串 `type`；26.3 的判别键是 `type`")
        };
        diagnostics.push(Diagnostic::new(message, span));
        return;
    };

    let condition_type = canonical_resource_location(raw_type);
    if !super::registry::validate_static_id(
        "loot_condition_type",
        &format!("{label} 的战利品条件 type"),
        &condition_type,
        span,
        diagnostics,
    ) {
        return;
    }
    match condition_type.as_str() {
        "minecraft:random_chance" => {
            if !condition.get("chance").is_some_and(|value| {
                value.as_f64().is_some_and(|number| number.is_finite())
                    || value.is_object()
                    || value
                        .as_str()
                        .is_some_and(|id| valid_resource_location(&canonical_resource_location(id)))
            }) {
                diagnostics.push(Diagnostic::new(
                    format!("`{label}` 的 random_chance 需要数字、浮点提供器对象或引用 chance"),
                    span,
                ));
            }
        }
        "minecraft:inverted" => match condition.get("term") {
            Some(term) => validate_holder(&format!("{label}.term"), term, span, diagnostics),
            None => diagnostics.push(Diagnostic::new(
                format!("`{label}` 的 inverted 条件缺少 `term`"),
                span,
            )),
        },
        "minecraft:all_of" | "minecraft:any_of" => {
            validate_loot_condition_terms(label, condition.get("terms"), span, diagnostics);
        }
        _ => {}
    }
}

fn validate_loot_condition_terms(
    label: &str,
    terms: Option<&serde_json::Value>,
    span: crate::ast::Span,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match terms {
        Some(serde_json::Value::Array(terms)) => {
            for (index, term) in terms.iter().enumerate() {
                validate_holder(
                    &format!("{label}.terms[{}]", index + 1),
                    term,
                    span,
                    diagnostics,
                );
            }
        }
        Some(serde_json::Value::String(tag)) if tag.starts_with('#') => {
            if !valid_resource_location(&canonical_resource_location(&tag[1..])) {
                diagnostics.push(Diagnostic::new(
                    format!("`{label}` 的组合条件 terms 标签引用 `{tag}` 无效"),
                    span,
                ));
            }
        }
        // HolderSetCodec uses compactListCodec when alwaysUseList is false:
        // a single predicate holder is accepted alongside arrays and #tags.
        Some(term) => validate_holder(&format!("{label}.terms"), term, span, diagnostics),
        None => diagnostics.push(Diagnostic::new(
            format!("`{label}` 的组合条件缺少 `terms`"),
            span,
        )),
    }
}
