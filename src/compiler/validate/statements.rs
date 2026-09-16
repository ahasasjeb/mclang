//! 函数体内的语句校验：控制流、实体操作、调用、赋值与作用域。
//!
//! 校验器遍历树时携带三层信息：当前可见的局部变量、符号表
//! （[`StatementSymbols`]）和当前执行上下文/返回规则。后两者放进
//! [`ValidationContext`]，避免每个辅助函数都接收一长串参数。每个语句种类对应
//! 一个独立的小函数，`validate_statement` 只负责分派。
//!
//! 表达式与条件的检查在 [`super::expressions`]。

use std::collections::HashSet;

use crate::ast::{
    AdvancementReference, AssignOp, CallTarget, Condition, DataSlotKind, EffectDuration,
    EntityQueryDecl, Expr, GiveItem, GiveTarget, Holder, MessageTarget, ReturnKind, ScoreTarget,
    SelfAction, Span, Statement, StatementKind, TeleportDestination, XpOperation,
};
use crate::compiler::constant::constant_value;
use crate::compiler::types::{ExecutionContext, ReturnRules, StatementSymbols};
use crate::diagnostic::Diagnostic;

use super::expressions::{
    execution_context_label, validate_call_context, validate_condition, validate_expr,
};
use super::items::validate_give_count;
use super::rules::{
    valid_entity_tag, valid_resource_location, valid_sound_source, valid_text_color,
    validate_identifier,
};
use super::tags::reachable_functions;

/// 遍历函数体时保持不变的校验环境。
#[derive(Clone, Copy)]
pub(super) struct ValidationContext<'a, 'b> {
    pub(super) symbols: &'a StatementSymbols<'b>,
    pub(super) context: ExecutionContext,
    pub(super) return_rules: ReturnRules,
}

/// 收集函数体内的全部 `let` 声明，并报告重名、与全局计分变量或参数冲突。
pub(super) fn collect_local_declarations<'a>(
    statements: &'a [Statement],
    scores: &HashSet<&str>,
    parameters: &HashSet<&str>,
    locals: &mut HashSet<&'a str>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for statement in statements {
        match &statement.kind {
            StatementKind::Let { name, .. } => {
                validate_identifier("局部变量", name, statement.span, diagnostics);
                if scores.contains(name.as_str()) {
                    diagnostics.push(Diagnostic::new(
                        format!("局部变量 `{name}` 与全局计分变量重名"),
                        statement.span,
                    ));
                }
                if parameters.contains(name.as_str()) {
                    diagnostics.push(Diagnostic::new(
                        format!("局部变量 `{name}` 与函数参数重名"),
                        statement.span,
                    ));
                }
                if !locals.insert(name) {
                    diagnostics.push(Diagnostic::new(
                        format!("函数内重复声明局部变量 `{name}`"),
                        statement.span,
                    ));
                }
            }
            StatementKind::If {
                then_body,
                else_body,
                ..
            } => {
                collect_local_declarations(then_body, scores, parameters, locals, diagnostics);
                collect_local_declarations(else_body, scores, parameters, locals, diagnostics);
            }
            StatementKind::Execute { body, .. }
            | StatementKind::Each { body, .. }
            | StatementKind::InDimension { body, .. }
            | StatementKind::Spawn { body, .. }
            | StatementKind::While { body, .. } => {
                collect_local_declarations(body, scores, parameters, locals, diagnostics);
            }
            StatementKind::Run(_)
            | StatementKind::Give { .. }
            | StatementKind::EffectGive { .. }
            | StatementKind::EffectClear { .. }
            | StatementKind::XpChange { .. }
            | StatementKind::StopwatchAction { .. }
            | StatementKind::ClearInventory { .. }
            | StatementKind::SetBlock { .. }
            | StatementKind::Fill { .. }
            | StatementKind::FillBiome { .. }
            | StatementKind::Clone { .. }
            | StatementKind::PlaceFeature { .. }
            | StatementKind::PlaceJigsaw { .. }
            | StatementKind::PlaceStructure { .. }
            | StatementKind::PlaceTemplate { .. }
            | StatementKind::ForceLoad(_)
            | StatementKind::TimeAction { .. }
            | StatementKind::Weather { .. }
            | StatementKind::GameRuleSet { .. }
            | StatementKind::WorldBorder(_)
            | StatementKind::Locate { .. }
            | StatementKind::SelfAction(_)
            | StatementKind::Message { .. }
            | StatementKind::PlaySound { .. }
            | StatementKind::Call { .. }
            | StatementKind::Schedule { .. }
            | StatementKind::ScheduleClear { .. }
            | StatementKind::Assign { .. }
            | StatementKind::ScoreSet { .. }
            | StatementKind::ScoreReset { .. }
            | StatementKind::Teleport { .. }
            | StatementKind::AdvancementAction { .. }
            | StatementKind::Return(_) => {}
        }
    }
}

