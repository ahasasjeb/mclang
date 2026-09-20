use std::collections::HashSet;

use crate::ast::*;

use super::expressions::{
    call_target_names, component_names, condition_names, expression_names, holder_names,
    item_source_names, nbt_source_names, score_target_names,
};
use super::{NameContext, NameRole, NameSite};

/// 资源引用：`external` 为真时是完整的外部资源位置，不参与名称解析。
pub(super) fn reference_names(
    reference: &mut AdvancementReference,
    role: NameRole,
    visitor: &mut impl FnMut(&NameContext<'_>, NameSite, NameRole, &mut String),
    context: &NameContext<'_>,
) {
    if !reference.external {
        visitor(context, NameSite::Reference, role, &mut reference.name);
    }
}

/// 收集函数体内的全部 `let` 局部变量名（含嵌套块）。
pub fn collect_local_names<'a>(statements: &'a [Statement], locals: &mut Vec<&'a str>) {
    for statement in statements {
        match &statement.kind {
            StatementKind::Let { name, .. } => locals.push(name),
            StatementKind::For { variable, body, .. } => {
                locals.push(variable);
                collect_local_names(body, locals);
            }
            StatementKind::If {
                then_body,
                else_body,
                ..
            } => {
                collect_local_names(then_body, locals);
                collect_local_names(else_body, locals);
            }
            StatementKind::Execute { body, .. }
            | StatementKind::Each { body, .. }
            | StatementKind::InDimension { body, .. }
            | StatementKind::Spawn { body, .. }
            | StatementKind::While { body, .. } => collect_local_names(body, locals),
            _ => {}
        }
    }
}

/// [`collect_local_names`] 的独立版本：返回拥有所有权的名字集合。
pub fn collect_local_names_owned(statements: &[Statement]) -> HashSet<String> {
    let mut names = Vec::new();
    collect_local_names(statements, &mut names);
    names.into_iter().map(str::to_owned).collect()
}

