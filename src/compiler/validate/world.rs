//! 世界与方块语句的语义检查：坐标范围、方块状态、资源位置与游戏规则表。
//!
//! 命令签名来自 26.3 的 `SetBlockCommand`、`FillCommand`、`CloneCommands`、
//! `FillBiomeCommand`、`PlaceCommand`、`ForceLoadCommand`、`TimeCommand`、
//! `WeatherCommand`、`GameRuleCommand`、`WorldBorderCommand` 与 `LocateCommand`；
//! 游戏规则名与取值类型来自 `GameRules` 的注册表引导代码。

use crate::ast::{
    BlockPosition, BlockStateValue, CloneFilter, ColumnPosition, Coordinate, ForceLoadOperation,
    GameRuleValue, LocateKind, PositionValue, Span, Statement, StatementKind, TimeOperation,
    Vec2Value, Vec3Value, WorldBorderOperation,
};
use crate::diagnostic::Diagnostic;

use super::registry::{validate_id, validate_id_or_tag};
use super::rules::valid_resource_location;

/// 水平坐标范围，对应 `Level.isInWorldBoundsHorizontal` 的半开区间。
const HORIZONTAL_MIN: i32 = -30_000_000;
const HORIZONTAL_MAX: i32 = 29_999_999;
/// 垂直坐标范围，对应 `DimensionType.MIN_Y..=MAX_Y`。
const VERTICAL_MIN: i32 = -2032;
const VERTICAL_MAX: i32 = 2031;
/// `WorldBorderCommand` 的边长与中心限制。
const BORDER_MAX_SIZE: f64 = 59_999_968.0;
const BORDER_MAX_CENTER: f64 = 29_999_984.0;

/// 游戏规则的取值类型与整数范围。
#[derive(Clone, Copy)]
enum GameRuleKind {
    Bool,
    Integer(i32, i32),
}

/// 26.3 `GameRules` 注册的全部规则。`max_minecart_speed` 需要
/// `minecart_improvements` 特性，默认特性集下命令不可用。
const GAME_RULES: &[(&str, GameRuleKind)] = &[
    ("advance_time", GameRuleKind::Bool),
    ("advance_weather", GameRuleKind::Bool),
    ("allow_entering_nether_using_portals", GameRuleKind::Bool),
    ("block_drops", GameRuleKind::Bool),
    ("block_explosion_drop_decay", GameRuleKind::Bool),
    ("command_blocks_work", GameRuleKind::Bool),
    ("command_block_output", GameRuleKind::Bool),
    ("drowning_damage", GameRuleKind::Bool),
    ("elytra_movement_check", GameRuleKind::Bool),
    ("ender_pearls_vanish_on_death", GameRuleKind::Bool),
    ("entity_drops", GameRuleKind::Bool),
    ("fall_damage", GameRuleKind::Bool),
    ("fire_damage", GameRuleKind::Bool),
    (
        "fire_spread_radius_around_player",
        GameRuleKind::Integer(-1, i32::MAX),
    ),
    ("forgive_dead_players", GameRuleKind::Bool),
    ("freeze_damage", GameRuleKind::Bool),
    ("global_sound_events", GameRuleKind::Bool),
    ("immediate_respawn", GameRuleKind::Bool),
    ("keep_inventory", GameRuleKind::Bool),
    ("lava_source_conversion", GameRuleKind::Bool),
    ("limited_crafting", GameRuleKind::Bool),
    ("locator_bar", GameRuleKind::Bool),
    ("log_admin_commands", GameRuleKind::Bool),
    (
        "max_block_modifications",
        GameRuleKind::Integer(1, i32::MAX),
    ),
    ("max_command_forks", GameRuleKind::Integer(0, i32::MAX)),
    (
        "max_command_sequence_length",
        GameRuleKind::Integer(0, i32::MAX),
    ),
    ("max_entity_cramming", GameRuleKind::Integer(0, i32::MAX)),
    ("max_minecart_speed", GameRuleKind::Integer(1, 1000)),
    ("max_snow_accumulation_height", GameRuleKind::Integer(0, 8)),
    ("mob_drops", GameRuleKind::Bool),
    ("mob_explosion_drop_decay", GameRuleKind::Bool),
    ("mob_griefing", GameRuleKind::Bool),
    ("natural_health_regeneration", GameRuleKind::Bool),
    ("player_movement_check", GameRuleKind::Bool),
    (
        "players_nether_portal_creative_delay",
        GameRuleKind::Integer(0, i32::MAX),
    ),
    (
        "players_nether_portal_default_delay",
        GameRuleKind::Integer(0, i32::MAX),
    ),
    (
        "players_sleeping_percentage",
        GameRuleKind::Integer(0, i32::MAX),
    ),
    ("projectiles_can_break_blocks", GameRuleKind::Bool),
    ("pvp", GameRuleKind::Bool),
    ("raids", GameRuleKind::Bool),
    ("random_tick_speed", GameRuleKind::Integer(0, i32::MAX)),
    ("reduced_debug_info", GameRuleKind::Bool),
    ("respawn_radius", GameRuleKind::Integer(0, i32::MAX)),
    ("send_command_feedback", GameRuleKind::Bool),
    ("show_advancement_messages", GameRuleKind::Bool),
    ("show_death_messages", GameRuleKind::Bool),
    ("spawner_blocks_work", GameRuleKind::Bool),
    ("spawn_mobs", GameRuleKind::Bool),
    ("spawn_monsters", GameRuleKind::Bool),
    ("spawn_patrols", GameRuleKind::Bool),
    ("spawn_phantoms", GameRuleKind::Bool),
    ("spawn_wandering_traders", GameRuleKind::Bool),
    ("spawn_wardens", GameRuleKind::Bool),
    ("spectators_generate_chunks", GameRuleKind::Bool),
    ("spread_vines", GameRuleKind::Bool),
    ("tnt_explodes", GameRuleKind::Bool),
    ("tnt_explosion_drop_decay", GameRuleKind::Bool),
    ("universal_anger", GameRuleKind::Bool),
    ("water_source_conversion", GameRuleKind::Bool),
];

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

