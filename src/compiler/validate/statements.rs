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
    AssignOp, Condition, Expr, GiveItem, GiveTarget, MessageTarget, SelfAction, Span, Statement,
    StatementKind,
};
use crate::compiler::constant::constant_value;
use crate::compiler::types::{ExecutionContext, ReturnRules, StatementSymbols};
use crate::diagnostic::Diagnostic;

use super::expressions::{validate_call_context, validate_condition, validate_expr};
use super::items::validate_give_count;
use super::rules::{
    valid_entity_tag, valid_resource_location, valid_sound_source, valid_text_color,
    validate_identifier,
};

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
            | StatementKind::SelfAction(_)
            | StatementKind::Message { .. }
            | StatementKind::PlaySound { .. }
            | StatementKind::Call { .. }
            | StatementKind::Schedule { .. }
            | StatementKind::Assign { .. }
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
        StatementKind::Return(value) => {
            validate_return(value.as_ref(), locals, statement.span, ctx, diagnostics);
        }
        StatementKind::Call {
            function,
            arguments,
        } => {
            validate_call(
                function,
                arguments,
                locals,
                statement.span,
                ctx,
                diagnostics,
            );
        }
        StatementKind::Schedule { function, .. } => {
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
        StatementKind::Let { name, value } => {
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
        SelfAction::SaveItems(_) => "save_items",
        SelfAction::RestoreItems(_) => "restore_items",
        SelfAction::RemovePreservingItems(_) => "remove_preserving_items",
        SelfAction::GiveItem { .. } => "give_item",
        SelfAction::ClearItems => "clear_items",
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
        | SelfAction::SaveItems(_)
        | SelfAction::RestoreItems(_)
        | SelfAction::RemovePreservingItems(_)
        | SelfAction::ClearItems => (
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
        SelfAction::SetInvulnerable(_) | SelfAction::ClearItems | SelfAction::Remove => {}
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
    value: Option<&'a Expr>,
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
    match (ctx.return_rules.returns_score, value) {
        (true, Some(value)) => validate_expr(value, locals, ctx, diagnostics),
        (true, None) => diagnostics.push(Diagnostic::new(
            "返回 score 的函数需要 `return <表达式>;`",
            span,
        )),
        (false, Some(_)) => {
            diagnostics.push(Diagnostic::new("无返回值函数只能使用 `return;`", span))
        }
        (false, None) => {}
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