pub(super) fn statement_names(
    statements: &mut [Statement],
    visitor: &mut impl FnMut(&NameContext<'_>, NameSite, NameRole, &mut String),
    context: &NameContext<'_>,
) {
    for statement in statements {
        statement_kind_names(&mut statement.kind, visitor, context);
    }
}

fn statement_kind_names(
    kind: &mut StatementKind,
    visitor: &mut impl FnMut(&NameContext<'_>, NameSite, NameRole, &mut String),
    context: &NameContext<'_>,
) {
    match kind {
        StatementKind::MacroCall { target, arguments } => {
            call_target_names(target, visitor, context);
            if let MacroArguments::With { source, .. } = arguments {
                nbt_source_names(source, visitor, context);
            }
        }
        StatementKind::CoreCommand(command) => {
            super::core_commands::core_command_names(command, visitor, context)
        }
        StatementKind::EntityCommand(command) => {
            super::entity_commands::entity_command_names(command, visitor, context)
        }
        StatementKind::Run(_) => {}
        StatementKind::Each { query, body } => {
            visitor(context, NameSite::Reference, NameRole::Query, query);
            statement_names(body, visitor, context);
        }
        StatementKind::InDimension { body, .. } | StatementKind::Spawn { body, .. } => {
            statement_names(body, visitor, context);
        }
        StatementKind::Give { target, item, .. } => {
            if let GiveTarget::Query(name) = target {
                visitor(context, NameSite::Reference, NameRole::Query, name);
            }
            if let GiveItem::Definition(name) = item {
                visitor(context, NameSite::Reference, NameRole::Item, name);
            }
        }
        StatementKind::EffectGive { target, .. }
        | StatementKind::EffectClear { target, .. }
        | StatementKind::XpChange { target, .. } => {
            visitor(context, NameSite::Reference, NameRole::Query, target);
        }
        StatementKind::StopwatchAction { .. } => {}
        StatementKind::ClearInventory { target, .. } => {
            if let Some(target) = target {
                holder_names(target, visitor, context);
            }
        }
        StatementKind::PlaySound { targets, .. } => {
            if let Some(targets) = targets {
                visitor(context, NameSite::Reference, NameRole::Query, targets);
            }
        }
        StatementKind::Call { target, arguments } => {
            call_target_names(target, visitor, context);
            for argument in arguments {
                expression_names(argument, visitor, context);
            }
        }
        StatementKind::Let { name, value, .. } => {
            visitor(context, NameSite::Declaration, NameRole::Local, name);
            expression_names(value, visitor, context);
        }
        StatementKind::Schedule { target, .. } => call_target_names(target, visitor, context),
        StatementKind::ScheduleClear { target } => call_target_names(target, visitor, context),
        StatementKind::Assign { target, value, .. } => {
            visitor(context, NameSite::Reference, NameRole::Score, target);
            expression_names(value, visitor, context);
        }
        StatementKind::ScoreSet { target, value } => {
            score_target_names(target, visitor, context);
            expression_names(value, visitor, context);
        }
        StatementKind::ScoreReset { target } | StatementKind::ScoreboardEnable { target } => {
            score_target_names(target, visitor, context);
        }
        StatementKind::ScoreboardOperation { result, source, .. } => {
            score_target_names(result, visitor, context);
            score_target_names(source, visitor, context);
        }
        StatementKind::ScoreboardDisplay { objective, .. } => {
            if let Some((name, _)) = objective {
                visitor(context, NameSite::Reference, NameRole::Objective, name);
            }
        }
        StatementKind::Teleport {
            targets,
            destination,
            rotation,
        } => {
            holder_names(targets, visitor, context);
            if let Some(facing) = rotation {
                super::entity_commands::facing_names(facing, visitor, context);
            }
            if let TeleportDestination::Entity { query, .. } = destination {
                visitor(context, NameSite::Reference, NameRole::Query, query);
            }
        }
        StatementKind::NbtMerge { .. } => {}
        StatementKind::DataMerge { target, .. }
        | StatementKind::DataRemove { target, .. }
        | StatementKind::DataModify { target, .. } => nbt_source_names(target, visitor, context),
        StatementKind::ItemAction { target, action, .. } => {
            item_source_names(target, visitor, context);
            match action {
                ItemActionKind::With(item, _) => {
                    visitor(context, NameSite::Reference, NameRole::Item, item);
                }
                ItemActionKind::From { source, .. } => {
                    item_source_names(source, visitor, context);
                }
                ItemActionKind::Modifier(_, _) => {}
            }
        }
        StatementKind::SelfAction(action) => self_action_names(action, visitor, context),
        StatementKind::Message { target, component } => {
            if let MessageTarget::Query { name, .. } = target {
                visitor(context, NameSite::Reference, NameRole::Query, name);
            }
            component_names(component, visitor, context);
        }
        StatementKind::UiCommand(command) => ui_command_names(command, visitor, context),
        StatementKind::AdvancementAction {
            targets,
            advancement,
            criterion,
            ..
        } => {
            holder_names(targets, visitor, context);
            if let Some(advancement) = advancement {
                reference_names(advancement, NameRole::Advancement, visitor, context);
            }
            if let Some(criterion) = criterion {
                visitor(context, NameSite::Reference, NameRole::Criterion, criterion);
            }
        }
        StatementKind::If {
            condition,
            then_body,
            else_body,
        } => {
            condition_names(condition, visitor, context);
            statement_names(then_body, visitor, context);
            statement_names(else_body, visitor, context);
        }
        StatementKind::While { condition, body } => {
            condition_names(condition, visitor, context);
            statement_names(body, visitor, context);
        }
        StatementKind::For {
            variable,
            start,
            end,
            body,
            ..
        } => {
            visitor(context, NameSite::Declaration, NameRole::Local, variable);
            expression_names(start, visitor, context);
            expression_names(end, visitor, context);
            statement_names(body, visitor, context);
        }
        StatementKind::Break | StatementKind::Continue => {}
        StatementKind::Execute { clauses, body } => {
            if let ExecuteClauses::Structured(clauses) = clauses {
                for clause in clauses {
                    clause_names(&mut clause.kind, visitor, context);
                }
            }
            statement_names(body, visitor, context);
        }
        StatementKind::Return(ReturnKind::Value(value)) => {
            expression_names(value, visitor, context)
        }
        StatementKind::Return(_) => {}
        StatementKind::SetBlock { .. }
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
        | StatementKind::Locate { .. } => {}
    }
}

fn ui_command_names(
    command: &mut UiCommand,
    visitor: &mut impl FnMut(&NameContext<'_>, NameSite, NameRole, &mut String),
    context: &NameContext<'_>,
) {
    match command {
        UiCommand::Title { targets, action } => {
            holder_names(targets, visitor, context);
            if let TitleAction::Text { component, .. } = action {
                component_names(component, visitor, context);
            }
        }
        UiCommand::BossBar(action) => match action {
            BossBarAction::Add { name, .. } => component_names(name, visitor, context),
            BossBarAction::Set { property, .. } => match property {
                BossBarProperty::Name(name) => component_names(name, visitor, context),
                BossBarProperty::Players(Some(targets)) => holder_names(targets, visitor, context),
                _ => {}
            },
            _ => {}
        },
        UiCommand::Dialog { targets, .. }
        | UiCommand::StopSound { targets, .. }
        | UiCommand::PrivateMessage { targets, .. } => holder_names(targets, visitor, context),
        UiCommand::Particle(particle) => {
            if let Some(viewers) = &mut particle.viewers {
                holder_names(viewers, visitor, context);
            }
        }
        UiCommand::PostEffect(action) => match action {
            PostEffectAction::Add { targets, .. }
            | PostEffectAction::Remove { targets, .. }
            | PostEffectAction::Clear(targets)
            | PostEffectAction::List(targets) => holder_names(targets, visitor, context),
        },
        UiCommand::TeamMessage(_) => {}
    }
}

fn self_action_names(
    action: &mut SelfAction,
    visitor: &mut impl FnMut(&NameContext<'_>, NameSite, NameRole, &mut String),
    context: &NameContext<'_>,
) {
    match action {
        SelfAction::AddTag(_)
        | SelfAction::RemoveTag(_)
        | SelfAction::SetInvulnerable(_)
        | SelfAction::SetNoGravity(_)
        | SelfAction::ClearItems
        | SelfAction::Remove => {}
        SelfAction::SaveItems(storage)
        | SelfAction::RestoreItems(storage)
        | SelfAction::RemovePreservingItems(storage) => {
            visitor(context, NameSite::Reference, NameRole::Storage, storage);
        }
        SelfAction::RemovePreservingSlot { slot, query, .. }
        | SelfAction::DataStore { slot, query, .. }
        | SelfAction::DataLoad { slot, query, .. } => {
            visitor(context, NameSite::Reference, NameRole::DataSlot, slot);
            visitor(context, NameSite::Reference, NameRole::Query, query);
        }
        SelfAction::GiveItem { item, .. } => {
            visitor(context, NameSite::Reference, NameRole::Item, item);
        }
        SelfAction::DataClear { slot, .. } => {
            visitor(context, NameSite::Reference, NameRole::DataSlot, slot);
        }
    }
}

fn clause_names(
    kind: &mut ExecuteClauseKind,
    visitor: &mut impl FnMut(&NameContext<'_>, NameSite, NameRole, &mut String),
    context: &NameContext<'_>,
) {
    match kind {
        ExecuteClauseKind::As { query, .. } | ExecuteClauseKind::At { query, .. } => {
            visitor(context, NameSite::Reference, NameRole::Query, query);
        }
        ExecuteClauseKind::FacingEntity { query, .. } => {
            visitor(context, NameSite::Reference, NameRole::Query, query);
        }
        ExecuteClauseKind::If(condition) | ExecuteClauseKind::Unless(condition) => {
            condition_names(condition, visitor, context);
        }
        ExecuteClauseKind::StoreResult(target) | ExecuteClauseKind::StoreSuccess(target) => {
            if let ExecuteStoreTarget::Score(target) = target {
                score_target_names(target, visitor, context);
            }
        }
        ExecuteClauseKind::StoreData(data) => nbt_source_names(&mut data.source, visitor, context),
        ExecuteClauseKind::Positioned(_)
        | ExecuteClauseKind::Rotated(_)
        | ExecuteClauseKind::FacingPosition(_)
        | ExecuteClauseKind::Align { .. }
        | ExecuteClauseKind::Anchored(_)
        | ExecuteClauseKind::In { .. }
        | ExecuteClauseKind::On(_)
        | ExecuteClauseKind::Summon { .. } => {}
    }
}