/// 校验方块坐标的绝对分量；相对坐标与局部坐标留给运行时。
pub(super) fn validate_block_position(position: &BlockPosition, diagnostics: &mut Vec<Diagnostic>) {
    for (axis, coordinate) in [("X", &position.x), ("Y", &position.y), ("Z", &position.z)] {
        let Some(value) = coordinate.absolute_integer() else {
            continue;
        };
        let valid = match axis {
            "Y" => (VERTICAL_MIN..=VERTICAL_MAX).contains(&value),
            _ => (HORIZONTAL_MIN..=HORIZONTAL_MAX).contains(&value),
        };
        if !valid {
            let (low, high) = if axis == "Y" {
                (VERTICAL_MIN, VERTICAL_MAX)
            } else {
                (HORIZONTAL_MIN, HORIZONTAL_MAX)
            };
            diagnostics.push(Diagnostic::new(
                format!("方块 {axis} 坐标 {value} 超出世界范围（{low} 到 {high}）"),
                position.span,
            ));
        }
    }
}

/// 校验精确坐标（`vec3`）的绝对分量；小数允许，范围与方块坐标一致。
pub(super) fn validate_vec3(position: &Vec3Value, diagnostics: &mut Vec<Diagnostic>) {
    validate_fractional_axis(
        "X",
        &position.x,
        HORIZONTAL_MIN as f64,
        HORIZONTAL_MAX as f64,
        position.span,
        diagnostics,
    );
    validate_fractional_axis(
        "Y",
        &position.y,
        VERTICAL_MIN as f64,
        VERTICAL_MAX as f64,
        position.span,
        diagnostics,
    );
    validate_fractional_axis(
        "Z",
        &position.z,
        HORIZONTAL_MIN as f64,
        HORIZONTAL_MAX as f64,
        position.span,
        diagnostics,
    );
}

/// 校验水平精确坐标（`vec2`）的绝对分量。
pub(super) fn validate_vec2(position: &Vec2Value, diagnostics: &mut Vec<Diagnostic>) {
    validate_fractional_axis(
        "X",
        &position.x,
        HORIZONTAL_MIN as f64,
        HORIZONTAL_MAX as f64,
        position.span,
        diagnostics,
    );
    validate_fractional_axis(
        "Z",
        &position.z,
        HORIZONTAL_MIN as f64,
        HORIZONTAL_MAX as f64,
        position.span,
        diagnostics,
    );
}

