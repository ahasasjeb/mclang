//! 粒子参数按 26.3 的 `ParticleTypes` 注册表和各 `ParticleOption` codec 检查。

use crate::ast::{NbtCategory, NbtValue, NbtValueKind, ParticleCommand, Span};
use crate::diagnostic::Diagnostic;

use super::super::super::registry::validate_id;

#[derive(Clone, Copy)]
enum FieldType {
    Integer,
    PositiveInteger,
    Float,
    Scale,
    RgbColor,
    ArgbColor,
    Vec3,
    Block,
    Item,
    Destination,
}

struct Field {
    name: &'static str,
    kind: FieldType,
    required: bool,
}

const fn required(name: &'static str, kind: FieldType) -> Field {
    Field {
        name,
        kind,
        required: true,
    }
}
const fn optional(name: &'static str, kind: FieldType) -> Field {
    Field {
        name,
        kind,
        required: false,
    }
}

const BLOCK: &[Field] = &[required("block_state", FieldType::Block)];
const COLOR: &[Field] = &[required("color", FieldType::ArgbColor)];
const DUST: &[Field] = &[
    required("color", FieldType::RgbColor),
    required("scale", FieldType::Scale),
];
const DUST_TRANSITION: &[Field] = &[
    required("from_color", FieldType::RgbColor),
    required("to_color", FieldType::RgbColor),
    required("scale", FieldType::Scale),
];
const GEYSER: &[Field] = &[required("water_blocks", FieldType::PositiveInteger)];
const GEYSER_BASE: &[Field] = &[
    required("water_blocks", FieldType::PositiveInteger),
    required("burst_impulse_base", FieldType::Float),
];
const ITEM: &[Field] = &[required("item", FieldType::Item)];
const POWER: &[Field] = &[optional("power", FieldType::Float)];
const SPELL: &[Field] = &[
    optional("color", FieldType::RgbColor),
    optional("power", FieldType::Float),
];
const SCULK: &[Field] = &[required("roll", FieldType::Float)];
const SHRIEK: &[Field] = &[required("delay", FieldType::Integer)];
const TRAIL: &[Field] = &[
    required("target", FieldType::Vec3),
    required("color", FieldType::RgbColor),
    required("duration", FieldType::PositiveInteger),
];
const VIBRATION: &[Field] = &[
    required("destination", FieldType::Destination),
    required("arrival_in_ticks", FieldType::Integer),
];

fn fields(id: &str) -> Option<&'static [Field]> {
    Some(match id {
        "minecraft:block"
        | "minecraft:block_marker"
        | "minecraft:falling_dust"
        | "minecraft:dust_pillar"
        | "minecraft:block_crumble" => BLOCK,
        "minecraft:entity_effect" | "minecraft:flash" | "minecraft:tinted_leaves" => COLOR,
        "minecraft:dust" => DUST,
        "minecraft:dust_color_transition" => DUST_TRANSITION,
        "minecraft:geyser" | "minecraft:geyser_plume" => GEYSER,
        "minecraft:geyser_base" | "minecraft:geyser_poof" => GEYSER_BASE,
        "minecraft:item" => ITEM,
        "minecraft:dragon_breath" => POWER,
        "minecraft:effect" | "minecraft:instant_effect" => SPELL,
        "minecraft:sculk_charge" => SCULK,
        "minecraft:shriek" => SHRIEK,
        "minecraft:trail" => TRAIL,
        "minecraft:vibration" => VIBRATION,
        _ if id.starts_with("minecraft:") => &[],
        _ => return None,
    })
}

