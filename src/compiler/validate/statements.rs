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
    AdvancementReference, AssignOp, CallTarget, Condition, DataSlotKind, DataSource,
    EffectDuration, EntityQueryDecl, ExecuteClauseKind, ExecuteClauses, ExecuteStoreData,
    ExecuteStoreTarget, Expr, GiveItem, GiveTarget, Holder, ItemActionKind, ItemConditionSource,
    MessageTarget, NbtComponentSource, NbtValue, NbtValueKind, PositionValue, ReturnKind,
    RotationValue, ScoreTarget, SelfAction, Span, Statement, StatementKind, TeleportDestination,
    TextComponent, XpOperation,
};
use crate::compiler::constant::constant_value;
use crate::compiler::types::{ExecutionContext, ReturnRules, StatementSymbols};
use crate::diagnostic::Diagnostic;

use super::expressions::{
    execution_context_label, validate_call_context, validate_condition, validate_expr,
};
use super::items::validate_give_count;
use super::registry::{validate_enum, validate_id};
use super::rules::{valid_entity_tag, valid_resource_location, validate_identifier};
use super::tags::reachable_functions;

/// 遍历函数体时保持不变的校验环境。
#[derive(Clone, Copy)]
pub(super) struct ValidationContext<'a, 'b> {
    pub(super) symbols: &'a StatementSymbols<'b>,
    pub(super) context: ExecutionContext,
    /// `each`/`spawn` 已知的实体类型，用于具名 NBT 标签检查；未知时为 `None`。
    pub(super) entity_type: Option<&'a str>,
    pub(super) return_rules: ReturnRules,
}

/// 收集函数体内的全部 `let` 与 `for` 循环变量，并报告重名、与全局计分变量或
/// 参数冲突。
///
/// `let` 是函数级名字：同名只允许声明一次。`for` 变量属于它所在的循环体，
/// 因此兄弟循环可以复用同一个名字，但不能与 `let`、参数、全局计分变量或
/// 外层循环变量重名。
pub(super) fn collect_local_declarations(
    statements: &[Statement],
    scores: &HashSet<&str>,
    parameters: &HashSet<&str>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut persistent = HashSet::new();
    let mut loops = Vec::new();
    collect_scoped_declarations(
        statements,
        scores,
        parameters,
        &mut persistent,
        &mut loops,
        diagnostics,
    );
}

fn collect_scoped_declarations<'a>(
    statements: &'a [Statement],
    scores: &HashSet<&str>,
    parameters: &HashSet<&str>,
    persistent: &mut HashSet<&'a str>,
    loops: &mut Vec<&'a str>,
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
                if !persistent.insert(name) || loops.contains(&name.as_str()) {
                    diagnostics.push(Diagnostic::new(
                        format!("函数内重复声明局部变量或循环变量 `{name}`"),
                        statement.span,
                    ));
                }
            }
            StatementKind::For {
                variable,
                variable_span,
                body,
                ..
            } => {
                validate_identifier("循环变量", variable, *variable_span, diagnostics);
                if scores.contains(variable.as_str()) {
                    diagnostics.push(Diagnostic::new(
                        format!("循环变量 `{variable}` 与全局计分变量重名"),
                        *variable_span,
                    ));
                }
                if parameters.contains(variable.as_str()) {
                    diagnostics.push(Diagnostic::new(
                        format!("循环变量 `{variable}` 与函数参数重名"),
                        *variable_span,
                    ));
                }
                if persistent.contains(variable.as_str()) || loops.contains(&variable.as_str()) {
                    diagnostics.push(Diagnostic::new(
                        format!("函数内重复声明循环变量或局部变量 `{variable}`"),
                        *variable_span,
                    ));
                }
                loops.push(variable);
                collect_scoped_declarations(
                    body,
                    scores,
                    parameters,
                    persistent,
                    loops,
                    diagnostics,
                );
                loops.pop();
            }
            StatementKind::If {
                then_body,
                else_body,
                ..
            } => {
                collect_scoped_declarations(
                    then_body,
                    scores,
                    parameters,
                    persistent,
                    loops,
                    diagnostics,
                );
                collect_scoped_declarations(
                    else_body,
                    scores,
                    parameters,
                    persistent,
                    loops,
                    diagnostics,
                );
            }
            StatementKind::Execute { body, .. }
            | StatementKind::Each { body, .. }
            | StatementKind::InDimension { body, .. }
            | StatementKind::Spawn { body, .. }
            | StatementKind::While { body, .. } => {
                collect_scoped_declarations(
                    body,
                    scores,
                    parameters,
                    persistent,
                    loops,
                    diagnostics,
                );
            }
            _ => {}
        }
    }
}