/// 任意位置值：方块坐标或精确坐标。
pub(super) fn validate_position_value(position: &PositionValue, diagnostics: &mut Vec<Diagnostic>) {
    match position {
        PositionValue::Block(position) => validate_block_position(position, diagnostics),
        PositionValue::Exact(position) => validate_vec3(position, diagnostics),
    }
}

/// 精确坐标的绝对分量范围；原版在运行期规范化朝向，编译器不限制取值。
fn validate_fractional_axis(
    axis: &str,
    coordinate: &Coordinate,
    min: f64,
    max: f64,
    span: Span,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Coordinate::Absolute(text) = coordinate else {
        return;
    };
    let Ok(value) = text.parse::<f64>() else {
        return;
    };
    if !(min..=max).contains(&value) {
        diagnostics.push(Diagnostic::new(
            format!("{axis} 坐标 {text} 超出世界范围（{min} 到 {max}）"),
            span,
        ));
    }
}

fn validate_column_position(position: &ColumnPosition, diagnostics: &mut Vec<Diagnostic>) {
    for (axis, coordinate) in [("X", &position.x), ("Z", &position.z)] {
        let Some(value) = coordinate.absolute_integer() else {
            continue;
        };
        if !(HORIZONTAL_MIN..=HORIZONTAL_MAX).contains(&value) {
            diagnostics.push(Diagnostic::new(
                format!(
                    "列 {axis} 坐标 {value} 超出世界范围（{HORIZONTAL_MIN} 到 {HORIZONTAL_MAX}）"
                ),
                position.span,
            ));
        }
    }
}

/// 方块状态或方块谓词：`#` 标签只在过滤器里合法，属性名与值限制为 SNBT 安全字符。
pub(super) fn validate_block_state(
    block: &BlockStateValue,
    allow_tag: bool,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let Some(tag) = block.id.strip_prefix('#') {
        if !allow_tag {
            diagnostics.push(Diagnostic::new(
                format!("`{}` 是方块标签，这里需要具体的方块资源位置", block.id),
                block.span,
            ));
        }
        if !valid_resource_location(tag) {
            diagnostics.push(Diagnostic::new(
                format!("`{}` 不是有效的方块标签资源位置", block.id),
                block.span,
            ));
        }
        if !block.properties.is_empty() {
            diagnostics.push(Diagnostic::new("方块标签不能声明方块属性", block.span));
        }
    } else {
        validate_id("block", "方块", &block.id, block.span, diagnostics);
    }
    for property in &block.properties {
        if !valid_block_property_name(&property.name) {
            diagnostics.push(Diagnostic::new(
                format!(
                    "方块属性名 `{}` 只能包含小写字母、数字和下划线",
                    property.name
                ),
                property.span,
            ));
        }
        if !valid_block_property_value(&property.value) {
            diagnostics.push(Diagnostic::new(
                format!("方块属性值 `{}` 含有不允许的字符", property.value),
                property.span,
            ));
        }
    }
}

fn valid_block_property_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|character| matches!(character, 'a'..='z' | '0'..='9' | '_'))
}

fn valid_block_property_value(value: &str) -> bool {
    !value.is_empty()
        && value
            .chars()
            .all(|character| matches!(character, 'a'..='z' | '0'..='9' | '_' | '.' | '+' | '-'))
}

fn validate_forceload(
    operation: &ForceLoadOperation,
    span: Span,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match operation {
        ForceLoadOperation::Add { from, to } | ForceLoadOperation::Remove { from, to } => {
            validate_column_position(from, diagnostics);
            if let Some(to) = to {
                validate_column_position(to, diagnostics);
            }
            if let Some(count) = forceload_chunk_count(from, to.as_ref())
                && count > 256
            {
                diagnostics.push(Diagnostic::new(
                    format!("forceload 一次最多影响 256 个区块，当前范围包含 {count} 个"),
                    span,
                ));
            }
        }
        ForceLoadOperation::RemoveAll => {}
        ForceLoadOperation::Query { pos } => {
            if let Some(pos) = pos {
                validate_column_position(pos, diagnostics);
            }
        }
    }
}

