//! Field checks for the 26.3 data component predicate codecs.

use crate::ast::{NbtEntry, NbtValue, NbtValueKind};
use crate::diagnostic::Diagnostic;

use super::super::registry::validate_id_or_tag;

pub(super) fn validate_predicate_value(
    id: &str,
    value: &NbtValue,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match id {
        "minecraft:count" => validate_range("count", value, diagnostics),
        "minecraft:damage" => validate_damage(value, diagnostics),
        "minecraft:potion_contents" => validate_potions(value, diagnostics),
        "minecraft:enchantments" | "minecraft:stored_enchantments" => {
            validate_enchantments(id, value, diagnostics);
        }
        "minecraft:trim" => validate_trim(value, diagnostics),
        "minecraft:firework_explosion" => validate_firework(value, diagnostics),
        "minecraft:written_book_content" => validate_written_book(value, diagnostics),
        "minecraft:jukebox_playable" => {
            validate_record(
                id,
                value,
                &["song"],
                diagnostics,
                |field, value, diagnostics| {
                    if field == "song" {
                        validate_holder_set("jukebox_song", "唱片歌曲", value, diagnostics);
                    }
                },
            );
        }
        "minecraft:villager/variant" => {
            validate_holder_set("villager_type", "村民类型", value, diagnostics);
        }
        "minecraft:custom_data" if !matches!(value.kind, NbtValueKind::Compound(_)) => {
            diagnostics.push(Diagnostic::new(
                "custom_data 子谓词需要复合 NBT",
                value.span,
            ));
        }
        _ => {}
    }
}

fn validate_damage(value: &NbtValue, diagnostics: &mut Vec<Diagnostic>) {
    validate_record(
        "minecraft:damage",
        value,
        &["damage", "durability"],
        diagnostics,
        validate_range,
    );
}

fn validate_potions(value: &NbtValue, diagnostics: &mut Vec<Diagnostic>) {
    validate_record(
        "minecraft:potion_contents",
        value,
        &["potions", "effects"],
        diagnostics,
        |field, value, diagnostics| match field {
            "potions" => validate_holder_set("potion", "药水", value, diagnostics),
            "effects" => validate_effect_collection(value, diagnostics),
            _ => {}
        },
    );
}

fn validate_effect_collection(value: &NbtValue, diagnostics: &mut Vec<Diagnostic>) {
    validate_record(
        "potion_contents.effects",
        value,
        &["contains", "count", "size"],
        diagnostics,
        |field, value, diagnostics| match field {
            "size" => validate_range("effects.size", value, diagnostics),
            "contains" | "count" if !matches!(value.kind, NbtValueKind::List(_)) => {
                diagnostics.push(Diagnostic::new(
                    format!("effects.{field} 需要列表"),
                    value.span,
                ));
            }
            _ => {}
        },
    );
}

fn validate_enchantments(id: &str, value: &NbtValue, diagnostics: &mut Vec<Diagnostic>) {
    let NbtValueKind::List(values) = &value.kind else {
        diagnostics.push(Diagnostic::new(
            format!("子谓词 `{id}` 需要附魔条件列表"),
            value.span,
        ));
        return;
    };
    for entry in values {
        validate_record(
            id,
            entry,
            &["enchantments", "levels"],
            diagnostics,
            |field, value, diagnostics| match field {
                "enchantments" => {
                    validate_holder_set("enchantment", "附魔", value, diagnostics);
                }
                "levels" => validate_range("附魔等级", value, diagnostics),
                _ => {}
            },
        );
    }
}

fn validate_trim(value: &NbtValue, diagnostics: &mut Vec<Diagnostic>) {
    validate_record(
        "minecraft:trim",
        value,
        &["material", "pattern"],
        diagnostics,
        |field, value, diagnostics| {
            let (registry, label) = if field == "material" {
                ("trim_material", "盔甲纹饰材料")
            } else {
                ("trim_pattern", "盔甲纹饰图案")
            };
            validate_holder_set(registry, label, value, diagnostics);
        },
    );
}

fn validate_firework(value: &NbtValue, diagnostics: &mut Vec<Diagnostic>) {
    validate_record(
        "minecraft:firework_explosion",
        value,
        &["shape", "has_twinkle", "has_trail"],
        diagnostics,
        |field, value, diagnostics| {
            let valid = if field == "shape" {
                matches!(&value.kind, NbtValueKind::String(shape) if matches!(shape.as_str(), "small_ball" | "large_ball" | "star" | "creeper" | "burst"))
            } else {
                is_boolean(value)
            };
            if !valid {
                diagnostics.push(Diagnostic::new(
                    format!("firework_explosion.{field} 的值不符合 26.3 codec"),
                    value.span,
                ));
            }
        },
    );
}