pub(super) fn validate_statements<'a>(
    statements: &'a [Statement],
    locals: &mut HashSet<&'a str>,
    symbols: &StatementSymbols<'_>,
    context: ExecutionContext,
    return_rules: ReturnRules,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let ctx = ValidationContext {
        symbols,
        context,
        return_rules,
    };
    for statement in statements {
        validate_statement(statement, locals, ctx, diagnostics);
    }
}

fn validate_statement<'a>(
    statement: &'a Statement,
    locals: &mut HashSet<&'a str>,
    ctx: ValidationContext<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match &statement.kind {
        StatementKind::Run(_) => {}
        StatementKind::Give {
            target,
            item,
            count,
            count_span,
        } => validate_give_statement(
            target,
            item,
            *count,
            *count_span,
            statement.span,
            ctx,
            diagnostics,
        ),
        StatementKind::SelfAction(action) => {
            validate_self_action(action, statement.span, ctx, diagnostics);
        }
        StatementKind::EffectGive {
            target,
            effect,
            duration,
            amplifier,
            ..
        } => validate_effect_give(
            target,
            effect,
            *duration,
            *amplifier,
            statement.span,
            ctx,
            diagnostics,
        ),
        StatementKind::EffectClear { target, effect } => {
            validate_effect_clear(target, effect.as_deref(), statement.span, ctx, diagnostics);
        }
        StatementKind::XpChange {
            target,
            operation,
            amount,
            ..
        } => validate_xp_change(
            target,
            *operation,
            *amount,
            statement.span,
            ctx,
            diagnostics,
        ),
        StatementKind::StopwatchAction { id, .. } => {
            validate_stopwatch_id(id, statement.span, diagnostics);
        }
        StatementKind::ClearInventory {
            target,
            item,
            max_count,
        } => validate_clear_inventory(
            target,
            item.as_deref(),
            *max_count,
            statement.span,
            ctx,
            diagnostics,
        ),
        StatementKind::Message { target, color, .. } => {
            validate_message(target, color.as_deref(), statement.span, ctx, diagnostics);
        }
        StatementKind::PlaySound { sound, source } => {
            validate_play_sound(sound, source, statement.span, ctx, diagnostics);
        }
        StatementKind::Each { query, body } => {
            validate_each(query, body, locals, statement.span, ctx, diagnostics);
        }
        StatementKind::InDimension { dimension, body } => {
            validate_in_dimension(dimension, body, locals, statement.span, ctx, diagnostics);
        }
        StatementKind::Spawn { entity_type, body } => {
            validate_spawn(entity_type, body, locals, statement.span, ctx, diagnostics);
        }
        StatementKind::Return(kind) => {
            validate_return(kind, locals, statement.span, ctx, diagnostics);
        }
        StatementKind::Call { target, arguments } => match target {
            CallTarget::Function(function) => {
                validate_call(
                    function,
                    arguments,
                    locals,
                    statement.span,
                    ctx,
                    diagnostics,
                );
            }
            CallTarget::Tag(tag) => {
                validate_tag_call(tag, arguments, locals, statement.span, ctx, diagnostics);
            }
        },
        StatementKind::Schedule { target, .. } => match target {
            CallTarget::Function(function) => {
                validate_schedule(function, statement.span, ctx, diagnostics);
            }
            CallTarget::Tag(tag) => {
                validate_tag_schedule(tag, statement.span, ctx, diagnostics);
            }
        },
        StatementKind::ScheduleClear { function } => {
            validate_schedule(function, statement.span, ctx, diagnostics);
        }
        StatementKind::Assign {
            target,
            operation,
            value,
        } => {
            validate_assign(
                target,
                *operation,
                value,
                locals,
                statement.span,
                ctx,
                diagnostics,
            );
        }
        StatementKind::ScoreSet { target, value } => {
            validate_score_target(target, statement.span, ctx, diagnostics);
            validate_expr(value, locals, ctx, diagnostics);
        }
        StatementKind::ScoreReset { target } => {
            validate_score_target(target, statement.span, ctx, diagnostics);
        }
        StatementKind::Teleport {
            targets,
            destination,
        } => {
            validate_teleport(targets, destination, statement.span, ctx, diagnostics);
        }
        StatementKind::AdvancementAction {
            targets,
            advancement,
            criterion,
            criterion_span,
            ..
        } => {
            validate_advancement_action(
                targets,
                advancement.as_ref(),
                criterion.as_deref(),
                *criterion_span,
                statement.span,
                ctx,
                diagnostics,
            );
        }
        StatementKind::Let { name, value, .. } => {
            validate_let(name, value, locals, ctx, diagnostics);
        }
        StatementKind::If {
            condition,
            then_body,
            else_body,
        } => {
            validate_if(condition, then_body, else_body, locals, ctx, diagnostics);
        }
        StatementKind::While { condition, body } => {
            validate_while(condition, body, locals, ctx, diagnostics);
        }
        StatementKind::Execute { body, .. } => {
            validate_execute(body, locals, ctx, diagnostics);
        }
        StatementKind::SetBlock { .. }
        | StatementKind::Fill { .. }
        | StatementKind::FillBiome { .. }
        | StatementKind::Clone { .. }
        | StatementKind::PlaceFeature { .. }
        | StatementKind::PlaceJigsaw { .. }
        | StatementKind::PlaceStructure { .. }
        | StatementKind::PlaceTemplate { .. }
        | StatementKind::ForceLoad(_)
        | StatementKind::TimeAction { .. }
        | StatementKind::Weather { .. }
        | StatementKind::GameRuleSet { .. }
        | StatementKind::WorldBorder(_)
        | StatementKind::Locate { .. } => {
            super::world::validate_world_statement(statement, diagnostics);
        }
    }
}

