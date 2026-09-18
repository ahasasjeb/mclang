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

mod actions;
mod advancement;
mod entities;
mod execute;
mod flow;
mod queries;
mod statement;
mod tags;

pub(super) use entities::{validate_holder, validate_score_target};
pub(super) use queries::{require_player_query, validate_stopwatch_id};

use actions::*;
use advancement::*;
use entities::*;
use execute::*;
use flow::*;
use queries::*;
use statement::validate_statement;
use tags::*;

/// 遍历函数体时保持不变的校验环境。
#[derive(Clone, Copy)]
pub(in crate::compiler::validate) struct ValidationContext<'a, 'b> {
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
pub(in crate::compiler::validate) fn collect_local_declarations(
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

pub(in crate::compiler::validate) fn validate_statements<'a>(
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