fn validate_written_book(value: &NbtValue, diagnostics: &mut Vec<Diagnostic>) {
    validate_record(
        "minecraft:written_book_content",
        value,
        &["pages", "author", "title", "generation", "resolved"],
        diagnostics,
        |field, value, diagnostics| match field {
            "generation" => validate_range("书本代数", value, diagnostics),
            "author" | "title" if !matches!(value.kind, NbtValueKind::String(_)) => {
                diagnostics.push(Diagnostic::new(
                    format!("written_book_content.{field} 需要字符串"),
                    value.span,
                ));
            }
            "resolved" if !is_boolean(value) => {
                diagnostics.push(Diagnostic::new(
                    "written_book_content.resolved 需要布尔值",
                    value.span,
                ));
            }
            "pages" => validate_record(
                "written_book_content.pages",
                value,
                &["contains", "count", "size"],
                diagnostics,
                |field, value, diagnostics| {
                    if field == "size" {
                        validate_range("pages.size", value, diagnostics);
                    } else if !matches!(value.kind, NbtValueKind::List(_)) {
                        diagnostics.push(Diagnostic::new(
                            format!("pages.{field} 需要列表"),
                            value.span,
                        ));
                    }
                },
            ),
            _ => {}
        },
    );
}

fn validate_holder_set(
    registry: &str,
    label: &str,
    value: &NbtValue,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match &value.kind {
        NbtValueKind::String(id) => {
            validate_id_or_tag(registry, label, id, value.span, diagnostics);
        }
        NbtValueKind::List(ids) if !ids.is_empty() => {
            for id in ids {
                if let NbtValueKind::String(name) = &id.kind {
                    validate_id_or_tag(registry, label, name, id.span, diagnostics);
                } else {
                    diagnostics.push(Diagnostic::new(
                        format!("{label}列表的元素需要资源位置字符串"),
                        id.span,
                    ));
                }
            }
        }
        _ => diagnostics.push(Diagnostic::new(
            format!("{label}需要资源位置、#标签或非空资源位置列表"),
            value.span,
        )),
    }
}

fn validate_range(label: &str, value: &NbtValue, diagnostics: &mut Vec<Diagnostic>) {
    if matches!(value.kind, NbtValueKind::Int(_)) {
        return;
    }
    let NbtValueKind::Compound(entries) = &value.kind else {
        diagnostics.push(Diagnostic::new(
            format!("子谓词 `{label}` 的范围需要整数或 min/max 整数字段"),
            value.span,
        ));
        return;
    };
    for entry in entries {
        if !matches!(entry.key.as_str(), "min" | "max") {
            diagnostics.push(Diagnostic::new(
                format!("子谓词 `{label}` 的范围没有字段 `{}`", entry.key),
                entry.key_span,
            ));
        } else if !matches!(entry.value.kind, NbtValueKind::Int(_)) {
            diagnostics.push(Diagnostic::new(
                format!("子谓词 `{label}` 的 `{}` 需要整数", entry.key),
                entry.value.span,
            ));
        }
    }
    let bound = |name| {
        entries.iter().find_map(|entry| {
            if entry.key == name
                && let NbtValueKind::Int(number) = entry.value.kind
            {
                return Some(number);
            }
            None
        })
    };
    if bound("min")
        .zip(bound("max"))
        .is_some_and(|(min, max)| min > max)
    {
        diagnostics.push(Diagnostic::new(
            format!("{label} 子谓词的 min 不能大于 max"),
            value.span,
        ));
    }
}

fn validate_record(
    id: &str,
    value: &NbtValue,
    fields: &[&str],
    diagnostics: &mut Vec<Diagnostic>,
    mut validate_field: impl FnMut(&str, &NbtValue, &mut Vec<Diagnostic>),
) {
    let NbtValueKind::Compound(entries) = &value.kind else {
        diagnostics.push(Diagnostic::new(
            format!("子谓词 `{id}` 需要复合字段"),
            value.span,
        ));
        return;
    };
    for NbtEntry {
        key,
        key_span,
        value,
    } in entries
    {
        if fields.contains(&key.as_str()) {
            validate_field(key, value, diagnostics);
        } else {
            diagnostics.push(Diagnostic::new(
                format!("子谓词 `{id}` 没有字段 `{key}`"),
                *key_span,
            ));
        }
    }
}

fn is_boolean(value: &NbtValue) -> bool {
    matches!(value.kind, NbtValueKind::Byte(0 | 1))
}