fn validate_give_statement(
    target: &GiveTarget,
    item: &GiveItem,
    count: Option<u32>,
    count_span: Option<Span>,
    span: Span,
    ctx: ValidationContext<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match target {
        GiveTarget::Query(name) => match ctx.symbols.queries.get(name.as_str()) {
            None => diagnostics.push(Diagnostic::new(format!("找不到实体查询 `{name}`"), span)),
            Some(query) if query.entity_type != "minecraft:player" => {
                diagnostics.push(Diagnostic::new(
                    format!("give 目标查询 `{name}` 必须匹配 minecraft:player"),
                    span,
                ));
            }
            Some(_) => {}
        },
        GiveTarget::Origin => {
            if !ctx.context.is_entity() {
                diagnostics.push(Diagnostic::new(
                    "give 的 origin/投掷者目标需要实体执行上下文",
                    span,
                ));
            }
        }
    }
    match item {
        GiveItem::Definition(name) => match ctx.symbols.item_stacks.get(name.as_str()) {
            None => diagnostics.push(Diagnostic::new(format!("找不到物品定义 `{name}`"), span)),
            Some(declaration) => {
                validate_give_count(declaration, count, span, count_span, diagnostics);
            }
        },
        GiveItem::SelfItem => {
            if !ctx.context.is_entity() {
                diagnostics.push(Diagnostic::new(
                    "self.item 需要实体执行上下文；请放入 each/spawn 块，或给函数添加 @entity",
                    span,
                ));
            }
            if count.is_some() {
                diagnostics.push(Diagnostic::new(
                    "原样给予 self.item 时不能指定数量",
                    count_span.unwrap_or(span),
                ));
            }
        }
    }
}

