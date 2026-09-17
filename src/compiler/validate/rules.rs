//! 与具体声明无关的语法规则：名称、资源位置、标签、枚举值和保留字。

use crate::ast::{Function, Span};
use crate::diagnostic::Diagnostic;
use crate::parser::keywords::reserved_word;

pub(super) fn valid_name(name: &str) -> bool {
    let mut characters = name.chars();
    matches!(characters.next(), Some('a'..='z' | '_'))
        && characters.all(|character| matches!(character, 'a'..='z' | '0'..='9' | '_'))
}

/// 用户标识符：ASCII 部分沿用 `valid_name` 的小写约定，同时允许中文等
/// 非 ASCII 字母；这些名字会在代码生成前由 `compiler::rename` 换成随机 ASCII 名。
pub(super) fn valid_user_name(name: &str) -> bool {
    let Some(first) = name.chars().next() else {
        return false;
    };
    let first_ok =
        first == '_' || first.is_ascii_lowercase() || (!first.is_ascii() && first.is_alphabetic());
    first_ok
        && name.chars().skip(1).all(|character| {
            character == '_'
                || character.is_ascii_lowercase()
                || character.is_ascii_digit()
                || (!character.is_ascii() && character.is_alphanumeric())
        })
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

pub(super) fn validate_identifier(
    kind: &str,
    name: &str,
    span: Span,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !valid_user_name(name) {
        diagnostics.push(Diagnostic::new(
            format!(
                "{kind}名 `{name}` 只能包含字母、数字和下划线，不能以数字开头；ASCII 字母必须小写"
            ),
            span,
        ));
    } else if name.chars().count() > 32 {
        diagnostics.push(Diagnostic::new(
            format!("{kind}名 `{name}` 不能超过 32 个字符"),
            span,
        ));
    } else if kind == "函数" && windows_reserved_name(name) {
        diagnostics.push(Diagnostic::new(
            format!("函数名 `{name}` 会与 Windows 设备名冲突"),
            span,
        ));
    } else if reserved_word(name) {
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
    } else if function.attributes.contains(&Attribute::NonPlayer) {
        super::ExecutionContext::Mob
    } else if function.attributes.contains(&Attribute::Entity) {
        super::ExecutionContext::Entity
    } else {
        super::ExecutionContext::None
    }
}
