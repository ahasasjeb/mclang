//! 表达式与布尔条件的校验。
//!
//! 这里只负责名称解析、函数调用形状和静态除零；语句结构留在
//! [`super::statements`]，执行上下文通过调用方的 [`ValidationContext`] 传入。

use std::collections::HashSet;

use crate::ast::{BinaryOp, Condition, Expr, ExprKind, ScoreHolder, Span};
use crate::compiler::constant::constant_value;
use crate::compiler::types::{ExecutionContext, Signature};
use crate::diagnostic::Diagnostic;

use super::statements::ValidationContext;

pub(super) fn validate_call_context(
    function: &str,
    signature: Signature,
    span: Span,
    ctx: ValidationContext<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if ctx.context.satisfies(signature.required_context) {
        return;
    }
    let Some((attribute, kind)) = execution_context_label(signature.required_context) else {
        return;
    };
    diagnostics.push(Diagnostic::new(
        format!("{attribute} 函数 `{function}` 需要{kind}执行上下文"),
        span,
    ));
}

/// 执行上下文对应的函数属性与中文描述，供调用点组织诊断文本。
pub(super) fn execution_context_label(
    context: ExecutionContext,
) -> Option<(&'static str, &'static str)> {
    match context {
        ExecutionContext::Player => Some(("@player", "玩家")),
        ExecutionContext::Mob => Some(("@non_player", "非玩家实体")),
        ExecutionContext::Entity => Some(("@entity", "实体")),
        ExecutionContext::None => None,
    }
}

pub(super) fn validate_condition(
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

pub(super) fn validate_expr(
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
        ExprKind::XpQuery { target, .. } => {
            if let Some(query) =
                super::statements::require_player_query(target, expression.span, ctx, diagnostics)
                && query.limit != Some(1)
            {
                diagnostics.push(Diagnostic::new(
                    format!("xp.query 需要 limit(1) 的单个玩家查询 `{target}`"),
                    expression.span,
                ));
            }
        }
        ExprKind::ScoreQuery { target } => {
            super::statements::validate_score_target(target, expression.span, ctx, diagnostics);
            if let ScoreHolder::Query(name, span) = &target.holder
                && let Some(query) = ctx.symbols.queries.get(name.as_str())
                && query.limit != Some(1)
            {
                diagnostics.push(Diagnostic::new(
                    format!("scoreboard.get 需要 limit(1) 的单个实体查询 `{name}`"),
                    *span,
                ));
            }
        }
        ExprKind::StopwatchQuery { id, .. } => {
            super::statements::validate_stopwatch_id(id, expression.span, diagnostics);
        }
        ExprKind::TimeQuery { clock } => {
            if let Some(clock) = clock
                && !super::rules::valid_resource_location(clock)
            {
                diagnostics.push(Diagnostic::new(
                    format!("`{clock}` 不是有效的世界时钟资源位置"),
                    expression.span,
                ));
            }
        }
        ExprKind::GameTimeQuery | ExprKind::WorldBorderSize => {}
        ExprKind::GameRuleQuery { name } => {
            if !super::world::game_rule_exists(name) {
                diagnostics.push(Diagnostic::new(
                    format!("未知游戏规则 `{name}`；规则名来自 26.3 的 GameRules 注册表"),
                    expression.span,
                ));
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