fn validate_self_action(
    action: &SelfAction,
    span: Span,
    ctx: ValidationContext<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let method = match action {
        SelfAction::AddTag(_) => "add_tag",
        SelfAction::RemoveTag(_) => "remove_tag",
        SelfAction::SetInvulnerable(_) => "set_invulnerable",
        SelfAction::SetNoGravity(_) => "set_no_gravity",
        SelfAction::SaveItems(_) => "save_items",
        SelfAction::RestoreItems(_) => "restore_items",
        SelfAction::RemovePreservingItems(_) => "remove_preserving_items",
        SelfAction::RemovePreservingSlot { .. } => "remove_preserving_items",
        SelfAction::GiveItem { .. } => "give_item",
        SelfAction::ClearItems => "clear_items",
        SelfAction::DataStore { .. } => "deposit",
        SelfAction::DataLoad { .. } => "withdraw",
        SelfAction::DataClear { .. } => "remove_data",
        SelfAction::Remove => "remove",
    };
    let (required, message) = match action {
        SelfAction::GiveItem { .. } => (
            ExecutionContext::Player,
            format!(
                "self.{method} 需要玩家执行上下文；请放入玩家 query 的 each 块，或给函数添加 @player"
            ),
        ),
        SelfAction::SetInvulnerable(_)
        | SelfAction::SetNoGravity(_)
        | SelfAction::SaveItems(_)
        | SelfAction::RestoreItems(_)
        | SelfAction::RemovePreservingItems(_)
        | SelfAction::RemovePreservingSlot { .. }
        | SelfAction::ClearItems
        | SelfAction::DataStore { .. }
        | SelfAction::DataLoad { .. }
        | SelfAction::DataClear { .. } => (
            ExecutionContext::Mob,
            format!(
                "self.{method} 通过 data 命令修改实体 NBT，Minecraft 不允许修改玩家数据；只能在确定不是玩家的实体上下文中使用（非玩家查询的 each、非玩家 spawn，或 @non_player 函数）"
            ),
        ),
        SelfAction::AddTag(_) | SelfAction::RemoveTag(_) | SelfAction::Remove => (
            ExecutionContext::Entity,
            format!("self.{method} 需要实体执行上下文；请放入 each/spawn 块，或给函数添加 @entity"),
        ),
    };
    if !ctx.context.satisfies(required) {
        diagnostics.push(Diagnostic::new(message, span));
    }
    match action {
        SelfAction::AddTag(tag) | SelfAction::RemoveTag(tag) => {
            if !valid_entity_tag(tag) {
                diagnostics.push(Diagnostic::new(format!("`{tag}` 不是有效的实体标签"), span));
            }
        }
        SelfAction::SaveItems(storage)
        | SelfAction::RestoreItems(storage)
        | SelfAction::RemovePreservingItems(storage) => {
            if !ctx.symbols.storages.contains(storage.as_str()) {
                diagnostics.push(Diagnostic::new(format!("找不到物品存储 `{storage}`"), span));
            }
        }
        SelfAction::DataStore {
            slot,
            slot_span,
            query,
            query_span,
        }
        | SelfAction::DataLoad {
            slot,
            slot_span,
            query,
            query_span,
        }
        | SelfAction::RemovePreservingSlot {
            slot,
            slot_span,
            query,
            query_span,
        } => {
            validate_data_slot(slot, *slot_span, ctx, diagnostics);
            validate_data_slot_query(slot, *slot_span, query, *query_span, ctx, diagnostics);
        }
        SelfAction::DataClear { slot, slot_span } => {
            validate_data_slot(slot, *slot_span, ctx, diagnostics);
        }
        SelfAction::GiveItem {
            item,
            count,
            count_span,
        } => match ctx.symbols.item_stacks.get(item.as_str()) {
            None => diagnostics.push(Diagnostic::new(format!("找不到物品定义 `{item}`"), span)),
            Some(declaration) => {
                validate_give_count(declaration, *count, span, *count_span, diagnostics);
            }
        },
        SelfAction::SetInvulnerable(_)
        | SelfAction::SetNoGravity(_)
        | SelfAction::ClearItems
        | SelfAction::Remove => {}
    }
}

/// 数据槽引用必须已声明。
fn validate_data_slot(
    name: &str,
    span: Span,
    ctx: ValidationContext<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !ctx.symbols.data_slots.contains_key(name) {
        diagnostics.push(Diagnostic::new(format!("找不到数据槽 `{name}`"), span));
    }
}

