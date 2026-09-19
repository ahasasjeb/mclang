//! 整程序语义检查。
//!
//! [`validate`] 按源码顺序检查顶层声明，收集符号表，再逐函数校验函数体，
//! 最后分析同步调用图。所有诊断都会返回，不提前退出，方便用户一次修完。

mod advancement;
mod collect;
mod components;
mod core_commands;
mod entity_commands;
mod entity_nbt;
mod expressions;
mod functions;
mod item_components;
mod items;
mod macros;
mod recursion;
mod registry;
pub(super) mod rules;
mod statements;
mod tags;
mod world;

use std::collections::{HashMap, HashSet};

use crate::ast::*;
use crate::diagnostic::Diagnostic;

use super::types::Signature;
use advancement::collect_advancements;
use recursion::validate_synchronous_recursion;
use tags::{collect_function_tags, validate_function_tags};

use collect::{
    collect_data_slots, collect_item_stacks, collect_objectives, collect_queries, collect_scores,
    collect_signatures, collect_storages, validate_namespace, validate_resources,
};
use functions::validate_function_bodies;

/// `resource` 声明按资源类型分组的名称集合，供进度声明与语句校验引用。
#[derive(Default)]
pub(super) struct ResourceSymbols<'a> {
    pub(super) predicates: HashSet<&'a str>,
    pub(super) loot_tables: HashSet<&'a str>,
    pub(super) recipes: HashSet<&'a str>,
    pub(super) advancements: HashSet<&'a str>,
}

pub(super) fn validate(program: &Program) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    validate_namespace(program, &mut diagnostics);
    let scores = collect_scores(program, &mut diagnostics);
    let objectives = collect_objectives(program, &mut diagnostics);
    let function_tags = collect_function_tags(program, &mut diagnostics);
    let signatures = collect_signatures(program, &scores, &mut diagnostics);
    validate_function_tags(&function_tags, &signatures, &mut diagnostics);
    let resources = validate_resources(program, &mut diagnostics);
    let item_stacks = collect_item_stacks(program, &mut diagnostics);
    let advancements = collect_advancements(
        program,
        &resources,
        &item_stacks,
        &signatures,
        &mut diagnostics,
    );
    let declarations = Declarations {
        queries: collect_queries(program, &mut diagnostics),
        item_stacks,
        storages: collect_storages(program, &mut diagnostics),
        data_slots: collect_data_slots(program, &mut diagnostics),
        predicates: resources.predicates.clone(),
        advancement_resources: resources.advancements.clone(),
        advancements,
        function_tags,
        signatures,
        scores,
        objectives,
        objective_declarations: program
            .objectives
            .iter()
            .map(|objective| (objective.name.as_str(), objective))
            .collect(),
    };

    validate_function_bodies(program, &declarations, &mut diagnostics);
    validate_synchronous_recursion(program, &mut diagnostics);
    diagnostics
}

/// 顶层声明收集出的符号表，供函数体校验共享。
struct Declarations<'a> {
    scores: HashSet<&'a str>,
    objectives: HashSet<&'a str>,
    /// 目标声明表，供 scoreboard.enable 等语句读取准则。
    objective_declarations: HashMap<&'a str, &'a ObjectiveDecl>,
    queries: HashMap<&'a str, &'a EntityQueryDecl>,
    item_stacks: HashMap<&'a str, &'a ItemStackDecl>,
    storages: HashSet<&'a str>,
    data_slots: HashMap<&'a str, &'a DataSlotDecl>,
    predicates: HashSet<&'a str>,
    /// `resource advancement` 声明的进度名，与结构化进度一起构成引用目标。
    advancement_resources: HashSet<&'a str>,
    advancements: HashMap<&'a str, &'a AdvancementDecl>,
    function_tags: HashMap<&'a str, &'a FunctionTagDecl>,
    signatures: HashMap<&'a str, Signature>,
}
