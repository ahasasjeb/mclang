//! 整程序语义检查。
//!
//! [`validate`] 按源码顺序检查顶层声明，收集符号表，再逐函数校验函数体，
//! 最后分析同步调用图。所有诊断都会返回，不提前退出，方便用户一次修完。

mod expressions;
mod items;
mod recursion;
mod rules;
mod statements;

use std::collections::{HashMap, HashSet};

use crate::ast::*;
use crate::diagnostic::Diagnostic;

use super::types::{ExecutionContext, ReturnRules, Signature, StatementSymbols};
use items::{validate_entity_query, validate_item_stack};
use recursion::validate_synchronous_recursion;
use rules::{
    function_context, supported_resource_kind, valid_name, valid_nbt_path, valid_resource_location,
    valid_resource_path, validate_identifier, windows_reserved_name,
};
use statements::{collect_local_declarations, validate_statements};

pub(super) fn validate(program: &Program) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    validate_namespace(program, &mut diagnostics);
    let scores = collect_scores(program, &mut diagnostics);
    let declarations = Declarations {
        queries: collect_queries(program, &mut diagnostics),
        item_stacks: collect_item_stacks(program, &mut diagnostics),
        storages: collect_storages(program, &mut diagnostics),
        predicates: validate_resources(program, &mut diagnostics),
        signatures: collect_signatures(program, &scores, &mut diagnostics),
        scores,
    };

    validate_function_bodies(program, &declarations, &mut diagnostics);
    validate_synchronous_recursion(program, &mut diagnostics);
    diagnostics
}

/// 顶层声明收集出的符号表，供函数体校验共享。
struct Declarations<'a> {
    scores: HashSet<&'a str>,
    queries: HashMap<&'a str, &'a EntityQueryDecl>,
    item_stacks: HashMap<&'a str, &'a ItemStackDecl>,
    storages: HashSet<&'a str>,
    predicates: HashSet<&'a str>,
    signatures: HashMap<&'a str, Signature>,
}

fn validate_namespace(program: &Program, diagnostics: &mut Vec<Diagnostic>) {
    if !valid_name(&program.namespace) || windows_reserved_name(&program.namespace) {
        diagnostics.push(Diagnostic::new(
            "命名空间只能包含小写 ASCII 字母、数字和下划线，不能以数字开头或使用 Windows 保留设备名",
            program.namespace_span,
        ));
    }
}

fn collect_scores<'a>(program: &'a Program, diagnostics: &mut Vec<Diagnostic>) -> HashSet<&'a str> {
    let mut scores = HashSet::new();
    for score in &program.scores {
        validate_identifier("计分变量", &score.name, score.span, diagnostics);
        if !scores.insert(score.name.as_str()) {
            diagnostics.push(Diagnostic::new(
                format!("重复声明计分变量 `{}`", score.name),
                score.span,
            ));
        }
    }
    scores
}

fn collect_queries<'a>(
    program: &'a Program,
    diagnostics: &mut Vec<Diagnostic>,
) -> HashMap<&'a str, &'a EntityQueryDecl> {
    let mut names = HashSet::new();
    let mut queries = HashMap::new();
    for query in &program.queries {
        validate_identifier("实体查询", &query.name, query.span, diagnostics);
        if !names.insert(query.name.as_str()) {
            diagnostics.push(Diagnostic::new(
                format!("重复声明实体查询 `{}`", query.name),
                query.span,
            ));
        }
        queries.insert(query.name.as_str(), query);
        validate_entity_query(query, diagnostics);
    }
    queries
}

fn collect_item_stacks<'a>(
    program: &'a Program,
    diagnostics: &mut Vec<Diagnostic>,
) -> HashMap<&'a str, &'a ItemStackDecl> {
    let mut item_stacks = HashMap::new();
    for item in &program.item_stacks {
        validate_identifier("物品定义", &item.name, item.span, diagnostics);
        if item_stacks.insert(item.name.as_str(), item).is_some() {
            diagnostics.push(Diagnostic::new(
                format!("重复声明物品定义 `{}`", item.name),
                item.span,
            ));
        }
        validate_item_stack(item, diagnostics);
    }
    item_stacks
}