/// `deposit`/`withdraw` 的另一侧查询：必须唯一，且实体类型与数据槽种类相容。
fn validate_data_slot_query(
    slot: &str,
    slot_span: Span,
    query: &str,
    query_span: Span,
    ctx: ValidationContext<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(declaration) = ctx.symbols.data_slots.get(slot) else {
        return;
    };
    let Some(target) = ctx.symbols.queries.get(query) else {
        diagnostics.push(Diagnostic::new(
            format!("找不到实体查询 `{query}`"),
            query_span,
        ));
        return;
    };
    if target.limit != Some(1) {
        diagnostics.push(Diagnostic::new(
            format!("数据槽操作需要 limit(1) 的单个实体查询 `{query}`"),
            query_span,
        ));
    }
    match declaration.kind {
        DataSlotKind::ItemData if target.entity_type != "minecraft:item" => {
            diagnostics.push(Diagnostic::new(
                format!(
                    "物品数据槽 `{slot}` 只能配合 minecraft:item 查询使用；`{query}` 匹配 {}",
                    target.entity_type
                ),
                query_span.merge(slot_span),
            ));
        }
        DataSlotKind::EntityData if target.entity_type == "minecraft:player" => {
            diagnostics.push(Diagnostic::new(
                format!("实体数据槽 `{slot}` 不能指向玩家：Minecraft 拒绝修改玩家 NBT"),
                query_span.merge(slot_span),
            ));
        }
        _ => {}
    }
}

/// 计分目标的持有者与目标名都必须已声明，且持有者类型与当前上下文相容。
pub(super) fn validate_score_target(
    target: &ScoreTarget,
    span: Span,
    ctx: ValidationContext<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !ctx.symbols.objectives.contains(target.objective.as_str()) {
        diagnostics.push(Diagnostic::new(
            format!("找不到计分板目标 `{}`", target.objective),
            target.objective_span,
        ));
    }
    validate_holder(&target.holder, span, ctx, diagnostics);
}

/// 持有者引用：`self`/`origin` 需要实体上下文，查询必须已声明。
fn validate_holder(
    holder: &Holder,
    span: Span,
    ctx: ValidationContext<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match holder {
        Holder::SelfEntity => {
            if !ctx.context.is_entity() {
                diagnostics.push(Diagnostic::new(
                    "持有者 self/自身 需要实体执行上下文；请放入 each/spawn 块，或给函数添加 @entity",
                    span,
                ));
            }
        }
        Holder::Origin => {
            if !ctx.context.is_entity() {
                diagnostics.push(Diagnostic::new(
                    "持有者 origin/投掷者 需要实体执行上下文",
                    span,
                ));
            }
        }
        Holder::Query(name, query_span) => {
            if !ctx.symbols.queries.contains_key(name.as_str()) {
                diagnostics.push(Diagnostic::new(
                    format!("找不到实体查询 `{name}`"),
                    *query_span,
                ));
            }
        }
    }
}

/// `teleport(持有者, 落点)`：落点是坐标或 `limit(1)` 的单个实体查询。
fn validate_teleport(
    targets: &Holder,
    destination: &TeleportDestination,
    span: Span,
    ctx: ValidationContext<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    validate_holder(targets, span, ctx, diagnostics);
    match destination {
        TeleportDestination::Position(position) => {
            super::world::validate_block_position(position, diagnostics);
        }
        TeleportDestination::Entity { query, query_span } => {
            if !ctx.symbols.queries.contains_key(query.as_str()) {
                diagnostics.push(Diagnostic::new(
                    format!("找不到实体查询 `{query}`"),
                    *query_span,
                ));
                return;
            }
            if let Some(declaration) = ctx.symbols.queries.get(query.as_str())
                && declaration.limit != Some(1)
            {
                diagnostics.push(Diagnostic::new(
                    format!("teleport 的落点需要 limit(1) 的单个实体查询 `{query}`"),
                    *query_span,
                ));
            }
        }
    }
}

