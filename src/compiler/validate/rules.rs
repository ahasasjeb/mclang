//! 与具体声明无关的语法规则：名称、资源位置、标签、枚举值和保留字。

use crate::ast::{Function, Span};
use crate::diagnostic::Diagnostic;

pub(super) fn valid_name(name: &str) -> bool {
    let mut characters = name.chars();
    matches!(characters.next(), Some('a'..='z' | '_'))
        && characters.all(|character| matches!(character, 'a'..='z' | '0'..='9' | '_'))
}

pub(super) fn valid_resource_path(path: &str) -> bool {
    !path.is_empty()
        && path.split('/').all(|segment| {
            !segment.is_empty()
                && segment != "."
                && segment != ".."
                && !windows_reserved_name(segment)
                && segment
                    .chars()
                    .all(|character| matches!(character, 'a'..='z' | '0'..='9' | '_' | '-' | '.'))
        })
}

pub(super) fn valid_resource_location(value: &str) -> bool {
    let Some((namespace, path)) = value.split_once(':') else {
        return false;
    };
    !namespace.is_empty()
        && namespace
            .chars()
            .all(|character| matches!(character, 'a'..='z' | '0'..='9' | '_' | '-' | '.'))
        && valid_resource_path(path)
}

pub(super) fn valid_nbt_path(path: &str) -> bool {
    !path.is_empty()
        && path.split('.').all(|segment| {
            !segment.is_empty()
                && segment
                    .chars()
                    .all(|character| character.is_ascii_alphanumeric() || character == '_')
        })
}

pub(super) fn valid_entity_tag(tag: &str) -> bool {
    !tag.is_empty()
        && tag.len() <= 1024
        && tag.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '_' | '-' | '.')
        })
}

pub(super) fn valid_text_color(color: &str) -> bool {
    matches!(
        color,
        "black"
            | "dark_blue"
            | "dark_green"
            | "dark_aqua"
            | "dark_red"
            | "dark_purple"
            | "gold"
            | "gray"
            | "dark_gray"
            | "blue"
            | "green"
            | "aqua"
            | "red"
            | "light_purple"
            | "yellow"
            | "white"
    )
}

pub(super) fn valid_sound_source(source: &str) -> bool {
    matches!(
        source,
        "master"
            | "music"
            | "record"
            | "weather"
            | "block"
            | "hostile"
            | "neutral"
            | "player"
            | "ambient"
            | "voice"
            | "ui"
    )
}

pub(super) fn windows_reserved_name(name: &str) -> bool {
    let stem = name.split('.').next().unwrap_or(name).to_ascii_uppercase();
    matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || stem
            .strip_prefix("COM")
            .or_else(|| stem.strip_prefix("LPT"))
            .is_some_and(|number| {
                matches!(number, "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9")
            })
}

pub(super) fn supported_resource_kind(kind: &str) -> bool {
    const SIMPLE_KINDS: &[&str] = &[
        "advancement",
        "banner_pattern",
        "block_transformer",
        "cat_sound_variant",
        "cat_variant",
        "chat_type",
        "chicken_sound_variant",
        "chicken_variant",
        "context_float_provider",
        "context_int_provider",
        "cow_sound_variant",
        "cow_variant",
        "damage_type",
        "decorated_pot_pattern",
        "dialog",
        "dimension_type",
        "enchantment",
        "enchantment_provider",
        "frog_variant",
        "instrument",
        "item_modifier",
        "jukebox_song",
        "loot_table",
        "painting_variant",
        "pig_sound_variant",
        "pig_variant",
        "predicate",
        "recipe",
        "sulfur_cube_archetype",
        "test_environment",
        "test_instance",
        "timeline",
        "trade_set",
        "trial_spawner",
        "trim_material",
        "trim_pattern",
        "villager_trade",
        "wolf_sound_variant",
        "wolf_variant",
        "world_clock",
        "zombie_nautilus_variant",
    ];
    SIMPLE_KINDS.contains(&kind) || kind.starts_with("worldgen/") && valid_resource_path(kind)
}

pub(super) fn validate_identifier(
    kind: &str,
    name: &str,
    span: Span,
    diagnostics: &mut Vec<Diagnostic>,
) {
    const RESERVED: &[&str] = &[
        "namespace",
        "score",
        "resource",
        "query",
        "item",
        "item_stack",
        "storage",
        "entity",
        "items",
        "fn",
        "let",
        "run",
        "call",
        "schedule",
        "after",
        "append",
        "replace",
        "if",
        "else",
        "while",
        "execute",
        "each",
        "in_dimension",
        "spawn",
        "self",
        "message",
        "sound",
        "predicate",
        "player",
        "true",
        "false",
        "return",
    ];
    if !valid_name(name) {
        diagnostics.push(Diagnostic::new(
            format!("{kind}名 `{name}` 只能包含小写 ASCII 字母、数字和下划线，且不能以数字开头"),
            span,
        ));
    } else if name.len() > 32 {
        diagnostics.push(Diagnostic::new(
            format!("{kind}名 `{name}` 不能超过 32 个字节"),
            span,
        ));
    } else if kind == "函数" && windows_reserved_name(name) {
        diagnostics.push(Diagnostic::new(
            format!("函数名 `{name}` 会与 Windows 设备名冲突"),
            span,
        ));
    } else if RESERVED.contains(&name) {
        diagnostics.push(Diagnostic::new(
            format!("`{name}` 是保留字，不能用作{kind}名"),
            span,
        ));
    }
}

/// 从函数属性推导它需要的最低执行上下文。
pub(super) fn function_context(function: &Function) -> super::ExecutionContext {
    use crate::ast::Attribute;

    if function.attributes.contains(&Attribute::Player) {
        super::ExecutionContext::Player
    } else if function.attributes.contains(&Attribute::Entity) {
        super::ExecutionContext::Entity
    } else {
        super::ExecutionContext::None
    }
}