/// `add`/`remove` 的区块数量：只有两个端点都是绝对坐标时才能静态计算。
fn forceload_chunk_count(from: &ColumnPosition, to: Option<&ColumnPosition>) -> Option<u64> {
    let x0 = from.x.absolute_integer()?;
    let z0 = from.z.absolute_integer()?;
    let (x1, z1) = match to {
        Some(to) => (to.x.absolute_integer()?, to.z.absolute_integer()?),
        None => (x0, z0),
    };
    let (min_x, max_x) = (x0.min(x1), x0.max(x1));
    let (min_z, max_z) = (z0.min(z1), z0.max(z1));
    let chunks_x = (max_x.div_euclid(16) - min_x.div_euclid(16) + 1) as u64;
    let chunks_z = (max_z.div_euclid(16) - min_z.div_euclid(16) + 1) as u64;
    Some(chunks_x * chunks_z)
}

/// `gamerule.query` 的表达式只需要存在性检查。
pub(super) fn game_rule_exists(name: &str) -> bool {
    game_rule(name).is_some()
}

fn game_rule(name: &str) -> Option<(&'static str, GameRuleKind)> {
    let short = name.strip_prefix("minecraft:").unwrap_or(name);
    GAME_RULES
        .iter()
        .find(|(id, _)| *id == short)
        .map(|(id, kind)| (*id, *kind))
}

fn validate_game_rule(
    name: &str,
    value: GameRuleValue,
    span: Span,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some((id, kind)) = game_rule(name) else {
        diagnostics.push(Diagnostic::new(
            format!("未知游戏规则 `{name}`；规则名来自 26.3 的 GameRules 注册表"),
            span,
        ));
        return;
    };
    match (kind, value) {
        (GameRuleKind::Bool, GameRuleValue::Bool(_)) => {}
        (GameRuleKind::Bool, GameRuleValue::Integer(_)) => diagnostics.push(Diagnostic::new(
            format!("游戏规则 `{id}` 需要 true 或 false"),
            span,
        )),
        (GameRuleKind::Integer(min, max), GameRuleValue::Integer(value)) => {
            if !(min..=max).contains(&value) {
                let upper = if max == i32::MAX {
                    "2147483647".to_owned()
                } else {
                    max.to_string()
                };
                diagnostics.push(Diagnostic::new(
                    format!("游戏规则 `{id}` 的值必须在 {min} 到 {upper} 之间"),
                    span,
                ));
            }
        }
        (GameRuleKind::Integer(..), GameRuleValue::Bool(_)) => {
            diagnostics.push(Diagnostic::new(format!("游戏规则 `{id}` 需要整数"), span))
        }
    }
}

fn validate_world_border(
    operation: &WorldBorderOperation,
    span: Span,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match operation {
        WorldBorderOperation::Add { distance, .. } => {
            if !distance
                .parse::<f64>()
                .is_ok_and(|value| value.abs() <= BORDER_MAX_SIZE)
            {
                diagnostics.push(Diagnostic::new(
                    format!("worldborder.add 的距离绝对值不能超过 {BORDER_MAX_SIZE}"),
                    span,
                ));
            }
        }
        WorldBorderOperation::Set { distance, .. } => {
            if !distance
                .parse::<f64>()
                .is_ok_and(|value| (1.0..=BORDER_MAX_SIZE).contains(&value))
            {
                diagnostics.push(Diagnostic::new(
                    format!("worldborder.set 的边长必须在 1 到 {BORDER_MAX_SIZE} 之间"),
                    span,
                ));
            }
        }
        WorldBorderOperation::Center(value) => {
            validate_vec2(value, diagnostics);
            for component in [&value.x, &value.z] {
                let Coordinate::Absolute(text) = component else {
                    continue;
                };
                if !text
                    .parse::<f64>()
                    .is_ok_and(|value| value.abs() <= BORDER_MAX_CENTER)
                {
                    diagnostics.push(Diagnostic::new(
                        format!("worldborder.center 的坐标绝对值不能超过 {BORDER_MAX_CENTER}"),
                        span,
                    ));
                }
            }
        }
        WorldBorderOperation::DamageAmount(value) | WorldBorderOperation::DamageBuffer(value) => {
            if !value.parse::<f64>().is_ok_and(|value| value >= 0.0) {
                diagnostics.push(Diagnostic::new("worldborder 伤害参数必须是非负数字", span));
            }
        }
        WorldBorderOperation::WarningDistance(value) => {
            if *value > i32::MAX as u32 {
                diagnostics.push(Diagnostic::new(
                    "worldborder 警告距离不能超过 2147483647",
                    span,
                ));
            }
        }
        WorldBorderOperation::WarningTime(_) => {}
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