/// `advancement.grant/revoke`：目标是玩家，进度引用必须可解析。
fn validate_advancement_action(
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
            if !valid_resource_location(&advancement.name) {
                diagnostics.push(Diagnostic::new(
                    format!("`{}` 不是有效的进度资源位置", advancement.name),
                    advancement.span,
                ));
            }
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

fn validate_effect_give(
    target: &str,
    effect: &str,
    duration: EffectDuration,
    amplifier: Option<u32>,
    span: Span,
    ctx: ValidationContext<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    query_reference(target, span, ctx, diagnostics);
    if !valid_resource_location(effect) {
        diagnostics.push(Diagnostic::new(
            format!("`{effect}` 不是有效的效果资源位置"),
            span,
        ));
    }
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

fn validate_effect_clear(
    target: &str,
    effect: Option<&str>,
    span: Span,
    ctx: ValidationContext<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    query_reference(target, span, ctx, diagnostics);
    if let Some(effect) = effect
        && !valid_resource_location(effect)
    {
        diagnostics.push(Diagnostic::new(
            format!("`{effect}` 不是有效的效果资源位置"),
            span,
        ));
    }
}

fn validate_xp_change(
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

fn validate_clear_inventory(
    target: &str,
    item: Option<&str>,
    max_count: Option<u32>,
    span: Span,
    ctx: ValidationContext<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    require_player_query(target, span, ctx, diagnostics);
    if let Some(item) = item
        && !valid_resource_location(item)
    {
        diagnostics.push(Diagnostic::new(
            format!("`{item}` 不是有效的物品资源位置"),
            span,
        ));
    }
    if let Some(max_count) = max_count
        && max_count > i32::MAX as u32
    {
        diagnostics.push(Diagnostic::new("clear 最大数量不能超过 2147483647", span));
    }
}

/// 秒表 id 的公共校验，供语句与表达式共用。
pub(super) fn validate_stopwatch_id(id: &str, span: Span, diagnostics: &mut Vec<Diagnostic>) {
    if !valid_resource_location(id) {
        diagnostics.push(Diagnostic::new(
            format!("`{id}` 不是有效的秒表资源位置"),
            span,
        ));
    }
}

/// 解析实体查询引用；未声明时报告并返回 `None`。
fn query_reference<'b>(
    name: &str,
    span: Span,
    ctx: ValidationContext<'_, 'b>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<&'b EntityQueryDecl> {
    match ctx.symbols.queries.get(name).copied() {
        Some(query) => Some(query),
        None => {
            diagnostics.push(Diagnostic::new(format!("找不到实体查询 `{name}`"), span));
            None
        }
    }
}

/// 解析并要求查询匹配玩家；命令目标是玩家而查询不匹配时报告。
pub(super) fn require_player_query<'b>(
    name: &str,
    span: Span,
    ctx: ValidationContext<'_, 'b>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<&'b EntityQueryDecl> {
    let query = query_reference(name, span, ctx, diagnostics)?;
    if query.entity_type != "minecraft:player" {
        diagnostics.push(Diagnostic::new(
            format!("查询 `{name}` 必须匹配 minecraft:player"),
            span,
        ));
    }
    Some(query)
}

fn validate_tag_call<'a>(
    tag: &str,
    arguments: &'a [Expr],
    locals: &HashSet<&'a str>,
    span: Span,
    ctx: ValidationContext<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match ctx.symbols.function_tags.get(tag) {
        None => diagnostics.push(Diagnostic::new(format!("找不到函数标签 `{tag}`"), span)),
        Some(_) if !arguments.is_empty() => diagnostics.push(Diagnostic::new(
            "函数标签调用不接受参数，标签不能传递实参",
            span,
        )),
        Some(_) => {
            for function in reachable_functions(tag, ctx.symbols.function_tags) {
                let Some(signature) = ctx.symbols.functions.get(function) else {
                    continue;
                };
                if signature.parameters > 0 {
                    diagnostics.push(Diagnostic::new(
                        format!(
                            "函数标签 `{tag}` 中的函数 `{function}` 需要 {} 个参数，标签调用无法传递",
                            signature.parameters
                        ),
                        span,
                    ));
                }
                if !ctx.context.satisfies(signature.required_context)
                    && let Some((attribute, kind)) =
                        execution_context_label(signature.required_context)
                {
                    diagnostics.push(Diagnostic::new(
                        format!(
                            "函数标签 `{tag}` 中的 {attribute} 函数 `{function}` 需要{kind}执行上下文"
                        ),
                        span,
                    ));
                }
            }
        }
    }
    for argument in arguments {
        validate_expr(argument, locals, ctx, diagnostics);
    }
}

