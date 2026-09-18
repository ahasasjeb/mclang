use super::*;

pub(super) fn validate_message(
    target: &MessageTarget,
    component: &TextComponent,
    span: Span,
    ctx: ValidationContext<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if matches!(target, MessageTarget::SelfEntity)
        && !ctx.context.satisfies(ExecutionContext::Player)
    {
        diagnostics.push(Diagnostic::new("message.self 需要玩家执行上下文", span));
    }
    if let MessageTarget::Nearest { within } = target
        && (*within == 0 || *within > 30_000_000)
    {
        diagnostics.push(Diagnostic::new(
            "最近玩家消息范围必须是 1 到 30000000",
            span,
        ));
    }
    if let MessageTarget::Query { name, name_span } = target
        && !ctx.symbols.queries.contains_key(name.as_str())
    {
        diagnostics.push(Diagnostic::new(
            format!("找不到实体查询 `{name}`"),
            *name_span,
        ));
    }
    crate::compiler::validate::components::validate_component(component, ctx, diagnostics);
}

#[allow(clippy::too_many_arguments)]
pub(super) fn validate_play_sound(
    sound: &str,
    source: &str,
    targets: Option<&str>,
    position: Option<&PositionValue>,
    volume: Option<&str>,
    pitch: Option<&str>,
    min_volume: Option<&str>,
    span: Span,
    ctx: ValidationContext<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match targets {
        None => {
            if !ctx.context.satisfies(ExecutionContext::Player) {
                diagnostics.push(Diagnostic::new("sound.self 需要玩家执行上下文", span));
            }
        }
        Some(query) => {
            require_player_query(query, span, ctx, diagnostics);
        }
    }
    validate_id("sound", "声音", sound, span, diagnostics);
    validate_enum("sound_source", "声音分类", source, span, diagnostics);
    if let Some(position) = position {
        crate::compiler::validate::world::validate_position_value(position, diagnostics);
    }
    for (label, value, max) in [
        ("音量", volume, None),
        ("音调", pitch, Some(2.0)),
        ("最小音量", min_volume, Some(1.0)),
    ] {
        let Some(value) = value else {
            continue;
        };
        let parsed = value.parse::<f64>().ok();
        let valid = parsed.is_some_and(|value| value >= 0.0 && max.is_none_or(|max| value <= max));
        if !valid {
            let range = match max {
                Some(max) => format!("0 到 {max}"),
                None => "非负数字".to_owned(),
            };
            diagnostics.push(Diagnostic::new(format!("声音{label}必须是{range}"), span));
        }
    }
}

pub(super) fn validate_each<'a>(
    query: &str,
    body: &'a [Statement],
    locals: &HashSet<&'a str>,
    span: Span,
    ctx: ValidationContext<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let query_decl = ctx.symbols.queries.get(query);
    if query_decl.is_none() {
        diagnostics.push(Diagnostic::new(format!("找不到实体查询 `{query}`"), span));
    }
    let body_context = query_decl.map_or(ExecutionContext::Mob, |query| {
        if query.entity_type == "minecraft:player" {
            ExecutionContext::Player
        } else {
            ExecutionContext::Mob
        }
    });
    let mut body_locals = locals.clone();
    validate_statements(
        body,
        &mut body_locals,
        ctx.symbols,
        body_context,
        query_decl.map(|query| query.entity_type.as_str()),
        ctx.return_rules.nested(),
        diagnostics,
    );
}

pub(super) fn validate_in_dimension<'a>(
    dimension: &str,
    body: &'a [Statement],
    locals: &HashSet<&'a str>,
    span: Span,
    ctx: ValidationContext<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    validate_id("dimension", "维度", dimension, span, diagnostics);
    let mut body_locals = locals.clone();
    validate_statements(
        body,
        &mut body_locals,
        ctx.symbols,
        ctx.context,
        ctx.entity_type,
        ctx.return_rules.nested(),
        diagnostics,
    );
}

pub(super) fn validate_spawn<'a>(
    entity_type: &str,
    position: Option<&PositionValue>,
    body: &'a [Statement],
    locals: &HashSet<&'a str>,
    span: Span,
    ctx: ValidationContext<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    validate_id("entity_type", "实体类型", entity_type, span, diagnostics);
    if crate::compiler::validate::rules::valid_resource_location(entity_type)
        && non_summonable_entity(entity_type)
    {
        diagnostics.push(Diagnostic::new(
            format!("Minecraft 的 /summon 不支持实体类型 `{entity_type}`"),
            span,
        ));
    }
    if let Some(position) = position {
        crate::compiler::validate::world::validate_position_value(position, diagnostics);
    }
    let mut body_locals = locals.clone();
    validate_statements(
        body,
        &mut body_locals,
        ctx.symbols,
        ExecutionContext::Mob,
        Some(entity_type),
        ctx.return_rules.nested(),
        diagnostics,
    );
}

/// 26.3 的 `EntityTypes` 用 `noSummon` 标记不可召唤的类型。
pub(super) fn non_summonable_entity(entity_type: &str) -> bool {
    matches!(entity_type, "minecraft:player" | "minecraft:fishing_bobber")
}
