//! 与具体声明无关的语法规则：名称、资源位置、标签、枚举值和保留字。

use crate::ast::{Function, Span};
use crate::compiler::types::ExecutionContext;
use crate::diagnostic::Diagnostic;
use crate::parser::keywords::reserved_word;

pub(super) fn valid_name(name: &str) -> bool {
    let mut characters = name.chars();
    matches!(characters.next(), Some('a'..='z' | '_'))
        && characters.all(|character| matches!(character, 'a'..='z' | '0'..='9' | '_'))
}

/// 用户标识符：ASCII 部分沿用 `valid_name` 的小写约定，同时允许中文等
/// 非 ASCII 字母；这些名字会在代码生成前由 `compiler::rename` 换成随机 ASCII 名。
///
/// 模块解析会把名字限定为 `模块/名称`（文件类）或 `模块.名称`（内部名字），
/// 因此这里按 `/` 与 `.` 分段检查，每个分段都必须合法。
pub(crate) fn valid_user_name(name: &str) -> bool {
    !name.is_empty() && name.split(['/', '.']).all(valid_user_name_segment)
}

fn valid_user_name_segment(segment: &str) -> bool {
    let mut characters = segment.chars();
    let Some(first) = characters.next() else {
        return false;
    };
    let first_ok =
        first == '_' || first.is_ascii_lowercase() || (!first.is_ascii() && first.is_alphabetic());
    first_ok
        && characters.all(|character| {
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
    crate::ast::NbtPath::parse(path).is_ok()
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

pub(crate) fn windows_reserved_name(name: &str) -> bool {
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
    } else if name
        .split(['/', '.'])
        .any(|segment| segment.chars().count() > 32)
    {
        diagnostics.push(Diagnostic::new(
            format!("{kind}名 `{name}` 的每一段都不能超过 32 个字符"),
            span,
        ));
    } else if kind == "函数" && name.split(['/', '.']).any(windows_reserved_name) {
        diagnostics.push(Diagnostic::new(
            format!("函数名 `{name}` 会与 Windows 设备名冲突"),
            span,
        ));
    } else if name.rsplit(['/', '.']).next().is_some_and(reserved_word) {
        diagnostics.push(Diagnostic::new(
            format!("`{name}` 含有保留字，不能用作{kind}名"),
            span,
        ));
    }
}

/// 从函数属性推导它需要的最低执行上下文。
pub(super) fn function_context(function: &Function) -> ExecutionContext {
    use crate::ast::Attribute;

    if function.attributes.contains(&Attribute::Player) {
        ExecutionContext::Player
    } else if function.attributes.contains(&Attribute::NonPlayer) {
        ExecutionContext::Mob
    } else if function.attributes.contains(&Attribute::Entity) {
        ExecutionContext::Entity
    } else {
        ExecutionContext::None
    }
}