pub(super) fn validate_statements<'a>(
    statements: &'a [Statement],
    locals: &mut HashSet<&'a str>,
    symbols: &StatementSymbols<'_>,
    context: ExecutionContext,
    entity_type: Option<&'a str>,
    return_rules: ReturnRules,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let ctx = ValidationContext {
        symbols,
        context,
        entity_type,
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
        StatementKind::Run(command) => {
            validate_raw_command(command, statement.span, ctx, diagnostics);
        }
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
        StatementKind::Message { target, component } => {
            validate_message(target, component, statement.span, ctx, diagnostics);
        }
        StatementKind::PlaySound {
            sound,
            source,
            targets,
            position,
            volume,
            pitch,
            min_volume,
        } => {
            validate_play_sound(
                sound,
                source,
                targets.as_deref(),
                position.as_ref(),
                volume.as_deref(),
                pitch.as_deref(),
                min_volume.as_deref(),
                statement.span,
                ctx,
                diagnostics,
            );
        }
        StatementKind::Each { query, body } => {
            validate_each(query, body, locals, statement.span, ctx, diagnostics);
        }
        StatementKind::InDimension { dimension, body } => {
            validate_in_dimension(dimension, body, locals, statement.span, ctx, diagnostics);
        }
        StatementKind::Spawn {
            entity_type,
            position,
            body,
        } => {
            validate_spawn(
                entity_type,
                position.as_ref(),
                body,
                locals,
                statement.span,
                ctx,
                diagnostics,
            );
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
        StatementKind::ScoreboardEnable { target } => {
            validate_score_target(target, statement.span, ctx, diagnostics);
            if let Some(declaration) = ctx
                .symbols
                .objective_declarations
                .get(target.objective.as_str())
                && declaration.criteria.as_deref() != Some("trigger")
            {
                diagnostics.push(Diagnostic::new(
                    format!(
                        "scoreboard.enable 只对 criteria = \"trigger\" 的目标有意义，`{}` 的准则是 `{}`",
                        target.objective,
                        declaration.criteria.as_deref().unwrap_or("dummy")
                    ),
                    statement.span,
                ));
            }
        }
        StatementKind::ScoreboardOperation {
            result,
            operation: _,
            source,
        } => {
            validate_score_target(result, statement.span, ctx, diagnostics);
            validate_score_target(source, statement.span, ctx, diagnostics);
            for target in [result, source] {
                if matches!(target.holder, Holder::Origin) {
                    diagnostics.push(Diagnostic::new(
                        "scoreboard.operation 的持有者不能是投掷者；请用 self/自身 或实体查询",
                        statement.span,
                    ));
                }
            }
        }
        StatementKind::ScoreboardDisplay {
            slot,
            slot_span,
            objective,
        } => {
            validate_enum("display_slot", "显示槽", slot, *slot_span, diagnostics);
            if let Some((name, name_span)) = objective
                && !ctx.symbols.objectives.contains(name.as_str())
            {
                diagnostics.push(Diagnostic::new(
                    format!("找不到计分板目标 `{name}`；先声明 `objective {name};`"),
                    *name_span,
                ));
            }
        }
        StatementKind::Teleport {
            targets,
            destination,
            rotation,
        } => {
            validate_teleport(
                targets,
                destination,
                rotation.as_ref(),
                statement.span,
                ctx,
                diagnostics,
            );
        }
        StatementKind::DataMerge { target, nbt } => {
            super::components::validate_nbt_source(target, statement.span, ctx, diagnostics);
            if !matches!(nbt.kind, NbtValueKind::Compound(_)) {
                diagnostics.push(Diagnostic::new(
                    "data.merge 需要复合标签 `nbt { ... }`（中文 `数据 { ... }`）",
                    statement.span,
                ));
            }
        }
        StatementKind::DataRemove {
            target,
            path,
            path_span,
        } => {
            super::components::validate_nbt_source(target, statement.span, ctx, diagnostics);
            if !super::components::valid_nbt_component_path(path) {
                diagnostics.push(Diagnostic::new(
                    format!("`{path}` 不是有效的 NBT 路径"),
                    *path_span,
                ));
            }
        }
        StatementKind::DataModify {
            target,
            path,
            path_span,
            operation,
        } => {
            super::components::validate_nbt_source(target, statement.span, ctx, diagnostics);
            if !super::components::valid_nbt_component_path(path) {
                diagnostics.push(Diagnostic::new(
                    format!("`{path}` 不是有效的 NBT 路径"),
                    *path_span,
                ));
            }
            super::expressions::validate_data_source(
                &operation.source,
                statement.span,
                ctx,
                diagnostics,
            );
            if let DataSource::String {
                start: Some(start),
                end: Some(end),
                ..
            } = &operation.source
                && start > end
            {
                diagnostics.push(Diagnostic::new(
                    format!("string 来源的起始下标 {start} 不能大于结束下标 {end}"),
                    statement.span,
                ));
            }
        }
        StatementKind::ItemAction {
            method: _,
            target,
            slots,
            slots_span,
            action,
        } => {
            super::expressions::validate_item_condition_source(
                target,
                statement.span,
                ctx,
                diagnostics,
            );
            super::expressions::validate_slot_source(slots, *slots_span, diagnostics);
            if let ItemConditionSource::Entity(Holder::Query(name, name_span)) = target
                && let Some(query) = ctx.symbols.queries.get(name.as_str())
                && query.limit != Some(1)
            {
                diagnostics.push(Diagnostic::new(
                    format!("item 目标需要 limit(1) 的单个实体查询 `{name}`"),
                    *name_span,
                ));
            }
            match action {
                ItemActionKind::With(item, span) => {
                    if !ctx.symbols.item_stacks.contains_key(item.as_str()) {
                        diagnostics.push(Diagnostic::new(
                            format!(
                                "找不到物品定义 `{item}`；先声明 `item {item} = item_stack(...);`"
                            ),
                            *span,
                        ));
                    }
                }
                ItemActionKind::From {
                    source,
                    slots,
                    slots_span,
                    modifier,
                } => {
                    super::expressions::validate_item_condition_source(
                        source,
                        statement.span,
                        ctx,
                        diagnostics,
                    );
                    super::expressions::validate_slot_source(slots, *slots_span, diagnostics);
                    if let Some(modifier) = modifier
                        && !super::rules::valid_resource_location(modifier)
                    {
                        diagnostics.push(Diagnostic::new(
                            format!("`{modifier}` 不是有效的物品修饰器资源位置"),
                            statement.span,
                        ));
                    }
                    if let ItemConditionSource::Entity(Holder::Query(name, name_span)) = source
                        && let Some(query) = ctx.symbols.queries.get(name.as_str())
                        && query.limit != Some(1)
                    {
                        diagnostics.push(Diagnostic::new(
                            format!("item 来源需要 limit(1) 的单个实体查询 `{name}`"),
                            *name_span,
                        ));
                    }
                }
                ItemActionKind::Modifier(modifier, span) => {
                    if !super::rules::valid_resource_location(modifier) {
                        diagnostics.push(Diagnostic::new(
                            format!("`{modifier}` 不是有效的物品修饰器资源位置"),
                            *span,
                        ));
                    }
                }
            }
        }
        StatementKind::NbtMerge { nbt } => {
            validate_nbt_merge(nbt, statement.span, ctx, diagnostics);
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
        StatementKind::For {
            variable,
            start,
            end,
            body,
            ..
        } => {
            validate_for(variable, start, end, body, locals, ctx, diagnostics);
        }
        StatementKind::Break | StatementKind::Continue => {
            if ctx.return_rules.loop_depth == 0 {
                let keyword = if matches!(&statement.kind, StatementKind::Break) {
                    "break"
                } else {
                    "continue"
                };
                diagnostics.push(Diagnostic::new(
                    format!(
                        "`{keyword}` 只能出现在 for/while 循环体内；each/spawn 的每个实体会单独执行，\
                         不能用它跳出"
                    ),
                    statement.span,
                ));
            }
        }
        StatementKind::Execute { clauses, body } => {
            validate_execute(clauses, body, statement.span, locals, ctx, diagnostics);
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
pub(super) fn validate_holder(
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
    rotation: Option<&RotationValue>,
    span: Span,
    ctx: ValidationContext<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    validate_holder(targets, span, ctx, diagnostics);
    if let Some(rotation) = rotation
        && matches!(destination, TeleportDestination::Entity { .. })
    {
        diagnostics.push(Diagnostic::new(
            "teleport 跟随实体时不能同时指定朝向：实体的朝向会一并跟随",
            rotation.span,
        ));
    }
    match destination {
        TeleportDestination::Position(position) => {
            super::world::validate_position_value(position, diagnostics);
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

/// `nbt { ... }`：把具名 NBT 合并到当前实体，键对照 26.3 源码标签表检查。
fn validate_nbt_merge(
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
    super::entity_nbt::validate_entity_nbt(nbt, ctx.entity_type, span, diagnostics);
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

fn validate_effect_clear(
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
    if let Some(item) = item {
        validate_id("item", "物品", item, span, diagnostics);
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
    super::components::validate_component(component, ctx, diagnostics);
}

#[allow(clippy::too_many_arguments)]
fn validate_play_sound(
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
        super::world::validate_position_value(position, diagnostics);
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
        query_decl.map(|query| query.entity_type.as_str()),
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

fn validate_spawn<'a>(
    entity_type: &str,
    position: Option<&PositionValue>,
    body: &'a [Statement],
    locals: &HashSet<&'a str>,
    span: Span,
    ctx: ValidationContext<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    validate_id("entity_type", "实体类型", entity_type, span, diagnostics);
    if super::rules::valid_resource_location(entity_type) && non_summonable_entity(entity_type) {
        diagnostics.push(Diagnostic::new(
            format!("Minecraft 的 /summon 不支持实体类型 `{entity_type}`"),
            span,
        ));
    }
    if let Some(position) = position {
        super::world::validate_position_value(position, diagnostics);
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
        (_, ReturnKind::Run(command)) => validate_raw_command(command, span, ctx, diagnostics),
        (true, ReturnKind::Fail) => {}
        (true, ReturnKind::Void) => diagnostics.push(Diagnostic::new(
            "返回 score 的函数需要 `return <表达式>;`、`return run \"命令\";` 或 `return fail;`",
            span,
        )),
        (false, ReturnKind::Value(_)) => diagnostics.push(Diagnostic::new(
            "无返回值函数只能使用 `return;`、`return fail;` 或 `return run \"命令\";`",
            span,
        )),
        (false, ReturnKind::Void | ReturnKind::Fail) => {}
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
        diagnostics.push(Diagnostic::new(format!("找不到函数 `{function}`；如果它来自其他模块，请确认对方声明了 `export`，并在本模块 `import` 它"), span));
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
        None => diagnostics.push(Diagnostic::new(format!("找不到函数 `{function}`；如果它来自其他模块，请确认对方声明了 `export`，并在本模块 `import` 它"), span)),
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
        ctx.entity_type,
        ctx.return_rules.nested(),
        diagnostics,
    );
    let mut else_locals = locals.clone();
    validate_statements(
        else_body,
        &mut else_locals,
        ctx.symbols,
        ctx.context,
        ctx.entity_type,
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
        ctx.entity_type,
        ctx.return_rules.in_loop(),
        diagnostics,
    );
}

fn validate_for<'a>(
    variable: &'a str,
    start: &'a Expr,
    end: &'a Expr,
    body: &'a [Statement],
    locals: &HashSet<&'a str>,
    ctx: ValidationContext<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    validate_expr(start, locals, ctx, diagnostics);
    validate_expr(end, locals, ctx, diagnostics);
    let mut body_locals = locals.clone();
    body_locals.insert(variable);
    validate_statements(
        body,
        &mut body_locals,
        ctx.symbols,
        ctx.context,
        ctx.entity_type,
        ctx.return_rules.in_loop(),
        diagnostics,
    );
}

fn validate_execute<'a>(
    clauses: &'a ExecuteClauses,
    body: &'a [Statement],
    span: Span,
    locals: &HashSet<&'a str>,
    ctx: ValidationContext<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let ExecuteClauses::Structured(clauses) = clauses else {
        // 字符串子句原样转发，权限等底层检查已由 run 语句族覆盖。
        validate_execute_body(body, locals, ctx, ctx.context, ctx.entity_type, diagnostics);
        return;
    };

    let mut context = ctx.context;
    let mut entity_type = ctx.entity_type;
    let mut seen: HashSet<&'static str> = HashSet::new();
    // 0 = 修饰符，1 = 条件，2 = store；子句只能按这个顺序推进。
    let mut stage = 0_u8;
    let mut has_stores = false;
    for clause in clauses {
        let clause_ctx = ValidationContext {
            context,
            entity_type,
            ..ctx
        };
        match &clause.kind {
            ExecuteClauseKind::As { query, query_span } => {
                if !mark_execute_modifier(&mut seen, "as", clause.span, stage, diagnostics) {
                    continue;
                }
                match query_reference(query, *query_span, clause_ctx, diagnostics) {
                    Some(declaration) => {
                        entity_type = Some(declaration.entity_type.as_str());
                        context = if declaration.entity_type == "minecraft:player" {
                            ExecutionContext::Player
                        } else {
                            ExecutionContext::Mob
                        };
                    }
                    None => {
                        entity_type = None;
                        context = ExecutionContext::Entity;
                    }
                }
            }
            ExecuteClauseKind::At { query, query_span } => {
                if !mark_execute_modifier(&mut seen, "at", clause.span, stage, diagnostics) {
                    continue;
                }
                validate_execute_query_selector(
                    query,
                    *query_span,
                    clause.span,
                    clause_ctx,
                    diagnostics,
                );
            }
            ExecuteClauseKind::Positioned(position) => {
                if mark_execute_modifier(&mut seen, "positioned", clause.span, stage, diagnostics) {
                    super::world::validate_position_value(position, diagnostics);
                }
            }
            ExecuteClauseKind::Rotated(_) => {
                mark_execute_modifier(&mut seen, "rotated", clause.span, stage, diagnostics);
            }
            ExecuteClauseKind::FacingPosition(position) => {
                if mark_execute_modifier(&mut seen, "facing", clause.span, stage, diagnostics) {
                    super::world::validate_position_value(position, diagnostics);
                }
            }
            ExecuteClauseKind::FacingEntity {
                query, query_span, ..
            } => {
                if mark_execute_modifier(&mut seen, "facing", clause.span, stage, diagnostics) {
                    validate_execute_query_selector(
                        query,
                        *query_span,
                        clause.span,
                        clause_ctx,
                        diagnostics,
                    );
                }
            }
            ExecuteClauseKind::Align { .. } => {
                mark_execute_modifier(&mut seen, "align", clause.span, stage, diagnostics);
            }
            ExecuteClauseKind::Anchored(_) => {
                mark_execute_modifier(&mut seen, "anchored", clause.span, stage, diagnostics);
            }
            ExecuteClauseKind::In {
                dimension,
                dimension_span,
            } => {
                if mark_execute_modifier(&mut seen, "in", clause.span, stage, diagnostics) {
                    validate_id("dimension", "维度", dimension, *dimension_span, diagnostics);
                }
            }
            ExecuteClauseKind::On(_) => {
                if mark_execute_modifier(&mut seen, "on", clause.span, stage, diagnostics)
                    && !context.is_entity()
                {
                    diagnostics.push(Diagnostic::new(
                        "execute on 需要当前存在执行实体：on 会读取 @s 的实体关系，请先写 as(查询) 或放进实体上下文",
                        clause.span,
                    ));
                }
                // 关系实体可能是玩家，也可能是非玩家，按最宽的实体上下文继续。
                context = ExecutionContext::Entity;
                entity_type = None;
            }
            ExecuteClauseKind::Summon {
                entity_type: summoned,
                entity_type_span,
            } => {
                if !mark_execute_modifier(&mut seen, "summon", clause.span, stage, diagnostics) {
                    continue;
                }
                validate_id(
                    "entity_type",
                    "实体类型",
                    summoned,
                    *entity_type_span,
                    diagnostics,
                );
                if valid_resource_location(summoned) && non_summonable_entity(summoned) {
                    diagnostics.push(Diagnostic::new(
                        format!("Minecraft 的 /summon 不支持实体类型 `{summoned}`"),
                        clause.span,
                    ));
                }
                context = ExecutionContext::Mob;
                entity_type = Some(summoned.as_str());
            }
            ExecuteClauseKind::If(condition) | ExecuteClauseKind::Unless(condition) => {
                if stage > 1 {
                    diagnostics.push(Diagnostic::new(
                        "execute 的条件必须写在 store 之前",
                        clause.span,
                    ));
                }
                stage = stage.max(1);
                validate_condition(condition, locals, clause_ctx, diagnostics);
            }
            ExecuteClauseKind::StoreResult(target) | ExecuteClauseKind::StoreSuccess(target) => {
                stage = 2;
                has_stores = true;
                validate_store_target(target, clause.span, clause_ctx, diagnostics);
            }
            ExecuteClauseKind::StoreData(data) => {
                stage = 2;
                has_stores = true;
                validate_store_data(data, clause.span, clause_ctx, diagnostics);
            }
        }
    }

    if has_stores && let Some(last) = body.last() {
        match &last.kind {
            StatementKind::If { .. }
            | StatementKind::While { .. }
            | StatementKind::Each { .. }
            | StatementKind::InDimension { .. }
            | StatementKind::Spawn { .. }
            | StatementKind::Execute { .. } => diagnostics.push(Diagnostic::new(
                "execute store 捕获块内最后一条命令的结果；if/while/each/spawn/in_dimension/execute \
                 作为最后一条语句时结果不会传递，请把要捕获的命令放到块末尾",
                last.span,
            )),
            StatementKind::Call {
                target: CallTarget::Function(name),
                ..
            } => {
                if let Some(signature) = ctx.symbols.functions.get(name.as_str())
                    && !signature.returns_score
                {
                    diagnostics.push(Diagnostic::new(
                        format!(
                            "execute store 捕获的是返回值，而函数 `{name}` 没有声明返回 score；\
                             请调用有返回值的函数，或把要捕获的命令放到块末尾"
                        ),
                        last.span,
                    ));
                }
            }
            _ => {}
        }
    }
    if has_stores && body.is_empty() {
        diagnostics.push(Diagnostic::new(
            "execute store 需要块内至少一条命令来产生结果",
            span,
        ));
    }

    validate_execute_body(body, locals, ctx, context, entity_type, diagnostics);
}

/// 结构化 execute 的块体校验：上下文由子句推导，`return` 仍然被拒绝。
fn validate_execute_body<'a>(
    body: &'a [Statement],
    locals: &HashSet<&'a str>,
    ctx: ValidationContext<'_, '_>,
    context: ExecutionContext,
    entity_type: Option<&'a str>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut body_locals = locals.clone();
    validate_statements(
        body,
        &mut body_locals,
        ctx.symbols,
        context,
        entity_type,
        ctx.return_rules.nested(),
        diagnostics,
    );
}

/// `at`/`facing entity` 只转发实体选择器，无法应用查询自带的物品过滤。
fn validate_execute_query_selector(
    query: &str,
    query_span: Span,
    clause_span: Span,
    ctx: ValidationContext<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let Some(declaration) = query_reference(query, query_span, ctx, diagnostics)
        && declaration.item.is_some()
    {
        diagnostics.push(Diagnostic::new(
            format!(
                "查询 `{query}` 带物品过滤，但 at/facing entity 只转发实体选择器，无法追加物品条件；\
                 请改用 as 子句或去掉过滤"
            ),
            clause_span,
        ));
    }
}

/// 记录一次修饰符出现：重复或出现在条件之后时报错并返回假。
fn mark_execute_modifier(
    seen: &mut HashSet<&'static str>,
    name: &'static str,
    span: Span,
    stage: u8,
    diagnostics: &mut Vec<Diagnostic>,
) -> bool {
    if stage > 0 {
        diagnostics.push(Diagnostic::new(
            "execute 的修饰符必须写在 if/unless 条件之前",
            span,
        ));
        return false;
    }
    if !seen.insert(name) {
        diagnostics.push(Diagnostic::new(
            format!("execute 的 {name} 子句只能出现一次"),
            span,
        ));
        return false;
    }
    true
}

/// `store.result/success` 的目标：用户计分板或 Boss 栏。
fn validate_store_target(
    target: &ExecuteStoreTarget,
    span: Span,
    ctx: ValidationContext<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match target {
        // origin 持有者的写入由代码生成拆成「临时项捕获 + on origin 复制」，
        // 这里只校验持有者与目标本身。
        ExecuteStoreTarget::Score(score) => validate_score_target(score, span, ctx, diagnostics),
        ExecuteStoreTarget::BossBar { id, id_span, .. } => {
            if !valid_resource_location(id) {
                diagnostics.push(Diagnostic::new(
                    format!("`{id}` 不是有效的 Boss 栏资源位置"),
                    *id_span,
                ));
            }
        }
    }
}

/// `store.data` 的 NBT 目标：路径、来源与实体写保护。
fn validate_store_data(
    data: &ExecuteStoreData,
    span: Span,
    ctx: ValidationContext<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !super::components::valid_nbt_component_path(&data.path) {
        diagnostics.push(Diagnostic::new(
            format!("`{}` 不是有效的 NBT 路径", data.path),
            data.path_span,
        ));
    }
    match &data.source {
        NbtComponentSource::Entity(holder) => {
            match holder {
                Holder::SelfEntity => {
                    if ctx.context != ExecutionContext::Mob {
                        diagnostics.push(Diagnostic::new(
                            "store.data 通过 data 命令写入实体 NBT，Minecraft 不允许修改玩家数据；只能在确定不是玩家的实体上下文中使用（非玩家查询的 each、非玩家 spawn，或 @non_player 函数）",
                            span,
                        ));
                        return;
                    }
                }
                Holder::Query(name, query_span) => {
                    if let Some(query) = ctx.symbols.queries.get(name.as_str()) {
                        if query.limit != Some(1) {
                            diagnostics.push(Diagnostic::new(
                                format!(
                                    "store.data 的实体来源需要 limit(1) 的单个实体查询 `{name}`"
                                ),
                                *query_span,
                            ));
                        } else if query.entity_type == "minecraft:player" {
                            diagnostics.push(Diagnostic::new(
                                format!(
                                    "store.data 不能写入玩家：查询 `{name}` 匹配 minecraft:player"
                                ),
                                *query_span,
                            ));
                            return;
                        }
                    }
                }
                // 投掷者的类型无法静态确认：运行期是玩家时 Minecraft 静默拒绝写入，
                // 与非玩家的成功路径共用同一份代码，不在编译期强制拒绝。
                Holder::Origin => {}
            }
            validate_holder(holder, span, ctx, diagnostics);
        }
        NbtComponentSource::Block(position) => {
            super::world::validate_block_position(position, diagnostics);
        }
        NbtComponentSource::Storage(storage, storage_span) => {
            if !valid_resource_location(storage) {
                diagnostics.push(Diagnostic::new(
                    format!("`{storage}` 不是有效的存储资源位置"),
                    *storage_span,
                ));
            }
        }
    }
}

/// 函数权限模型（1.6）：`run` 字符串里的根命令不得越过
/// `function-permission-level`。未知根命令留给后续的命令树校验（8.4）。
fn validate_raw_command(
    command: &str,
    span: Span,
    ctx: ValidationContext<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(first) = command.split_whitespace().next() else {
        return;
    };
    let name = first.strip_prefix('/').unwrap_or(first);
    let snapshot = crate::version::snapshot::snapshot();
    let Some(level) = snapshot.root_command_level(name) else {
        return;
    };
    if level > ctx.symbols.function_permission_level {
        diagnostics.push(Diagnostic::new(
            format!(
                "`{name}` 需要权限等级 {level}（{}），超过数据包函数上限 {}（{}）；可用 `--function-permission-level` 提高",
                permission_label(level),
                ctx.symbols.function_permission_level,
                permission_label(ctx.symbols.function_permission_level),
            ),
            span,
        ));
    }
}

/// 权限等级的中文标签。
fn permission_label(level: u8) -> &'static str {
    match level {
        0 => "所有人",
        1 => "MODERATOR",
        2 => "GAMEMASTER",
        3 => "ADMIN",
        4 => "OWNER",
        _ => "未知",
    }
}