fn collect_storages<'a>(
    program: &'a Program,
    diagnostics: &mut Vec<Diagnostic>,
) -> HashSet<&'a str> {
    let mut storages = HashSet::new();
    for storage in &program.storages {
        validate_identifier("物品存储", &storage.name, storage.span, diagnostics);
        if !storages.insert(storage.name.as_str()) {
            diagnostics.push(Diagnostic::new(
                format!("重复声明物品存储 `{}`", storage.name),
                storage.span,
            ));
        }
        if !valid_resource_location(&storage.storage_id) {
            diagnostics.push(Diagnostic::new(
                format!("`{}` 不是有效的存储资源位置", storage.storage_id),
                storage.span,
            ));
        }
        if !valid_nbt_path(&storage.path) {
            diagnostics.push(Diagnostic::new(
                format!("`{}` 不是受支持的存储路径", storage.path),
                storage.span,
            ));
        }
    }
    storages
}

/// 校验 JSON 资源声明，并返回可作为谓词条件引用的资源名集合。
fn validate_resources<'a>(
    program: &'a Program,
    diagnostics: &mut Vec<Diagnostic>,
) -> HashSet<&'a str> {
    let mut resources = HashSet::new();
    for resource in &program.resources {
        if !supported_resource_kind(&resource.kind) {
            diagnostics.push(Diagnostic::new(
                format!("Minecraft 26.3 不支持 JSON 资源类型 `{}`", resource.kind),
                resource.span,
            ));
        }
        if !valid_resource_path(&resource.name) {
            diagnostics.push(Diagnostic::new(
                format!("资源名称 `{}` 不是有效的资源路径", resource.name),
                resource.span,
            ));
        }
        if !resources.insert((resource.kind.as_str(), resource.name.as_str())) {
            diagnostics.push(Diagnostic::new(
                format!("重复声明资源 `{}/{}`", resource.kind, resource.name),
                resource.span,
            ));
        }
        if let Err(error) = serde_json::from_str::<serde_json::Value>(&resource.json) {
            diagnostics.push(Diagnostic::new(
                format!(
                    "资源 `{}/{}` 的 JSON 无效（JSON 第 {} 行第 {} 列）：{}",
                    resource.kind,
                    resource.name,
                    error.line(),
                    error.column(),
                    error
                ),
                resource.span,
            ));
        }
    }
    program
        .resources
        .iter()
        .filter(|resource| resource.kind == "predicate")
        .map(|resource| resource.name.as_str())
        .collect()
}

/// 校验函数声明本身（名称、签名、属性和返回约定），并建立签名表。
fn collect_signatures<'a>(
    program: &'a Program,
    scores: &HashSet<&'a str>,
    diagnostics: &mut Vec<Diagnostic>,
) -> HashMap<&'a str, Signature> {
    let mut functions = HashSet::new();
    let mut signatures = HashMap::new();
    for function in &program.functions {
        validate_identifier("函数", &function.name, function.span, diagnostics);
        if !functions.insert(function.name.as_str()) {
            diagnostics.push(Diagnostic::new(
                format!("重复声明函数 `{}`", function.name),
                function.span,
            ));
        }
        signatures.insert(
            function.name.as_str(),
            Signature {
                parameters: function.parameters.len(),
                returns_score: function.returns_score,
                required_context: function_context(function),
            },
        );
        validate_function_declaration(function, scores, diagnostics);
    }
    signatures
}

fn validate_function_declaration(
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
            Some(StatementKind::Return(Some(_)))
        )
    {
        diagnostics.push(Diagnostic::new(
            format!(
                "返回 score 的函数 `{}` 必须以 return 表达式结束",
                function.name
            ),
            function.span,
        ));
    }
}

/// 逐函数校验函数体：先收集全部局部声明，再按块作用域检查语句。
fn validate_function_bodies(
    program: &Program,
    declarations: &Declarations<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for function in &program.functions {
        let parameters = function
            .parameters
            .iter()
            .map(|parameter| parameter.name.as_str())
            .collect::<HashSet<_>>();
        let mut declared_locals = HashSet::new();
        collect_local_declarations(
            &function.body,
            &declarations.scores,
            &parameters,
            &mut declared_locals,
            diagnostics,
        );
        let mut visible_locals = HashSet::new();
        let symbols = StatementSymbols {
            scores: &declarations.scores,
            parameters: &parameters,
            functions: &declarations.signatures,
            queries: &declarations.queries,
            item_stacks: &declarations.item_stacks,
            storages: &declarations.storages,
            predicates: &declarations.predicates,
        };
        validate_statements(
            &function.body,
            &mut visible_locals,
            &symbols,
            function_context(function),
            ReturnRules {
                returns_score: function.returns_score,
                allowed_here: true,
            },
            diagnostics,
        );
    }
}