pub(super) fn validate_options(
    particle: &ParticleCommand,
    span: Span,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(fields) = fields(&particle.name) else {
        return;
    };
    let entries = match particle.options.as_ref().map(|value| &value.kind) {
        Some(NbtValueKind::Compound(entries)) => entries.as_slice(),
        None => &[],
        _ => unreachable!("粒子选项解析器只生成 compound"),
    };
    for field in fields.iter().filter(|field| field.required) {
        if !entries.iter().any(|entry| entry.key == field.name) {
            diagnostics.push(Diagnostic::new(
                format!("粒子 `{}` 缺少必需选项 `{}`", particle.name, field.name),
                particle.options.as_ref().map_or(span, |value| value.span),
            ));
        }
    }
    for entry in entries {
        match fields.iter().find(|field| field.name == entry.key) {
            Some(field) => validate_field(&particle.name, field.kind, &entry.value, diagnostics),
            None => diagnostics.push(Diagnostic::new(
                format!("粒子 `{}` 没有选项 `{}`", particle.name, entry.key),
                entry.key_span,
            )),
        }
    }
}

fn validate_field(id: &str, kind: FieldType, value: &NbtValue, diagnostics: &mut Vec<Diagnostic>) {
    let valid = match kind {
        FieldType::Integer => integer(value).is_some(),
        FieldType::PositiveInteger => integer(value).is_some_and(|number| number > 0),
        FieldType::Float => number(value)
            .is_some_and(|number| number.is_finite() && number.abs() <= f32::MAX as f64),
        FieldType::Scale => number(value).is_some_and(|number| (0.01..=4.0).contains(&number)),
        FieldType::RgbColor => {
            integer(value).is_some_and(|number| (0..=0x00ff_ffff).contains(&number))
                || numeric_vector(value, 3)
        }
        FieldType::ArgbColor => integer(value).is_some() || numeric_vector(value, 4),
        FieldType::Vec3 => numeric_vector(value, 3),
        FieldType::Block | FieldType::Item => match &value.kind {
            NbtValueKind::String(resource) => {
                validate_id(
                    if matches!(kind, FieldType::Block) {
                        "block"
                    } else {
                        "item"
                    },
                    "粒子选项",
                    resource,
                    value.span,
                    diagnostics,
                );
                true
            }
            NbtValueKind::Compound(entries) => {
                if matches!(kind, FieldType::Block) {
                    validate_block_state(entries, diagnostics)
                } else {
                    validate_item_stack(entries, value.span, diagnostics)
                }
            }
            _ => false,
        },
        FieldType::Destination => validate_destination(value, diagnostics),
    };
    if !valid {
        diagnostics.push(Diagnostic::new(
            format!("粒子 `{id}` 的选项类型或数值不符合 26.3 codec"),
            value.span,
        ));
    }
}

fn validate_block_state(
    entries: &[crate::ast::NbtEntry],
    diagnostics: &mut Vec<Diagnostic>,
) -> bool {
    let Some(id) = entries.iter().find(|entry| entry.key == "id") else {
        return false;
    };
    let NbtValueKind::String(id_value) = &id.value.kind else {
        return false;
    };
    let mut state = crate::ast::BlockStateValue {
        id: id_value.clone(),
        properties: Vec::new(),
        span: id.value.span,
    };
    for entry in entries {
        match entry.key.as_str() {
            "id" => {}
            "properties" => {
                if let NbtValueKind::Compound(properties) = &entry.value.kind {
                    for property in properties {
                        if let NbtValueKind::String(value) = &property.value.kind {
                            state.properties.push(crate::ast::BlockProperty {
                                name: property.key.clone(),
                                value: value.clone(),
                                span: property.key_span.merge(property.value.span),
                            });
                        } else {
                            diagnostics.push(Diagnostic::new(
                                format!("粒子方块属性 `{}` 需要字符串值", property.key),
                                property.value.span,
                            ));
                        }
                    }
                } else {
                    diagnostics.push(Diagnostic::new(
                        "粒子方块 properties 需要字符串值的复合字段",
                        entry.value.span,
                    ));
                }
            }
            _ => diagnostics.push(Diagnostic::new(
                format!("粒子方块没有字段 `{}`", entry.key),
                entry.key_span,
            )),
        }
    }
    super::super::super::world::validate_block_state(&state, false, diagnostics);
    true
}