fn validate_tag_schedule(
    tag: &str,
    span: Span,
    ctx: ValidationContext<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !ctx.symbols.function_tags.contains_key(tag) {
        diagnostics.push(Diagnostic::new(format!("找不到函数标签 `{tag}`"), span));
        return;
    }
    for function in reachable_functions(tag, ctx.symbols.function_tags) {
        let Some(signature) = ctx.symbols.functions.get(function) else {
            continue;
        };
        if signature.required_context != ExecutionContext::None {
            diagnostics.push(Diagnostic::new(
                format!(
                    "不能调度函数标签 `{tag}` 中需要执行上下文的函数 `{function}`，调度不会保留实体或玩家"
                ),
                span,
            ));
        }
        if signature.parameters > 0 {
            diagnostics.push(Diagnostic::new(
                format!(
                    "不能调度函数标签 `{tag}` 中需要 {} 个参数的函数 `{function}`",
                    signature.parameters
                ),
                span,
            ));
        }
    }
}

fn validate_message(
    target: &MessageTarget,
    color: Option<&str>,
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
    if let Some(color) = color
        && !valid_text_color(color)
    {
        diagnostics.push(Diagnostic::new(
            format!("`{color}` 不是有效的文本颜色"),
            span,
        ));
    }
}

fn validate_play_sound(
    sound: &str,
    source: &str,
    span: Span,
    ctx: ValidationContext<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !ctx.context.satisfies(ExecutionContext::Player) {
        diagnostics.push(Diagnostic::new("sound.self 需要玩家执行上下文", span));
    }
    if !valid_resource_location(sound) {
        diagnostics.push(Diagnostic::new(
            format!("`{sound}` 不是有效的声音资源位置"),
            span,
        ));
    }
    if !valid_sound_source(source) {
        diagnostics.push(Diagnostic::new(
            format!("`{source}` 不是有效的声音分类"),
            span,
        ));
    }
}

fn validate_each<'a>(
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
        ctx.return_rules.nested(),
        diagnostics,
    );
}

fn validate_in_dimension<'a>(
    dimension: &str,
    body: &'a [Statement],
    locals: &HashSet<&'a str>,
    span: Span,
    ctx: ValidationContext<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !valid_resource_location(dimension) {
        diagnostics.push(Diagnostic::new(
            format!("`{dimension}` 不是有效的维度资源位置"),
            span,
        ));
    }
    let mut body_locals = locals.clone();
    validate_statements(
        body,
        &mut body_locals,
        ctx.symbols,
        ctx.context,
        ctx.return_rules.nested(),
        diagnostics,
    );
}

fn validate_spawn<'a>(
    entity_type: &str,
    body: &'a [Statement],
    locals: &HashSet<&'a str>,
    span: Span,
    ctx: ValidationContext<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !valid_resource_location(entity_type) {
        diagnostics.push(Diagnostic::new(
            format!("`{entity_type}` 不是有效的实体类型资源位置"),
            span,
        ));
    } else if non_summonable_entity(entity_type) {
        diagnostics.push(Diagnostic::new(
            format!("Minecraft 的 /summon 不支持实体类型 `{entity_type}`"),
            span,
        ));
    }
    let mut body_locals = locals.clone();
    validate_statements(
        body,
        &mut body_locals,
        ctx.symbols,
        ExecutionContext::Mob,
        ctx.return_rules.nested(),
        diagnostics,
    );
}

/// 26.3 的 `EntityTypes` 用 `noSummon` 标记不可召唤的类型。
fn non_summonable_entity(entity_type: &str) -> bool {
    matches!(entity_type, "minecraft:player" | "minecraft:fishing_bobber")
}

fn validate_return<'a>(
    kind: &'a ReturnKind,
    locals: &HashSet<&'a str>,
    span: Span,
    ctx: ValidationContext<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !ctx.return_rules.allowed_here {
        diagnostics.push(Diagnostic::new(
            "return 只能直接出现在函数代码块中，不能放在 if、while、each、spawn、in_dimension 或 execute 块内",
            span,
        ));
    }
    match (ctx.return_rules.returns_score, kind) {
        (true, ReturnKind::Value(value)) => validate_expr(value, locals, ctx, diagnostics),
        (true, ReturnKind::Run(_) | ReturnKind::Fail) => {}
        (true, ReturnKind::Void) => diagnostics.push(Diagnostic::new(
            "返回 score 的函数需要 `return <表达式>;`、`return run \"命令\";` 或 `return fail;`",
            span,
        )),
        (false, ReturnKind::Value(_)) => diagnostics.push(Diagnostic::new(
            "无返回值函数只能使用 `return;`、`return fail;` 或 `return run \"命令\";`",
            span,
        )),
        (false, ReturnKind::Void | ReturnKind::Run(_) | ReturnKind::Fail) => {}
    }
}

