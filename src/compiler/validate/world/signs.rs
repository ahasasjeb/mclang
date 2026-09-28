use crate::ast::{BlockStateValue, NbtEntry, NbtValue, NbtValueKind, Span};
use crate::diagnostic::Diagnostic;

/// 26.3 的 `SignBlockEntity` 默认关闭点击命令；只检查能在右键时执行的行级事件。
pub(super) fn warn_disabled_sign_commands(
    block: &BlockStateValue,
    nbt: Option<&NbtValue>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !is_vanilla_sign(&block.id) {
        return;
    }
    let Some(nbt) = nbt else { return };
    let Some(entries) = compound_entries(nbt) else {
        return;
    };
    let click_span = ["front_text", "back_text"]
        .into_iter()
        .filter_map(|face| field(entries, face))
        .find_map(run_command_span);
    let Some(click_span) = click_span else {
        return;
    };

    let enabled = field(entries, "allow_op_features");
    if enabled.is_some_and(nbt_boolean_true) {
        return;
    }
    diagnostics.push(Diagnostic::warning(
        format!(
            "命令木牌 `{}` 含 `run_command` 点击事件，但 NBT `allow_op_features` 未设为 true；26.3 默认关闭，玩家右键不会执行命令",
            block.id
        ),
        enabled.map_or(click_span, |value| value.span),
    ));
}

fn is_vanilla_sign(id: &str) -> bool {
    let path = match id.strip_prefix("minecraft:") {
        Some(path) => path,
        None if !id.contains(':') && !id.starts_with('#') => id,
        None => return false,
    };
    path.ends_with("_sign")
}

fn run_command_span(sign_text: &NbtValue) -> Option<Span> {
    let messages = field(compound_entries(sign_text)?, "messages")?;
    let NbtValueKind::List(lines) = &messages.kind else {
        return None;
    };
    lines.iter().find_map(|line| {
        let click = field(compound_entries(line)?, "click_event")?;
        let action = field(compound_entries(click)?, "action")?;
        matches!(&action.kind, NbtValueKind::String(value) if value == "run_command")
            .then_some(action.span)
    })
}

fn compound_entries(value: &NbtValue) -> Option<&[NbtEntry]> {
    match &value.kind {
        NbtValueKind::Compound(entries) => Some(entries),
        _ => None,
    }
}

fn field<'a>(entries: &'a [NbtEntry], key: &str) -> Option<&'a NbtValue> {
    entries
        .iter()
        .find(|entry| entry.key == key)
        .map(|entry| &entry.value)
}

/// `TagValueInput.getBooleanOr` reads a numeric tag through its byte value.
fn nbt_boolean_true(value: &NbtValue) -> bool {
    match value.kind {
        NbtValueKind::Byte(number) => number != 0,
        NbtValueKind::Short(number) => number as i8 != 0,
        NbtValueKind::Int(number) => number as i8 != 0,
        NbtValueKind::Long(number) => number as i8 != 0,
        NbtValueKind::Float(number) => number as i32 as i8 != 0,
        NbtValueKind::Double(number) => number as i32 as i8 != 0,
        _ => false,
    }
}
