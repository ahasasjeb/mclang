//! 函数体内的语句、表达式和条件校验。
//!
//! 校验器遍历树时携带三层信息：当前可见的局部变量、符号表
//! （[`StatementSymbols`]）和当前执行上下文/返回规则。后两者放进
//! [`ValidationContext`]，避免每个辅助函数都接收一长串参数。每个语句种类对应
//! 一个独立的小函数，`validate_statement` 只负责分派。

use std::collections::HashSet;

use crate::ast::{
    AssignOp, BinaryOp, Condition, Expr, ExprKind, MessageTarget, SelfAction, Span, Statement,
    StatementKind,
};
use crate::compiler::constant::constant_value;
use crate::compiler::types::{ExecutionContext, ReturnRules, Signature, StatementSymbols};
use crate::diagnostic::Diagnostic;

use super::items::validate_give_count;
use super::rules::{
    valid_entity_tag, valid_resource_location, valid_sound_source, valid_text_color,
    validate_identifier,
};

/// 遍历函数体时保持不变的校验环境。
#[derive(Clone, Copy)]
struct ValidationContext<'a, 'b> {
    symbols: &'a StatementSymbols<'b>,
    context: ExecutionContext,
    return_rules: ReturnRules,
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
    target: &str,
    item: &str,
    count: Option<u32>,
    count_span: Option<Span>,
    span: Span,
    ctx: ValidationContext<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match ctx.symbols.queries.get(target) {
        None => diagnostics.push(Diagnostic::new(format!("找不到实体查询 `{target}`"), span)),
        Some(query) if query.entity_type != "minecraft:player" => {
            diagnostics.push(Diagnostic::new(
                format!("give 目标查询 `{target}` 必须匹配 minecraft:player"),
                span,
            ));
        }
        Some(_) => {}
    }
    match ctx.symbols.item_stacks.get(item) {
        None => diagnostics.push(Diagnostic::new(format!("找不到物品定义 `{item}`"), span)),
        Some(declaration) => {
            validate_give_count(declaration, count, span, count_span, diagnostics);
        }
    }
}

fn validate_self_action(
    action: &SelfAction,
    span: Span,
    ctx: ValidationContext<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let required_context = if matches!(action, SelfAction::GiveItem { .. }) {
        ExecutionContext::Player
    } else {
        ExecutionContext::Entity
    };
    if ctx.context < required_context {
        let message = if required_context == ExecutionContext::Player {
            "self.give_item 需要玩家执行上下文；请放入玩家 query 的 each 块，或给函数添加 @player"
        } else {
            "self 方法需要实体执行上下文；请放入 each/spawn 块，或给函数添加 @entity"
        };
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
        SelfAction::SetInvulnerable(_)
        | SelfAction::ClearItems
        | SelfAction::Remove
        | SelfAction::Consume
        | SelfAction::ReturnToOwner => {}
    }
}

fn validate_message(
    target: &MessageTarget,
    color: Option<&str>,
    span: Span,
    ctx: ValidationContext<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if matches!(target, MessageTarget::SelfEntity) && ctx.context < ExecutionContext::Player {
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
    if ctx.context < ExecutionContext::Player {
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
    let body_context = query_decl.map_or(ExecutionContext::Entity, |query| {
        if query.entity_type == "minecraft:player" {
            ExecutionContext::Player
        } else {
            ExecutionContext::Entity
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
    }
    let mut body_locals = locals.clone();
    validate_statements(
        body,
        &mut body_locals,
        ctx.symbols,
        ExecutionContext::Entity,
        ctx.return_rules.nested(),
        diagnostics,
    );
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

fn validate_call_context(
    function: &str,
    signature: Signature,
    span: Span,
    ctx: ValidationContext<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if ctx.context >= signature.required_context {
        return;
    }
    let (attribute, kind) = match signature.required_context {
        ExecutionContext::Player => ("@player", "玩家"),
        ExecutionContext::Entity => ("@entity", "实体"),
        ExecutionContext::None => return,
    };
    diagnostics.push(Diagnostic::new(
        format!("{attribute} 函数 `{function}` 需要{kind}执行上下文"),
        span,
    ));
}

fn validate_condition(
    condition: &Condition,
    locals: &HashSet<&str>,
    ctx: ValidationContext<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match condition {
        Condition::Predicate { name, span } => {
            if !ctx.symbols.predicates.contains(name.as_str()) {
                diagnostics.push(Diagnostic::new(
                    format!("找不到 predicate 资源 `{name}`"),
                    *span,
                ));
            }
        }
        Condition::Compare { left, right, .. } => {
            validate_expr(left, locals, ctx, diagnostics);
            validate_expr(right, locals, ctx, diagnostics);
        }
        Condition::Not(condition) => validate_condition(condition, locals, ctx, diagnostics),
        Condition::And(left, right) | Condition::Or(left, right) => {
            validate_condition(left, locals, ctx, diagnostics);
            validate_condition(right, locals, ctx, diagnostics);
        }
    }
}

fn validate_expr(
    expression: &Expr,
    locals: &HashSet<&str>,
    ctx: ValidationContext<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match &expression.kind {
        ExprKind::Integer(_) => {}
        ExprKind::Score(name) => {
            if !ctx.symbols.scores.contains(name.as_str())
                && !ctx.symbols.parameters.contains(name.as_str())
                && !locals.contains(name.as_str())
            {
                diagnostics.push(Diagnostic::new(
                    format!("找不到计分变量 `{name}`"),
                    expression.span,
                ));
            }
        }
        ExprKind::Call {
            function,
            arguments,
        } => {
            match ctx.symbols.functions.get(function.as_str()) {
                None => diagnostics.push(Diagnostic::new(
                    format!("找不到函数 `{function}`"),
                    expression.span,
                )),
                Some(signature) => {
                    if !signature.returns_score {
                        diagnostics.push(Diagnostic::new(
                            format!("无返回值函数 `{function}` 不能用于表达式"),
                            expression.span,
                        ));
                    }
                    validate_call_context(function, *signature, expression.span, ctx, diagnostics);
                    if arguments.len() != signature.parameters {
                        diagnostics.push(Diagnostic::new(
                            format!(
                                "函数 `{function}` 需要 {} 个参数，实际提供 {} 个",
                                signature.parameters,
                                arguments.len()
                            ),
                            expression.span,
                        ));
                    }
                }
            }
            for argument in arguments {
                validate_expr(argument, locals, ctx, diagnostics);
            }
        }
        ExprKind::Negate(value) => validate_expr(value, locals, ctx, diagnostics),
        ExprKind::Binary {
            left,
            operation,
            right,
        } => {
            validate_expr(left, locals, ctx, diagnostics);
            validate_expr(right, locals, ctx, diagnostics);
            if matches!(operation, BinaryOp::Divide | BinaryOp::Modulo)
                && constant_value(right) == Some(0)
            {
                diagnostics.push(Diagnostic::new("不能除以零", right.span));
            }
        }
    }
}