fn validate_call<'a>(
    function: &str,
    arguments: &'a [Expr],
    locals: &HashSet<&'a str>,
    span: Span,
    ctx: ValidationContext<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let Some(signature) = ctx.symbols.functions.get(function) {
        if arguments.len() != signature.parameters {
            diagnostics.push(Diagnostic::new(
                format!(
                    "函数 `{function}` 需要 {} 个参数，实际提供 {} 个",
                    signature.parameters,
                    arguments.len()
                ),
                span,
            ));
        }
        validate_call_context(function, *signature, span, ctx, diagnostics);
    } else {
        diagnostics.push(Diagnostic::new(format!("找不到函数 `{function}`"), span));
    }
    for argument in arguments {
        validate_expr(argument, locals, ctx, diagnostics);
    }
}

fn validate_schedule(
    function: &str,
    span: Span,
    ctx: ValidationContext<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match ctx.symbols.functions.get(function) {
        None => diagnostics.push(Diagnostic::new(format!("找不到函数 `{function}`"), span)),
        Some(signature) if signature.required_context != ExecutionContext::None => diagnostics
            .push(Diagnostic::new(
                format!("不能调度需要执行上下文的函数 `{function}`，调度不会保留实体或玩家"),
                span,
            )),
        Some(signature) if signature.parameters == 0 => {}
        Some(signature) => diagnostics.push(Diagnostic::new(
            format!(
                "不能调度需要 {} 个参数的函数 `{function}`",
                signature.parameters
            ),
            span,
        )),
    }
}

fn validate_assign<'a>(
    target: &str,
    operation: AssignOp,
    value: &'a Expr,
    locals: &HashSet<&'a str>,
    span: Span,
    ctx: ValidationContext<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !ctx.symbols.scores.contains(target)
        && !ctx.symbols.parameters.contains(target)
        && !locals.contains(target)
    {
        diagnostics.push(Diagnostic::new(format!("找不到计分变量 `{target}`"), span));
    }
    validate_expr(value, locals, ctx, diagnostics);
    if matches!(operation, AssignOp::Divide | AssignOp::Modulo) && constant_value(value) == Some(0)
    {
        diagnostics.push(Diagnostic::new("不能除以零", value.span));
    }
}

fn validate_let<'a>(
    name: &'a str,
    value: &'a Expr,
    locals: &mut HashSet<&'a str>,
    ctx: ValidationContext<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    validate_expr(value, locals, ctx, diagnostics);
    locals.insert(name);
}

fn validate_if<'a>(
    condition: &Condition,
    then_body: &'a [Statement],
    else_body: &'a [Statement],
    locals: &HashSet<&'a str>,
    ctx: ValidationContext<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    validate_condition(condition, locals, ctx, diagnostics);
    let mut then_locals = locals.clone();
    validate_statements(
        then_body,
        &mut then_locals,
        ctx.symbols,
        ctx.context,
        ctx.return_rules.nested(),
        diagnostics,
    );
    let mut else_locals = locals.clone();
    validate_statements(
        else_body,
        &mut else_locals,
        ctx.symbols,
        ctx.context,
        ctx.return_rules.nested(),
        diagnostics,
    );
}

fn validate_while<'a>(
    condition: &Condition,
    body: &'a [Statement],
    locals: &HashSet<&'a str>,
    ctx: ValidationContext<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    validate_condition(condition, locals, ctx, diagnostics);
    let mut body_locals = locals.clone();
    validate_statements(
        body,
        &mut body_locals,
        ctx.symbols,
        ctx.context,
        ctx.return_rules.nested(),
        diagnostics,
    );
}

fn validate_execute<'a>(
    body: &'a [Statement],
    locals: &HashSet<&'a str>,
    ctx: ValidationContext<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut body_locals = locals.clone();
    validate_statements(
        body,
        &mut body_locals,
        ctx.symbols,
        ctx.context,
        ctx.return_rules.nested(),
        diagnostics,
    );
}