fn validate_item_stack(
    entries: &[crate::ast::NbtEntry],
    _span: Span,
    diagnostics: &mut Vec<Diagnostic>,
) -> bool {
    let Some(id) = entries.iter().find(|entry| entry.key == "id") else {
        return false;
    };
    let NbtValueKind::String(id_value) = &id.value.kind else {
        return false;
    };
    validate_id("item", "粒子物品", id_value, id.value.span, diagnostics);
    for entry in entries {
        match entry.key.as_str() {
            "id" => {}
            "count" => {
                if !matches!(entry.value.kind, NbtValueKind::Int(count) if (1..=99).contains(&count))
                {
                    diagnostics.push(Diagnostic::new(
                        "粒子物品 count 必须是 1 到 99 的整数",
                        entry.value.span,
                    ));
                }
            }
            "components" => {
                if let NbtValueKind::Compound(components) = &entry.value.kind {
                    for component in components {
                        validate_id(
                            "data_component_type",
                            "粒子物品组件",
                            &component.key,
                            component.key_span,
                            diagnostics,
                        );
                        super::super::super::item_components::validate_component_value(
                            &component.key,
                            &component.value,
                            diagnostics,
                        );
                    }
                } else {
                    diagnostics.push(Diagnostic::new(
                        "粒子物品 components 需要复合字段",
                        entry.value.span,
                    ));
                }
            }
            _ => diagnostics.push(Diagnostic::new(
                format!("粒子物品没有字段 `{}`", entry.key),
                entry.key_span,
            )),
        }
    }
    true
}

fn integer(value: &NbtValue) -> Option<i64> {
    match value.kind {
        NbtValueKind::Byte(v) => Some(i64::from(v)),
        NbtValueKind::Short(v) => Some(i64::from(v)),
        NbtValueKind::Int(v) => Some(i64::from(v)),
        NbtValueKind::Long(v) if i32::try_from(v).is_ok() => Some(v),
        _ => None,
    }
}

fn number(value: &NbtValue) -> Option<f64> {
    match value.kind {
        NbtValueKind::Byte(v) => Some(f64::from(v)),
        NbtValueKind::Short(v) => Some(f64::from(v)),
        NbtValueKind::Int(v) => Some(f64::from(v)),
        NbtValueKind::Long(v) => Some(v as f64),
        NbtValueKind::Float(v) => Some(f64::from(v)),
        NbtValueKind::Double(v) => Some(v),
        _ => None,
    }
}

fn numeric_vector(value: &NbtValue, len: usize) -> bool {
    matches!(&value.kind, NbtValueKind::List(values) if values.len() == len && values.iter().all(|value| value.category() == NbtCategory::Numeric))
}

fn validate_destination(value: &NbtValue, diagnostics: &mut Vec<Diagnostic>) -> bool {
    let NbtValueKind::Compound(entries) = &value.kind else {
        return false;
    };
    let kind = entries.iter().find(|entry| entry.key == "type");
    let pos = entries.iter().find(|entry| entry.key == "pos");
    if !matches!(kind.map(|entry| &entry.value.kind), Some(NbtValueKind::String(value)) if value == "block")
    {
        diagnostics.push(Diagnostic::new(
            "振动粒子 destination.type 必须是 block；26.3 不接受 entity 位置来源",
            value.span,
        ));
        return false;
    }
    if !pos.is_some_and(|entry| numeric_vector(&entry.value, 3)) {
        diagnostics.push(Diagnostic::new(
            "振动粒子 destination.pos 需要三个数值的列表",
            value.span,
        ));
        return false;
    }
    for entry in entries {
        if !matches!(entry.key.as_str(), "type" | "pos") {
            diagnostics.push(Diagnostic::new(
                format!("振动粒子 destination 没有字段 `{}`", entry.key),
                entry.key_span,
            ));
        }
    }
    true
}
