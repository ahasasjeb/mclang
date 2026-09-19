use super::{
    components::validate_component,
    registry::{validate_enum, validate_id},
    rules::{valid_entity_tag, valid_resource_location},
    statements::{ValidationContext, validate_holder},
    world::{validate_block_position, validate_position_value},
};
use crate::ast::*;
use crate::compiler::types::ExecutionContext;
use crate::diagnostic::Diagnostic;

mod teams;

pub(super) fn validate_entity_command(
    command: &EntityCommand,
    span: Span,
    ctx: ValidationContext<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match command {
        EntityCommand::Kill(target) | EntityCommand::Swing { target, .. } => {
            target_or_sender(target.as_ref(), false, false, span, ctx, diagnostics)
        }
        EntityCommand::Tag { target, operation } => {
            entity_target(target, false, false, span, ctx, diagnostics);
            if let TagOperation::Add(tag) | TagOperation::Remove(tag) = operation
                && !valid_entity_tag(tag)
            {
                diagnostics.push(Diagnostic::new(format!("`{tag}` 不是有效的实体标签"), span));
            }
        }
        EntityCommand::Enchant {
            target,
            enchantment,
            level,
        } => {
            entity_target(target, false, false, span, ctx, diagnostics);
            validate_id("enchantment", "附魔", enchantment, span, diagnostics);
            if level.is_some_and(|v| v > i32::MAX as u32) {
                diagnostics.push(Diagnostic::new("附魔等级不能超过 2147483647", span));
            }
        }
        EntityCommand::Damage {
            target,
            amount,
            damage_type,
            source,
        } => {
            entity_target(target, true, false, span, ctx, diagnostics);
            number_range(amount, 0.0, f32::MAX as f64, "伤害值", span, diagnostics);
            if let Some(id) = damage_type {
                validate_id("damage_type", "伤害类型", id, span, diagnostics);
            }
            match source {
                Some(DamageOrigin::At(pos)) => validate_position_value(pos, diagnostics),
                Some(DamageOrigin::By { entity, cause }) => {
                    entity_target(entity, true, false, span, ctx, diagnostics);
                    if let Some(cause) = cause {
                        entity_target(cause, true, false, span, ctx, diagnostics);
                    }
                }
                None => {}
            }
        }
        EntityCommand::Attribute {
            target,
            attribute,
            operation,
        } => {
            entity_target(target, true, false, span, ctx, diagnostics);
            validate_id("attribute", "属性", attribute, span, diagnostics);
            match operation {
                AttributeOperation::Get(scale) | AttributeOperation::BaseGet(scale) => {
                    if let Some(scale) = scale {
                        finite_number(scale, span, diagnostics);
                    }
                }
                AttributeOperation::BaseSet(value) => finite_number(value, span, diagnostics),
                AttributeOperation::ModifierAdd { id, value, .. } => {
                    resource_id(id, span, diagnostics);
                    finite_number(value, span, diagnostics);
                }
                AttributeOperation::ModifierRemove(id) => resource_id(id, span, diagnostics),
                AttributeOperation::ModifierGet { id, scale } => {
                    resource_id(id, span, diagnostics);
                    if let Some(scale) = scale {
                        finite_number(scale, span, diagnostics);
                    }
                }
                AttributeOperation::BaseReset => {}
            }
        }
        EntityCommand::Ride { target, vehicle } => {
            entity_target(target, true, false, span, ctx, diagnostics);
            if let Some(vehicle) = vehicle {
                entity_target(vehicle, true, false, span, ctx, diagnostics);
            }
        }
        EntityCommand::Rotate { target, facing } => {
            entity_target(target, true, false, span, ctx, diagnostics);
            validate_facing(facing, span, ctx, diagnostics);
        }
        EntityCommand::Spread {
            center,
            spread,
            range,
            target,
            ..
        } => {
            super::world::validate_vec2(center, diagnostics);
            number_range(spread, 0.0, f32::MAX as f64, "散布间距", span, diagnostics);
            number_range(range, 1.0, f32::MAX as f64, "散布范围", span, diagnostics);
            entity_target(target, false, false, span, ctx, diagnostics);
        }
        EntityCommand::Spectate { target, player } => {
            if let Some(target) = target {
                entity_target(target, true, false, span, ctx, diagnostics);
            }
            target_or_sender(player.as_ref(), true, true, span, ctx, diagnostics);
        }
        EntityCommand::Trigger { objective, .. } => {
            require_sender(true, span, ctx, diagnostics);
            match ctx.symbols.objective_declarations.get(objective.as_str()) {
                Some(decl) if decl.criteria.as_deref() == Some("trigger") => {}
                _ => diagnostics.push(Diagnostic::new(
                    format!("trigger 需要已声明且 criteria = \"trigger\" 的目标 `{objective}`"),
                    span,
                )),
            }
        }
        EntityCommand::GameMode { mode, target } => {
            validate_enum("gamemode", "游戏模式", mode, span, diagnostics);
            target_or_sender(target.as_ref(), false, true, span, ctx, diagnostics);
        }
        EntityCommand::DefaultGameMode(mode) => {
            validate_enum("gamemode", "游戏模式", mode, span, diagnostics)
        }
        EntityCommand::Difficulty(Some(mode)) => {
            validate_enum("difficulty", "难度", mode, span, diagnostics)
        }
        EntityCommand::SpawnPoint {
            target, position, ..
        } => {
            target_or_sender(target.as_ref(), false, true, span, ctx, diagnostics);
            if let Some(position) = position {
                validate_block_position(position, diagnostics);
            }
        }
        EntityCommand::WorldSpawn { position, .. } => {
            if let Some(position) = position {
                validate_block_position(position, diagnostics);
            }
        }
        EntityCommand::Team(operation) => teams::validate_team(operation, span, ctx, diagnostics),
        EntityCommand::Waypoint(operation) => {
            teams::validate_waypoint(operation, span, ctx, diagnostics)
        }
        EntityCommand::Difficulty(None) | EntityCommand::List { .. } => {}
    }
}

