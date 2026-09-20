use crate::ast::*;

use super::{NameContext, NameRole, NameSite};

pub(super) fn condition_names(
    condition: &mut Condition,
    visitor: &mut impl FnMut(&NameContext<'_>, NameSite, NameRole, &mut String),
    context: &NameContext<'_>,
) {
    match condition {
        Condition::Predicate { name, .. } => {
            visitor(context, NameSite::Reference, NameRole::Resource, name);
        }
        Condition::Compare { left, right, .. } => {
            expression_names(left, visitor, context);
            expression_names(right, visitor, context);
        }
        Condition::Entity { query, .. } => {
            visitor(context, NameSite::Reference, NameRole::Query, query);
        }
        Condition::Data { source, .. } => nbt_source_names(source, visitor, context),
        Condition::Items { source, .. } | Condition::Slots { source, .. } => {
            item_source_names(source, visitor, context);
        }
        Condition::Function { target, .. } => call_target_names(target, visitor, context),
        Condition::Not(inner) => condition_names(inner, visitor, context),
        Condition::And(left, right) | Condition::Or(left, right) => {
            condition_names(left, visitor, context);
            condition_names(right, visitor, context);
        }
        Condition::Block { .. }
        | Condition::Blocks { .. }
        | Condition::Biome { .. }
        | Condition::Loaded { .. }
        | Condition::Dimension { .. }
        | Condition::Stopwatch { .. } => {}
    }
}

pub(super) fn expression_names(
    expression: &mut Expr,
    visitor: &mut impl FnMut(&NameContext<'_>, NameSite, NameRole, &mut String),
    context: &NameContext<'_>,
) {
    match &mut expression.kind {
        ExprKind::CoreCommand(command) => {
            super::core_commands::core_command_names(command, visitor, context)
        }
        ExprKind::EntityCommand(command) => {
            super::entity_commands::entity_command_names(command, visitor, context)
        }
        ExprKind::Integer(_) => {}
        ExprKind::Score(name) => visitor(context, NameSite::Reference, NameRole::Score, name),
        ExprKind::Call {
            function,
            arguments,
        } => {
            visitor(context, NameSite::Reference, NameRole::Function, function);
            for argument in arguments {
                expression_names(argument, visitor, context);
            }
        }
        ExprKind::XpQuery { target, .. } => {
            visitor(context, NameSite::Reference, NameRole::Query, target);
        }
        ExprKind::ScoreQuery { target } => score_target_names(target, visitor, context),
        ExprKind::Count { query, .. } => {
            visitor(context, NameSite::Reference, NameRole::Query, query);
        }
        ExprKind::DataGet { source, .. } => nbt_source_names(source, visitor, context),
        ExprKind::Compute { source, .. } => {
            if let ComputeSource::Entity(holder) = source {
                holder_names(holder, visitor, context);
            }
        }
        ExprKind::Negate(value) => expression_names(value, visitor, context),
        ExprKind::Binary { left, right, .. } => {
            expression_names(left, visitor, context);
            expression_names(right, visitor, context);
        }
        ExprKind::StopwatchQuery { .. }
        | ExprKind::TimeQuery { .. }
        | ExprKind::GameTimeQuery
        | ExprKind::GameRuleQuery { .. }
        | ExprKind::WorldBorderSize
        | ExprKind::BossBarGet { .. }
        | ExprKind::Random { .. } => {}
    }
}

pub(super) fn component_names(
    component: &mut TextComponent,
    visitor: &mut impl FnMut(&NameContext<'_>, NameSite, NameRole, &mut String),
    context: &NameContext<'_>,
) {
    match &mut component.kind {
        TextComponentKind::Text(_) | TextComponentKind::Keybind(_) => {}
        TextComponentKind::Object(object) => {
            let fallback = match object {
                ObjectContent::Atlas { fallback, .. } | ObjectContent::Player { fallback, .. } => {
                    fallback
                }
            };
            if let Some(fallback) = fallback {
                component_names(fallback, visitor, context);
            }
        }
        TextComponentKind::Translate { args, .. } => {
            for arg in args {
                component_names(arg, visitor, context);
            }
        }
        TextComponentKind::Score {
            holder, objective, ..
        } => {
            holder_names(holder, visitor, context);
            if let ObjectiveRef::Declared(name) = objective {
                visitor(context, NameSite::Reference, NameRole::Objective, name);
            }
        }
        TextComponentKind::Selector(SelectorValue::Query(name, _)) => {
            visitor(context, NameSite::Reference, NameRole::Query, name);
        }
        TextComponentKind::Selector(SelectorValue::Raw(_, _)) => {}
        TextComponentKind::Nbt {
            source, separator, ..
        } => {
            nbt_source_names(source, visitor, context);
            if let Some(separator) = separator {
                component_names(separator, visitor, context);
            }
        }
    }
    if let Some(hover) = &mut component.style.hover {
        match hover {
            HoverEvent::Text(value) => component_names(value, visitor, context),
            HoverEvent::Entity {
                name: Some(name), ..
            } => {
                component_names(name, visitor, context);
            }
            HoverEvent::Item { .. } | HoverEvent::Entity { name: None, .. } => {}
        }
    }
}

pub(super) fn holder_names(
    holder: &mut Holder,
    visitor: &mut impl FnMut(&NameContext<'_>, NameSite, NameRole, &mut String),
    context: &NameContext<'_>,
) {
    if let Holder::Query(name, _) = holder {
        visitor(context, NameSite::Reference, NameRole::Query, name);
    }
}

pub(super) fn call_target_names(
    target: &mut CallTarget,
    visitor: &mut impl FnMut(&NameContext<'_>, NameSite, NameRole, &mut String),
    context: &NameContext<'_>,
) {
    match target {
        CallTarget::External(_) => {}
        CallTarget::Function(name) => {
            visitor(context, NameSite::Reference, NameRole::Function, name);
        }
        CallTarget::Tag(name) => {
            visitor(context, NameSite::Reference, NameRole::Tag, name);
        }
    }
}

pub(super) fn score_target_names(
    target: &mut ScoreTarget,
    visitor: &mut impl FnMut(&NameContext<'_>, NameSite, NameRole, &mut String),
    context: &NameContext<'_>,
) {
    holder_names(&mut target.holder, visitor, context);
    visitor(
        context,
        NameSite::Reference,
        NameRole::Objective,
        &mut target.objective,
    );
}

pub(super) fn nbt_source_names(
    source: &mut NbtComponentSource,
    visitor: &mut impl FnMut(&NameContext<'_>, NameSite, NameRole, &mut String),
    context: &NameContext<'_>,
) {
    if let NbtComponentSource::Entity(holder) = source {
        holder_names(holder, visitor, context);
    }
}

pub(super) fn item_source_names(
    source: &mut ItemConditionSource,
    visitor: &mut impl FnMut(&NameContext<'_>, NameSite, NameRole, &mut String),
    context: &NameContext<'_>,
) {
    if let ItemConditionSource::Entity(holder) = source {
        holder_names(holder, visitor, context);
    }
}
