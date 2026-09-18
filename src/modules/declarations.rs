use crate::ast::{Program, Span};
use crate::name_walk::NameRole;

/// 按声明顺序收集模块里的顶层声明。
pub(super) fn collect_declarations(program: &Program) -> Vec<(NameRole, String, bool, Span)> {
    let mut declarations = Vec::new();
    for score in &program.scores {
        declarations.push((
            NameRole::Score,
            score.name.clone(),
            score.exported,
            score.span,
        ));
    }
    for objective in &program.objectives {
        declarations.push((
            NameRole::Objective,
            objective.name.clone(),
            objective.exported,
            objective.span,
        ));
    }
    for query in &program.queries {
        declarations.push((
            NameRole::Query,
            query.name.clone(),
            query.exported,
            query.span,
        ));
    }
    for item in &program.item_stacks {
        declarations.push((NameRole::Item, item.name.clone(), item.exported, item.span));
    }
    for storage in &program.storages {
        declarations.push((
            NameRole::Storage,
            storage.name.clone(),
            storage.exported,
            storage.span,
        ));
    }
    for slot in &program.data_slots {
        declarations.push((
            NameRole::DataSlot,
            slot.name.clone(),
            slot.exported,
            slot.span,
        ));
    }
    for resource in &program.resources {
        declarations.push((
            NameRole::Resource,
            resource.name.clone(),
            resource.exported,
            resource.span,
        ));
    }
    for advancement in &program.advancements {
        declarations.push((
            NameRole::Advancement,
            advancement.name.clone(),
            advancement.exported,
            advancement.span,
        ));
    }
    for tag in &program.function_tags {
        declarations.push((NameRole::Tag, tag.name.clone(), tag.exported, tag.span));
    }
    for function in &program.functions {
        declarations.push((
            NameRole::Function,
            function.name.clone(),
            function.exported,
            function.span,
        ));
    }
    declarations
}

pub(super) fn first_span(program: &Program) -> Span {
    if let Some(span) = program.namespace_span {
        return span;
    }
    let mut spans = Vec::new();
    spans.extend(program.scores.iter().map(|declaration| declaration.span));
    spans.extend(
        program
            .objectives
            .iter()
            .map(|declaration| declaration.span),
    );
    spans.extend(program.queries.iter().map(|declaration| declaration.span));
    spans.extend(
        program
            .item_stacks
            .iter()
            .map(|declaration| declaration.span),
    );
    spans.extend(program.storages.iter().map(|declaration| declaration.span));
    spans.extend(
        program
            .data_slots
            .iter()
            .map(|declaration| declaration.span),
    );
    spans.extend(program.resources.iter().map(|declaration| declaration.span));
    spans.extend(
        program
            .advancements
            .iter()
            .map(|declaration| declaration.span),
    );
    spans.extend(
        program
            .function_tags
            .iter()
            .map(|declaration| declaration.span),
    );
    spans.extend(program.functions.iter().map(|declaration| declaration.span));
    spans.into_iter().next().unwrap_or_default()
}