pub(in crate::compiler::validate) fn entity_target(
    target: &Holder,
    single: bool,
    player: bool,
    span: Span,
    ctx: ValidationContext<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    validate_holder(target, span, ctx, diagnostics);
    match target {
        Holder::Query(name, query_span) => {
            if let Some(query) = ctx.symbols.queries.get(name.as_str()) {
                if single && query.limit != Some(1) {
                    diagnostics.push(Diagnostic::new(
                        format!("目标 `{name}` 必须使用 limit(1)，此参数只接受单实体"),
                        *query_span,
                    ));
                }
                if player && query.entity_type != "minecraft:player" {
                    diagnostics.push(Diagnostic::new(
                        format!("目标 `{name}` 必须匹配 minecraft:player"),
                        *query_span,
                    ));
                }
                if query.item.is_some() {
                    diagnostics.push(Diagnostic::new(format!("目标 `{name}` 含物品条件，不能直接用作选择器；请在 each({name}) 中使用 self"), *query_span));
                }
            }
        }
        Holder::SelfEntity if player && ctx.context != ExecutionContext::Player => diagnostics
            .push(Diagnostic::new(
                "self 需要玩家执行上下文（@player 或玩家查询的 each）",
                span,
            )),
        Holder::Origin => diagnostics.push(Diagnostic::new(
            "此命令的实体参数不接受 origin；请使用 execute on origin 后的 self",
            span,
        )),
        _ => {}
    }
}

pub(in crate::compiler::validate) fn validate_facing(
    facing: &Facing,
    span: Span,
    ctx: ValidationContext<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match facing {
        Facing::Position(pos) => validate_position_value(pos, diagnostics),
        Facing::Entity { target, .. } => entity_target(target, true, false, span, ctx, diagnostics),
        Facing::Rotation(rotation) => {
            for coordinate in [&rotation.yaw, &rotation.pitch] {
                if let Coordinate::Absolute(value) | Coordinate::Relative(value) = coordinate {
                    let value = value.trim_start_matches('~');
                    if !value.is_empty() {
                        number_range(
                            value,
                            -(f32::MAX as f64),
                            f32::MAX as f64,
                            "朝向",
                            rotation.span,
                            diagnostics,
                        );
                    }
                }
            }
        }
    }
}

fn target_or_sender(
    target: Option<&Holder>,
    single: bool,
    player: bool,
    span: Span,
    ctx: ValidationContext<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let Some(target) = target {
        entity_target(target, single, player, span, ctx, diagnostics);
    } else {
        require_sender(player, span, ctx, diagnostics);
    }
}

fn require_sender(
    player: bool,
    span: Span,
    ctx: ValidationContext<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !ctx.context.satisfies(if player {
        ExecutionContext::Player
    } else {
        ExecutionContext::Entity
    }) {
        diagnostics.push(Diagnostic::new(
            if player {
                "省略目标时需要玩家执行上下文"
            } else {
                "省略目标时需要实体执行上下文"
            },
            span,
        ));
    }
}

pub(in crate::compiler::validate) fn number_range(
    value: &str,
    min: f64,
    max: f64,
    label: &str,
    span: Span,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !value
        .parse::<f64>()
        .is_ok_and(|v| v.is_finite() && v >= min && v <= max)
    {
        diagnostics.push(Diagnostic::new(
            format!("{label} `{value}` 必须是 {min} 到 {max} 之间的有限数值"),
            span,
        ));
    }
}

fn finite_number(value: &str, span: Span, diagnostics: &mut Vec<Diagnostic>) {
    number_range(value, -f64::MAX, f64::MAX, "数值", span, diagnostics);
}

pub(in crate::compiler::validate) fn resource_id(
    id: &str,
    span: Span,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !valid_resource_location(id) {
        diagnostics.push(Diagnostic::new(format!("`{id}` 不是有效的资源位置"), span));
    }
}
