//! 同步调用图分析：拒绝任何直接或间接的同步递归。
//!
//! `schedule` 在未来的游戏刻建立新的调用，因此不属于同步调用图。函数标签调用
//! 会先展开为可达函数集合，保证 `fn a() { call #t(); }` 这类经标签形成的环也
//! 会被检测到。

use std::collections::{HashMap, HashSet};

use crate::ast::{
    CallTarget, Condition, Expr, ExprKind, FunctionTagDecl, Program, Statement, StatementKind,
};
use crate::diagnostic::Diagnostic;

use super::tags::reachable_functions;

pub(super) fn validate_synchronous_recursion(program: &Program, diagnostics: &mut Vec<Diagnostic>) {
    let known_functions = program
        .functions
        .iter()
        .map(|function| function.name.as_str())
        .collect::<HashSet<_>>();
    let tags = program
        .function_tags
        .iter()
        .map(|tag| (tag.name.as_str(), tag))
        .collect::<HashMap<&str, &FunctionTagDecl>>();
    let mut graph = HashMap::<&str, HashSet<&str>>::new();
    for function in &program.functions {
        let mut calls = HashSet::new();
        collect_synchronous_calls(&function.body, &tags, &mut calls);
        calls.retain(|callee| known_functions.contains(callee));
        graph.insert(&function.name, calls);
    }

    for function in &program.functions {
        let mut visited = HashSet::new();
        if reaches_function(&function.name, &function.name, &graph, &mut visited) {
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
    tags: &HashMap<&'a str, &'a FunctionTagDecl>,
    calls: &mut HashSet<&'a str>,
) {
    for statement in statements {
        match &statement.kind {
            StatementKind::MacroCall { target: CallTarget::Function(name), .. } => { calls.insert(name); }
            StatementKind::MacroCall { target: CallTarget::Tag(name), .. } => { calls.extend(reachable_functions(name, tags)); }
            StatementKind::MacroCall { .. } => {},
            StatementKind::CoreCommand(_) | StatementKind::EntityCommand(_) => {}
            StatementKind::Call { target, arguments } => {
                match target {
                    CallTarget::External(_) => {},
                    CallTarget::Function(function) => {
                        calls.insert(function);
                    }
                    CallTarget::Tag(tag) => {
                        calls.extend(reachable_functions(tag, tags));
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
                collect_synchronous_calls(then_body, tags, calls);
                collect_synchronous_calls(else_body, tags, calls);
            }
            StatementKind::While { condition, body } => {
                collect_condition_calls(condition, calls);
                collect_synchronous_calls(body, tags, calls);
            }
            StatementKind::For {
                start, end, body, ..
            } => {
                collect_expr_calls(start, calls);
                collect_expr_calls(end, calls);
                collect_synchronous_calls(body, tags, calls);
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
                collect_synchronous_calls(body, tags, calls);
            }
            StatementKind::Each { body, .. }
            | StatementKind::InDimension { body, .. }
            | StatementKind::Spawn { body, .. } => collect_synchronous_calls(body, tags, calls),
            StatementKind::Assign { value, .. } | StatementKind::Let { value, .. } => {
                collect_expr_calls(value, calls)
            }
            StatementKind::Return(kind) => {
                if let crate::ast::ReturnKind::Value(value) = kind {
                    collect_expr_calls(value, calls);
                }
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
            | StatementKind::AdvancementAction { .. }
            | StatementKind::Schedule { .. }
            | StatementKind::ScheduleClear { .. }
            | StatementKind::ScoreSet { .. }
            | StatementKind::ScoreReset { .. }
            | StatementKind::ScoreboardEnable { .. }
            | StatementKind::ScoreboardOperation { .. }
            | StatementKind::ScoreboardDisplay { .. }
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
        | ExprKind::Count { .. }
        | ExprKind::Random { .. }
        | ExprKind::DataGet { .. }
        | ExprKind::Compute { .. } => {}
    }
}

fn reaches_function<'a>(
    current: &'a str,
    target: &str,
    graph: &HashMap<&'a str, HashSet<&'a str>>,
    visited: &mut HashSet<&'a str>,
) -> bool {
    let Some(callees) = graph.get(current) else {
        return false;
    };
    for callee in callees {
        if *callee == target {
            return true;
        }
        if visited.insert(callee) && reaches_function(callee, target, graph, visited) {
            return true;
        }
    }
    false
}
