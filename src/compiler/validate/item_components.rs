use super::registry::{validate_id, validate_id_or_tag};
use crate::{ast::*, diagnostic::Diagnostic};

mod predicate_values;
use predicate_values::validate_predicate_value;

pub(super) fn validate_predicate(predicate: &ItemPredicate, diagnostics: &mut Vec<Diagnostic>) {
    let base = predicate.item.split('[').next().unwrap_or(&predicate.item);
    if base != "*" {
        validate_id_or_tag("item", "物品", base, predicate.span, diagnostics);
    }
    if predicate.item.contains('[') && !super::items::valid_item_predicate(&predicate.item) {
        diagnostics.push(Diagnostic::new(
            "物品谓词的组件过滤器不完整；建议使用 item_predicate 结构化条件",
            predicate.span,
        ));
    }
    for test in predicate.clauses.iter().flatten() {
        let registry = if matches!(test.kind, ItemComponentTestKind::Match(_)) {
            "data_component_predicate_type"
        } else {
            "data_component_type"
        };
        let component_existence = matches!(test.kind, ItemComponentTestKind::Match(_))
            && crate::version::snapshot::snapshot()
                .registry_contains("data_component_type", &test.id)
                == Some(true);
        if test.id != "minecraft:count" && !component_existence {
            validate_id(registry, "组件条件", &test.id, test.span, diagnostics);
        }
        match &test.kind {
            ItemComponentTestKind::Present => {}
            ItemComponentTestKind::Equal(value) => {
                validate_component_value(&test.id, value, diagnostics)
            }
            ItemComponentTestKind::Match(value) => {
                validate_predicate_value(&test.id, value, diagnostics)
            }
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
        let id = entry.key.strip_prefix('!').unwrap_or(&entry.key);
        validate_id(
            "data_component_type",
            "物品组件",
            id,
            entry.value.span,
            diagnostics,
        );
        if matches!(
            id,
            "minecraft:creative_slot_lock"
                | "minecraft:additional_trade_cost"
                | "minecraft:map_post_processing"
        ) {
            diagnostics.push(Diagnostic::new(
                format!("物品组件 `{id}` 在 26.3 没有持久化 codec，不能写入数据包"),
                entry.key_span,
            ));
        }
        if !seen.insert(id.to_owned()) {
            diagnostics.push(Diagnostic::new(
                format!("物品组件 `{id}` 重复赋值或与具名属性冲突"),
                entry.value.span,
            ));
        }
        if removed {
            if !matches!(&entry.value.kind, NbtValueKind::Compound(v) if v.is_empty()) {
                diagnostics.push(Diagnostic::new(
                    "删除组件使用 \"!minecraft:组件\" = {};，不接受组件值",
                    entry.value.span,
                ));
            }
        } else {
            validate_component_value(id, &entry.value, diagnostics);
        }
    }
}

pub(super) fn validate_component_value(
    id: &str,
    value: &NbtValue,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let schema = match id {
        "minecraft:max_stack_size" => Some(ValueSchema::Integer(1, 99)),
        "minecraft:max_damage" => Some(ValueSchema::Integer(1, i32::MAX)),
        "minecraft:damage" | "minecraft:repair_cost" | "minecraft:map_id" => {
            Some(ValueSchema::Integer(0, i32::MAX))
        }
        "minecraft:enchantment_glint_override" => Some(ValueSchema::Boolean),
        "minecraft:unbreakable"
        | "minecraft:intangible_projectile"
        | "minecraft:glider"
        | "minecraft:waxed" => Some(ValueSchema::Unit),
        "minecraft:minimum_attack_charge" => Some(ValueSchema::Float(0.0, 1.0)),
        "minecraft:potion_duration_scale" => Some(ValueSchema::Float(0.0, f32::MAX as f64)),
        "minecraft:dyed_color" => Some(ValueSchema::RgbColor),
        "minecraft:item_model"
        | "minecraft:tooltip_style"
        | "minecraft:note_block_sound"
        | "minecraft:break_sound"
        | "minecraft:damage_type" => Some(ValueSchema::Resource),
        "minecraft:rarity" => Some(ValueSchema::Rarity),
        "minecraft:custom_data" | "minecraft:bucket_entity_data" => {
            Some(ValueSchema::Compound(&[]))
        }
        "minecraft:custom_name" | "minecraft:item_name" => Some(ValueSchema::Text),
        "minecraft:lore" => Some(ValueSchema::TextList),
        "minecraft:enchantments" | "minecraft:stored_enchantments" => {
            Some(ValueSchema::Enchantments)
        }
        "minecraft:food" => Some(ValueSchema::Compound(FOOD_FIELDS)),
        "minecraft:use_cooldown" => Some(ValueSchema::Compound(COOLDOWN_FIELDS)),
        "minecraft:use_effects" => Some(ValueSchema::Compound(USE_EFFECTS_FIELDS)),
        "minecraft:weapon" => Some(ValueSchema::Compound(WEAPON_FIELDS)),
        "minecraft:attack_range" => Some(ValueSchema::Compound(ATTACK_RANGE_FIELDS)),
        "minecraft:enchantable" => Some(ValueSchema::Compound(ENCHANTABLE_FIELDS)),
        _ => None,
    };
    if let Some(schema) = schema {
        validate_schema(id, value, schema, diagnostics);
    }
}

#[derive(Clone, Copy)]
enum ValueSchema {
    Integer(i32, i32),
    Float(f64, f64),
    Boolean,
    Unit,
    Resource,
    Rarity,
    RgbColor,
    Text,
    TextList,
    Enchantments,
    Compound(&'static [SchemaField]),
}

#[derive(Clone, Copy)]
struct SchemaField {
    name: &'static str,
    schema: ValueSchema,
    required: bool,
}

const FOOD_FIELDS: &[SchemaField] = &[
    SchemaField {
        name: "nutrition",
        schema: ValueSchema::Integer(0, i32::MAX),
        required: true,
    },
    SchemaField {
        name: "saturation",
        schema: ValueSchema::Float(-f32::MAX as f64, f32::MAX as f64),
        required: true,
    },
    SchemaField {
        name: "can_always_eat",
        schema: ValueSchema::Boolean,
        required: false,
    },
];
const COOLDOWN_FIELDS: &[SchemaField] = &[
    SchemaField {
        name: "seconds",
        schema: ValueSchema::Float(f64::MIN_POSITIVE, f32::MAX as f64),
        required: true,
    },
    SchemaField {
        name: "cooldown_group",
        schema: ValueSchema::Resource,
        required: false,
    },
];
const USE_EFFECTS_FIELDS: &[SchemaField] = &[
    SchemaField {
        name: "can_sprint",
        schema: ValueSchema::Boolean,
        required: false,
    },
    SchemaField {
        name: "interact_vibrations",
        schema: ValueSchema::Boolean,
        required: false,
    },
    SchemaField {
        name: "speed_multiplier",
        schema: ValueSchema::Float(0.0, 1.0),
        required: false,
    },
];
const WEAPON_FIELDS: &[SchemaField] = &[
    SchemaField {
        name: "item_damage_per_attack",
        schema: ValueSchema::Integer(0, i32::MAX),
        required: false,
    },
    SchemaField {
        name: "disable_blocking_for_seconds",
        schema: ValueSchema::Float(0.0, f32::MAX as f64),
        required: false,
    },
];
const ATTACK_RANGE_FIELDS: &[SchemaField] = &[
    SchemaField {
        name: "min_reach",
        schema: ValueSchema::Float(0.0, 64.0),
        required: false,
    },
    SchemaField {
        name: "max_reach",
        schema: ValueSchema::Float(0.0, 64.0),
        required: false,
    },
    SchemaField {
        name: "min_creative_reach",
        schema: ValueSchema::Float(0.0, 64.0),
        required: false,
    },
    SchemaField {
        name: "max_creative_reach",
        schema: ValueSchema::Float(0.0, 64.0),
        required: false,
    },
    SchemaField {
        name: "hitbox_margin",
        schema: ValueSchema::Float(0.0, 1.0),
        required: false,
    },
    SchemaField {
        name: "mob_factor",
        schema: ValueSchema::Float(0.0, 2.0),
        required: false,
    },
];
const ENCHANTABLE_FIELDS: &[SchemaField] = &[SchemaField {
    name: "value",
    schema: ValueSchema::Integer(1, i32::MAX),
    required: true,
}];

fn validate_schema(
    label: &str,
    value: &NbtValue,
    schema: ValueSchema,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let valid = match schema {
        ValueSchema::Integer(min, max) => {
            matches!(value.kind, NbtValueKind::Int(number) if (min..=max).contains(&number))
        }
        ValueSchema::Float(min, max) => {
            numeric(value).is_some_and(|number| number.is_finite() && (min..=max).contains(&number))
        }
        ValueSchema::Boolean => matches!(value.kind, NbtValueKind::Byte(0 | 1)),
        ValueSchema::Unit => {
            matches!(&value.kind, NbtValueKind::Compound(entries) if entries.is_empty())
        }
        ValueSchema::Resource => {
            matches!(&value.kind, NbtValueKind::String(id) if super::rules::valid_resource_location(id))
        }
        ValueSchema::Rarity => {
            matches!(&value.kind, NbtValueKind::String(name) if matches!(name.as_str(), "common" | "uncommon" | "rare" | "epic"))
        }
        ValueSchema::RgbColor => {
            matches!(value.kind, NbtValueKind::Int(number) if (0..=0x00ff_ffff).contains(&number))
                || matches!(&value.kind, NbtValueKind::List(channels) if channels.len() == 3 && channels.iter().all(|channel| numeric(channel).is_some_and(|number| number.is_finite())))
        }
        ValueSchema::Text => {
            matches!(
                value.kind,
                NbtValueKind::String(_) | NbtValueKind::Compound(_)
            ) || matches!(&value.kind, NbtValueKind::List(parts) if !parts.is_empty() && parts.iter().all(|part| matches!(part.kind, NbtValueKind::String(_) | NbtValueKind::Compound(_))))
        }
        ValueSchema::TextList => {
            matches!(&value.kind, NbtValueKind::List(lines) if lines.len() <= 256 && lines.iter().all(|line| matches!(line.kind, NbtValueKind::String(_) | NbtValueKind::Compound(_))))
        }
        ValueSchema::Enchantments => {
            if let NbtValueKind::Compound(entries) = &value.kind {
                for entry in entries {
                    validate_id(
                        "enchantment",
                        "附魔",
                        &entry.key,
                        entry.key_span,
                        diagnostics,
                    );
                    validate_schema(
                        &format!("{label}.{}", entry.key),
                        &entry.value,
                        ValueSchema::Integer(1, 255),
                        diagnostics,
                    );
                }
                true
            } else {
                false
            }
        }
        ValueSchema::Compound(fields) => {
            if let NbtValueKind::Compound(entries) = &value.kind {
                if !fields.is_empty() {
                    for field in fields {
                        if field.required && !entries.iter().any(|entry| entry.key == field.name) {
                            diagnostics.push(Diagnostic::new(
                                format!("组件 `{label}` 缺少字段 `{}`", field.name),
                                value.span,
                            ));
                        }
                    }
                    for entry in entries {
                        if let Some(field) = fields.iter().find(|field| field.name == entry.key) {
                            validate_schema(
                                &format!("{label}.{}", entry.key),
                                &entry.value,
                                field.schema,
                                diagnostics,
                            );
                        } else {
                            diagnostics.push(Diagnostic::new(
                                format!("组件 `{label}` 没有字段 `{}`", entry.key),
                                entry.key_span,
                            ));
                        }
                    }
                }
                true
            } else {
                false
            }
        }
    };
    if !valid {
        diagnostics.push(Diagnostic::new(
            format!("组件 `{label}` 的值不符合 26.3 codec 的类型或范围"),
            value.span,
        ));
    }
}

fn numeric(value: &NbtValue) -> Option<f64> {
    match value.kind {
        NbtValueKind::Int(value) => Some(f64::from(value)),
        NbtValueKind::Float(value) => Some(f64::from(value)),
        NbtValueKind::Double(value) => Some(value),
        _ => None,
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
