use crate::ast::{GameRuleValue, Span};
use crate::diagnostic::Diagnostic;

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

/// `gamerule.query` 的表达式只需要存在性检查。
pub(crate) fn game_rule_exists(name: &str) -> bool {
    game_rule(name).is_some()
}

fn game_rule(name: &str) -> Option<(&'static str, GameRuleKind)> {
    let short = name.strip_prefix("minecraft:").unwrap_or(name);
    GAME_RULES
        .iter()
        .find(|(id, _)| *id == short)
        .map(|(id, kind)| (*id, *kind))
}

pub(super) fn validate_game_rule(
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
