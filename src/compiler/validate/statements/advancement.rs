use super::*;

/// `nbt { ... }`：把具名 NBT 合并到当前实体，键对照 26.3 源码标签表检查。
pub(super) fn validate_nbt_merge(
    nbt: &NbtValue,
    span: Span,
    ctx: ValidationContext<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if ctx.context != ExecutionContext::Mob {
        diagnostics.push(Diagnostic::new(
            "nbt/数据 通过 data 命令合并实体 NBT，Minecraft 不允许修改玩家数据；只能在确定不是玩家的实体上下文中使用（非玩家查询的 each、非玩家 spawn，或 @non_player 函数）",
            span,
        ));
        return;
    }
    crate::compiler::validate::entity_nbt::validate_entity_nbt(
        nbt,
        ctx.entity_type,
        span,
        diagnostics,
    );
}

/// `advancement.grant/revoke`：目标是玩家，进度引用必须可解析。
pub(super) fn validate_advancement_action(
    targets: &Holder,
    advancement: Option<&AdvancementReference>,
    criterion: Option<&str>,
    criterion_span: Option<Span>,
    span: Span,
    ctx: ValidationContext<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match targets {
        Holder::SelfEntity => {
            if ctx.context != ExecutionContext::Player {
                diagnostics.push(Diagnostic::new(
                    "advancement 目标 self/自身 需要玩家执行上下文；请放入玩家查询的 each 块，或给函数添加 @player",
                    span,
                ));
            }
        }
        Holder::Origin => {
            if !ctx.context.is_entity() {
                diagnostics.push(Diagnostic::new(
                    "advancement 目标 origin/投掷者 需要实体执行上下文",
                    span,
                ));
            }
        }
        Holder::Query(name, query_span) => {
            require_player_query(name, *query_span, ctx, diagnostics);
        }
    }
    if let Some(advancement) = advancement {
        if advancement.external {
            validate_id(
                "advancement",
                "进度",
                &advancement.name,
                advancement.span,
                diagnostics,
            );
        } else if !ctx
            .symbols
            .advancements
            .contains_key(advancement.name.as_str())
            && !ctx
                .symbols
                .advancement_resources
                .contains(advancement.name.as_str())
        {
            diagnostics.push(Diagnostic::new(
                format!("找不到进度 `{}`", advancement.name),
                advancement.span,
            ));
        }
    }
    if let Some(criterion) = criterion {
        validate_identifier(
            "准则",
            criterion,
            criterion_span.unwrap_or(span),
            diagnostics,
        );
    }
}

pub(super) fn validate_effect_give(
    target: &str,
    effect: &str,
    duration: EffectDuration,
    amplifier: Option<u32>,
    span: Span,
    ctx: ValidationContext<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    query_reference(target, span, ctx, diagnostics);
    validate_id("mob_effect", "效果", effect, span, diagnostics);
    if let EffectDuration::Seconds(seconds) = duration
        && !(1..=1_000_000).contains(&seconds)
    {
        diagnostics.push(Diagnostic::new(
            "effect 持续秒数必须是 1 到 1000000 之间的整数",
            span,
        ));
    }
    if let Some(amplifier) = amplifier
        && amplifier > 255
    {
        diagnostics.push(Diagnostic::new(
            "effect 等级必须是 0 到 255 之间的整数",
            span,
        ));
    }
}

pub(super) fn validate_effect_clear(
    target: &str,
    effect: Option<&str>,
    span: Span,
    ctx: ValidationContext<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    query_reference(target, span, ctx, diagnostics);
    if let Some(effect) = effect {
        validate_id("mob_effect", "效果", effect, span, diagnostics);
    }
}

pub(super) fn validate_xp_change(
    target: &str,
    operation: XpOperation,
    amount: i32,
    span: Span,
    ctx: ValidationContext<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    require_player_query(target, span, ctx, diagnostics);
    if operation == XpOperation::Set && amount < 0 {
        diagnostics.push(Diagnostic::new("xp.set 的数量必须是非负整数", span));
    }
}

pub(super) fn validate_clear_inventory(
    target: &str,
    item: Option<&str>,
    max_count: Option<u32>,
    span: Span,
    ctx: ValidationContext<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    require_player_query(target, span, ctx, diagnostics);
    if let Some(item) = item {
        validate_id("item", "物品", item, span, diagnostics);
    }
    if let Some(max_count) = max_count
        && max_count > i32::MAX as u32
    {
        diagnostics.push(Diagnostic::new("clear 最大数量不能超过 2147483647", span));
    }
}
