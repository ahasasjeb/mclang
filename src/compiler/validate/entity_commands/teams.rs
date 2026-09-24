use super::*;

pub(super) fn validate_team(
    operation: &TeamOperation,
    span: Span,
    ctx: ValidationContext<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let name = match operation {
        TeamOperation::List(name) => name.as_deref(),
        TeamOperation::Add { name, .. }
        | TeamOperation::Remove(name)
        | TeamOperation::Empty(name)
        | TeamOperation::Join { name, .. }
        | TeamOperation::Modify { name, .. } => Some(name.as_str()),
        TeamOperation::Leave(_) => None,
    };
    if let Some(name) = name
        && !command_word(name)
    {
        diagnostics.push(Diagnostic::new(
            format!("队伍名称 `{name}` 只能包含字母、数字、下划线、连字符、点与加号"),
            span,
        ));
    }
    match operation {
        TeamOperation::Add {
            display: Some(component),
            ..
        } => validate_component(component, ctx, diagnostics),
        TeamOperation::Join { members, .. } => {
            if let Some(members) = members {
                validate_members(members, span, ctx, diagnostics);
            } else {
                require_sender(false, span, ctx, diagnostics);
            }
        }
        TeamOperation::Leave(members) => validate_members(members, span, ctx, diagnostics),
        TeamOperation::Modify { option, .. } => match option {
            TeamOption::DisplayName(component)
            | TeamOption::Prefix(component)
            | TeamOption::Suffix(component) => validate_component(component, ctx, diagnostics),
            TeamOption::Color(color) if color != "reset" => {
                validate_enum("team_color", "队伍颜色", color, span, diagnostics)
            }
            _ => {}
        },
        _ => {}
    }
}

fn validate_members(
    members: &TeamMembers,
    span: Span,
    ctx: ValidationContext<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match members {
        TeamMembers::Entities(target) => {
            entity_target(target, false, false, span, ctx, diagnostics)
        }
        TeamMembers::Name(name)
            if name != "*" && (!command_word(name) || name.starts_with('@')) =>
        {
            diagnostics.push(Diagnostic::new(
                format!("无效成员名称 `{name}`；选择器请使用已声明查询"),
                span,
            ))
        }
        _ => {}
    }
}

fn command_word(value: &str) -> bool {
    !value.is_empty()
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "_-.+".contains(c))
}

pub(super) fn validate_waypoint(
    operation: &WaypointOperation,
    span: Span,
    ctx: ValidationContext<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let target = match operation {
        WaypointOperation::List => return,
        WaypointOperation::Color { target, color } => {
            validate_enum("team_color", "路径点颜色", color, span, diagnostics);
            target
        }
        WaypointOperation::Hex { target, color } => {
            if !matches!(color.len(), 3 | 6) || !color.bytes().all(|c| c.is_ascii_hexdigit()) {
                diagnostics.push(Diagnostic::new(
                    format!("路径点颜色 `{color}` 必须是三位或六位 RGB 十六进制（不带 #）"),
                    span,
                ));
            }
            target
        }
        WaypointOperation::Style { target, style } => {
            resource_id(style, span, diagnostics);
            target
        }
        WaypointOperation::ResetColor(target) | WaypointOperation::ResetStyle(target) => target,
    };
    entity_target(target, true, false, span, ctx, diagnostics);
}
