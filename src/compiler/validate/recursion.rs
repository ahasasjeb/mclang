//! 同步调用图分析：拒绝任何直接或间接的同步递归。
//!
//! `schedule` 在未来的游戏刻建立新的调用，因此不属于同步调用图。函数标签调用
//! 会先展开为可达函数集合，保证 `fn a() { call #t(); }` 这类经标签形成的环也
//! 会被检测到。

use std::collections::{HashMap, HashSet};

use crate::ast::{CallTarget, Condition, Expr, ExprKind, Program, Statement, StatementKind};
use crate::diagnostic::Diagnostic;

use super::graph::cyclic_nodes;

pub(super) fn validate_synchronous_recursion<'a>(
    program: &'a Program,
    reachable_tag_functions: &HashMap<&'a str, Vec<&'a str>>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let known_functions = program
        .functions
        .iter()
        .map(|function| function.name.as_str())
        .collect::<HashSet<_>>();
    let mut graph = HashMap::<&str, HashSet<&str>>::new();
    for function in &program.functions {
        let mut calls = HashSet::new();
        collect_synchronous_calls(&function.body, reachable_tag_functions, &mut calls);
        calls.retain(|callee| known_functions.contains(callee));
        graph.insert(&function.name, calls);
    }

    let cyclic = cyclic_nodes(known_functions.iter().copied(), &graph);
    for function in &program.functions {
        if cyclic.contains(function.name.as_str()) {
            diagnostics.push(Diagnostic::new(
                format!(
                    "函数 `{}` 位于同步递归调用环中；请改用 schedule 推迟下一次调用",
                    function.name
                ),
                function.span,
            ));
        }
    }
}

fn collect_synchronous_calls<'a>(
    statements: &'a [Statement],
    reachable_tag_functions: &HashMap<&'a str, Vec<&'a str>>,
    calls: &mut HashSet<&'a str>,
) {
    for statement in statements {
        match &statement.kind {
            StatementKind::MacroCall {
                target: CallTarget::Function(name),
                ..
            } => {
                calls.insert(name);
            }
            StatementKind::MacroCall {
                target: CallTarget::Tag(name),
                ..
            } => {
                calls.extend(
                    reachable_tag_functions
                        .get(name.as_str())
                        .into_iter()
                        .flatten()
                        .copied(),
                );
            }
            StatementKind::MacroCall { .. } => {}
            StatementKind::CoreCommand(_) | StatementKind::EntityCommand(_) => {}
            StatementKind::Call { target, arguments } => {
                match target {
                    CallTarget::External(_) => {}
                    CallTarget::Function(function) => {
                        calls.insert(function);
                    }
                    CallTarget::Tag(tag) => {
                        calls.extend(
                            reachable_tag_functions
                                .get(tag.as_str())
                                .into_iter()
                                .flatten()
                                .copied(),
                        );
                    }
                }
                for argument in arguments {
                    collect_expr_calls(argument, calls);
                }
            }
            StatementKind::If {
                condition,
                then_body,
                else_body,
                ..
            } => {
                collect_condition_calls(condition, calls);
                collect_synchronous_calls(then_body, reachable_tag_functions, calls);
                collect_synchronous_calls(else_body, reachable_tag_functions, calls);
            }
            StatementKind::While { condition, body } => {
                collect_condition_calls(condition, calls);
                collect_synchronous_calls(body, reachable_tag_functions, calls);
            }
            StatementKind::For {
                start, end, body, ..
            } => {
                collect_expr_calls(start, calls);
                collect_expr_calls(end, calls);
                collect_synchronous_calls(body, reachable_tag_functions, calls);
            }
            StatementKind::Execute { clauses, body } => {
                if let crate::ast::ExecuteClauses::Structured(clauses) = clauses {
                    for clause in clauses {
                        if let crate::ast::ExecuteClauseKind::If(condition)
                        | crate::ast::ExecuteClauseKind::Unless(condition) = &clause.kind
                        {
                            collect_condition_calls(condition, calls);
                        }
                    }
                }
                collect_synchronous_calls(body, reachable_tag_functions, calls);
            }
            StatementKind::Each { body, .. }
            | StatementKind::InDimension { body, .. }
            | StatementKind::Spawn { body, .. } => {
                collect_synchronous_calls(body, reachable_tag_functions, calls)
            }
            StatementKind::Assign { value, .. } | StatementKind::Let { value, .. } => {
                collect_expr_calls(value, calls)
            }
            StatementKind::Return(kind) => match kind {
                crate::ast::ReturnKind::Value(value) => collect_expr_calls(value, calls),
                crate::ast::ReturnKind::Command(command) => {
                    collect_synchronous_calls(
                        std::slice::from_ref(command.as_ref()),
                        reachable_tag_functions,
                        calls,
                    );
                }
                _ => {}
            },
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
            | StatementKind::UiCommand(_)
            | StatementKind::AdvancementAction { .. }
            | StatementKind::Schedule { .. }
            | StatementKind::ScheduleClear { .. }
            | StatementKind::ScoreSet { .. }
            | StatementKind::ScoreReset { .. }
            | StatementKind::ScoreboardEnable { .. }
            | StatementKind::ScoreboardOperation { .. }
            | StatementKind::ScoreboardDisplay { .. }
            | StatementKind::ScoreboardCommand(_)
            | StatementKind::DataMerge { .. }
            | StatementKind::DataRemove { .. }
            | StatementKind::DataModify { .. }
            | StatementKind::ItemAction { .. }
            | StatementKind::Teleport { .. }
            | StatementKind::Break
            | StatementKind::Continue
            | StatementKind::NbtMerge { .. } => {}
        }
    }
}

