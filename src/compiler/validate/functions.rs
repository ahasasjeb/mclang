use std::collections::HashSet;

use crate::ast::*;
use crate::diagnostic::Diagnostic;

use super::Declarations;
use super::rules::{function_context, validate_identifier};
use super::statements::{collect_local_declarations, validate_statements};
use crate::compiler::types::{ReturnRules, StatementSymbols};

pub(super) fn validate_function_declaration(
    function: &Function,
    scores: &HashSet<&str>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut parameters = HashSet::new();
    for parameter in &function.parameters {
        validate_identifier("参数", &parameter.name, parameter.span, diagnostics);
        if !parameters.insert(parameter.name.as_str()) {
            diagnostics.push(Diagnostic::new(
                format!("重复声明参数 `{}`", parameter.name),
                parameter.span,
            ));
        }
        if scores.contains(parameter.name.as_str()) {
            diagnostics.push(Diagnostic::new(
                format!("参数 `{}` 与全局计分变量重名", parameter.name),
                parameter.span,
            ));
        }
    }
    let mut attributes = HashSet::new();
    for attribute in &function.attributes {
        if !attributes.insert(*attribute) {
            diagnostics.push(Diagnostic::new(
                "同一个函数不能重复使用相同属性",
                function.span,
            ));
        }
    }
    let is_entry = function.attributes.contains(&Attribute::Load)
        || function.attributes.contains(&Attribute::Tick);
    if is_entry && !function.parameters.is_empty() {
        diagnostics.push(Diagnostic::new(
            "@load 和 @tick 入口函数不能声明参数",
            function.span,
        ));
    }
    if is_entry && function.returns_score {
        diagnostics.push(Diagnostic::new(
            "@load 和 @tick 入口函数不能返回值",
            function.span,
        ));
    }
    if is_entry
        && function.attributes.iter().any(|attribute| {
            matches!(
                attribute,
                Attribute::Entity | Attribute::Player | Attribute::NonPlayer
            )
        })
    {
        diagnostics.push(Diagnostic::new(
            "@entity、@non_player 和 @player 不能与 @load 或 @tick 用在同一个函数上",
            function.span,
        ));
    }
    let entity_attributes = function
        .attributes
        .iter()
        .filter(|attribute| {
            matches!(
                attribute,
                Attribute::Entity | Attribute::Player | Attribute::NonPlayer
            )
        })
        .count();
    if entity_attributes > 1 {
        diagnostics.push(Diagnostic::new(
            "同一个函数只能在 @entity、@non_player 和 @player 中选择一个执行上下文属性",
            function.span,
        ));
    }
    if function.returns_score
        && !matches!(
            function.body.last().map(|statement| &statement.kind),
            Some(StatementKind::Return(_))
        )
    {
        diagnostics.push(Diagnostic::new(
            format!("返回 score 的函数 `{}` 必须以 return 结束", function.name),
            function.span,
        ));
    }
}

/// 逐函数校验函数体：先收集全部局部声明，再按块作用域检查语句。
pub(super) fn validate_function_bodies(
    program: &Program,
    declarations: &Declarations<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for function in &program.functions {
        let loot_tables = program
            .resources
            .iter()
            .filter(|r| r.kind == "loot_table")
            .map(|r| r.name.as_str())
            .collect();
        let recipes = program
            .resources
            .iter()
            .filter(|r| r.kind == "recipe")
            .map(|r| r.name.as_str())
            .collect();
        let parameters = function
            .parameters
            .iter()
            .map(|parameter| parameter.name.as_str())
            .collect::<HashSet<_>>();
        collect_local_declarations(
            &function.body,
            &declarations.scores,
            &parameters,
            diagnostics,
        );
        let mut visible_locals = HashSet::new();
        let symbols = StatementSymbols {
            loot_tables: &loot_tables,
            recipes: &recipes,
            scores: &declarations.scores,
            objectives: &declarations.objectives,
            objective_declarations: &declarations.objective_declarations,
            parameters: &parameters,
            functions: &declarations.signatures,
            queries: &declarations.queries,
            item_stacks: &declarations.item_stacks,
            storages: &declarations.storages,
            data_slots: &declarations.data_slots,
            predicates: &declarations.predicates,
            advancements: &declarations.advancements,
            advancement_resources: &declarations.advancement_resources,
            function_tags: &declarations.function_tags,
        };
        validate_statements(
            &function.body,
            &mut visible_locals,
            &symbols,
            function_context(function),
            None,
            ReturnRules {
                returns_score: function.returns_score,
                allowed_here: true,
                loop_depth: 0,
            },
            diagnostics,
        );
    }
}
