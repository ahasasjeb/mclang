//! 世界与方块语句的语义检查：坐标范围、方块状态、资源位置与游戏规则表。
//!
//! 命令签名来自 26.3 的 `SetBlockCommand`、`FillCommand`、`CloneCommands`、
//! `FillBiomeCommand`、`PlaceCommand`、`ForceLoadCommand`、`TimeCommand`、
//! `WeatherCommand`、`GameRuleCommand`、`WorldBorderCommand` 与 `LocateCommand`；
//! 游戏规则名与取值类型来自 `GameRules` 的注册表引导代码。

mod blocks;
mod border;
mod gamerules;
mod positions;

use crate::ast::{CloneFilter, LocateKind, Span, Statement, StatementKind, TimeOperation};
use crate::diagnostic::Diagnostic;

use super::registry::{validate_id, validate_id_or_tag};
use super::rules::valid_resource_location;

pub(super) use blocks::validate_block_state;
pub(super) use gamerules::game_rule_exists;
pub(super) use positions::{validate_block_position, validate_position_value, validate_vec2};

use blocks::validate_forceload;
use border::validate_world_border;
use gamerules::validate_game_rule;

/// 水平坐标范围，对应 `Level.isInWorldBoundsHorizontal` 的半开区间。
pub(super) const HORIZONTAL_MIN: i32 = -30_000_000;
pub(super) const HORIZONTAL_MAX: i32 = 29_999_999;
/// 垂直坐标范围，对应 `DimensionType.MIN_Y..=MAX_Y`。
pub(super) const VERTICAL_MIN: i32 = -2032;
pub(super) const VERTICAL_MAX: i32 = 2031;
/// `WorldBorderCommand` 的边长与中心限制。
const BORDER_MAX_SIZE: f64 = 59_999_968.0;
const BORDER_MAX_CENTER: f64 = 29_999_984.0;

/// 世界语句的校验入口；调用方已按 [`StatementKind`] 分派。
pub(super) fn validate_world_statement(statement: &Statement, diagnostics: &mut Vec<Diagnostic>) {
    match &statement.kind {
        StatementKind::SetBlock { pos, block, .. } => {
            validate_block_position(pos, diagnostics);
            validate_block_state(block, false, diagnostics);
        }
        StatementKind::Fill {
            from,
            to,
            block,
            filter,
            ..
        } => {
            validate_block_position(from, diagnostics);
            validate_block_position(to, diagnostics);
            validate_block_state(block, false, diagnostics);
            if let Some(filter) = filter {
                validate_block_state(filter, true, diagnostics);
            }
        }
        StatementKind::FillBiome {
            from,
            to,
            biome,
            filter,
        } => {
            validate_block_position(from, diagnostics);
            validate_block_position(to, diagnostics);
            validate_id("biome", "生物群系", biome, statement.span, diagnostics);
            if let Some(filter) = filter {
                validate_id_or_tag("biome", "生物群系过滤", filter, statement.span, diagnostics);
            }
        }
        StatementKind::Clone {
            begin,
            end,
            destination,
            from_dimension,
            to_dimension,
            filter,
            ..
        } => {
            validate_block_position(begin, diagnostics);
            validate_block_position(end, diagnostics);
            validate_block_position(destination, diagnostics);
            for dimension in [from_dimension, to_dimension].into_iter().flatten() {
                validate_id("dimension", "维度", dimension, statement.span, diagnostics);
            }
            if let CloneFilter::Filtered(filter) = filter {
                validate_block_state(filter, true, diagnostics);
            }
        }
        StatementKind::PlaceFeature { feature, pos } => {
            validate_id_or_tag(
                "worldgen/feature",
                "地物",
                feature,
                statement.span,
                diagnostics,
            );
            if let Some(pos) = pos {
                validate_block_position(pos, diagnostics);
            }
        }
        StatementKind::PlaceJigsaw {
            pool,
            target,
            max_depth,
            pos,
        } => {
            validate_id(
                "worldgen/template_pool",
                "模板池",
                pool,
                statement.span,
                diagnostics,
            );
            // 拼图目标是 `IdentifierArgument`：结构 id 或 `minecraft:empty`，
            // 不受模板池注册表约束，只做资源位置语法检查。
            validate_resource(target, "拼图目标", statement.span, diagnostics);
            if !(1..=20).contains(max_depth) {
                diagnostics.push(Diagnostic::new(
                    "place.jigsaw 最大深度必须是 1 到 20 之间的整数",
                    statement.span,
                ));
            }
            if let Some(pos) = pos {
                validate_block_position(pos, diagnostics);
            }
        }
        StatementKind::PlaceStructure { structure, pos } => {
            validate_id(
                "worldgen/structure",
                "结构",
                structure,
                statement.span,
                diagnostics,
            );
            if let Some(pos) = pos {
                validate_block_position(pos, diagnostics);
            }
        }
        StatementKind::PlaceTemplate {
            template,
            pos,
            integrity,
            ..
        } => {
            validate_id(
                "structure",
                "结构模板",
                template,
                statement.span,
                diagnostics,
            );
            validate_block_position(pos, diagnostics);
            if let Some(integrity) = integrity
                && !integrity
                    .parse::<f64>()
                    .is_ok_and(|value| (0.0..=1.0).contains(&value))
            {
                diagnostics.push(Diagnostic::new(
                    "place.template 完整度必须是 0.0 到 1.0 之间的数字",
                    statement.span,
                ));
            }
        }
        StatementKind::ForceLoad(operation) => {
            validate_forceload(operation, statement.span, diagnostics);
        }
        StatementKind::TimeAction { operation, clock } => {
            if let Some(clock) = clock {
                validate_id(
                    "world_clock",
                    "世界时钟",
                    clock,
                    statement.span,
                    diagnostics,
                );
            }
            if let TimeOperation::Rate(rate) = operation
                && !rate
                    .parse::<f64>()
                    .is_ok_and(|value| (1.0e-5..=1000.0).contains(&value))
            {
                diagnostics.push(Diagnostic::new(
                    "time.rate 的速率必须在 0.00001 到 1000 之间",
                    statement.span,
                ));
            }
        }
        StatementKind::Weather { .. } => {}
        StatementKind::GameRuleSet { name, value } => {
            validate_game_rule(name, *value, statement.span, diagnostics);
        }
        StatementKind::WorldBorder(operation) => {
            validate_world_border(operation, statement.span, diagnostics);
        }
        StatementKind::Locate { kind, target } => {
            let (registry, label) = match kind {
                LocateKind::Structure => ("worldgen/structure", "结构"),
                LocateKind::Biome => ("biome", "生物群系"),
                LocateKind::Poi => ("point_of_interest_type", "兴趣点"),
            };
            validate_id_or_tag(registry, label, target, statement.span, diagnostics);
        }
        _ => unreachable!("validate_world_statement 只处理世界与方块语句"),
    }
}

fn validate_resource(value: &str, label: &str, span: Span, diagnostics: &mut Vec<Diagnostic>) {
    if !valid_resource_location(value) {
        diagnostics.push(Diagnostic::new(
            format!("`{value}` 不是有效的{label}资源位置"),
            span,
        ));
    }
}