fn collect_condition_calls<'a>(condition: &'a Condition, calls: &mut HashSet<&'a str>) {
    match condition {
        Condition::Predicate { .. } => {}
        Condition::Function { target, .. } => {
            // `execute if function` 会在运行期调用目标函数，属于调用图的一部分。
            if let CallTarget::Function(name) = target {
                calls.insert(name);
            }
        }
        Condition::Block { .. }
        | Condition::Blocks { .. }
        | Condition::Biome { .. }
        | Condition::Loaded { .. }
        | Condition::Dimension { .. }
        | Condition::Entity { .. }
        | Condition::Data { .. }
        | Condition::Items { .. }
        | Condition::Slots { .. }
        | Condition::Stopwatch { .. } => {}
        Condition::Compare { left, right, .. } => {
            collect_expr_calls(left, calls);
            collect_expr_calls(right, calls);
        }
        Condition::Not(condition) => collect_condition_calls(condition, calls),
        Condition::And(left, right) | Condition::Or(left, right) => {
            collect_condition_calls(left, calls);
            collect_condition_calls(right, calls);
        }
    }
}

fn collect_expr_calls<'a>(expression: &'a Expr, calls: &mut HashSet<&'a str>) {
    match &expression.kind {
        ExprKind::Call {
            function,
            arguments,
        } => {
            calls.insert(function);
            for argument in arguments {
                collect_expr_calls(argument, calls);
            }
        }
        ExprKind::Negate(value) => collect_expr_calls(value, calls),
        ExprKind::Binary { left, right, .. } => {
            collect_expr_calls(left, calls);
            collect_expr_calls(right, calls);
        }
        ExprKind::CoreCommand(_)
        | ExprKind::EntityCommand(_)
        | ExprKind::Integer(_)
        | ExprKind::Score(_)
        | ExprKind::ScoreQuery { .. }
        | ExprKind::XpQuery { .. }
        | ExprKind::StopwatchQuery { .. }
        | ExprKind::TimeQuery { .. }
        | ExprKind::GameTimeQuery
        | ExprKind::GameRuleQuery { .. }
        | ExprKind::WorldBorderSize
        | ExprKind::BossBarGet { .. }
        | ExprKind::Count { .. }
        | ExprKind::Random { .. }
        | ExprKind::DataGet { .. }
        | ExprKind::Compute { .. } => {}
    }
}
