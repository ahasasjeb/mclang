use super::{
    NameContext, NameRole, NameSite,
    expressions::{holder_names, item_source_names},
    statements::reference_names,
};
use crate::ast::*;

pub(super) fn core_command_names(
    command: &mut CoreCommand,
    visitor: &mut impl FnMut(&NameContext<'_>, NameSite, NameRole, &mut String),
    context: &NameContext<'_>,
) {
    match command {
        CoreCommand::FetchProfile(FetchProfileTarget::Entity(target)) => {
            holder_names(target, visitor, context);
        }
        CoreCommand::Recipe { target, recipe, .. } => {
            holder_names(target, visitor, context);
            if let Some(recipe) = recipe {
                reference_names(recipe, NameRole::Resource, visitor, context);
            }
        }
        CoreCommand::Loot { target, source } => {
            match target {
                LootTarget::Give(holder) => holder_names(holder, visitor, context),
                LootTarget::Replace { target, .. } => item_source_names(target, visitor, context),
                _ => {}
            }
            match source.as_mut() {
                LootSource::Table(table) => {
                    reference_names(table, NameRole::Resource, visitor, context)
                }
                LootSource::Kill(holder) => holder_names(holder, visitor, context),
                LootSource::Fish { table, tool, .. } => {
                    reference_names(table, NameRole::Resource, visitor, context);
                    tool_names(tool, visitor, context);
                }
                LootSource::Mine { tool, .. } => tool_names(tool, visitor, context),
            }
        }
        _ => {}
    }
}

fn tool_names(
    tool: &mut Option<LootTool>,
    visitor: &mut impl FnMut(&NameContext<'_>, NameSite, NameRole, &mut String),
    context: &NameContext<'_>,
) {
    if let Some(LootTool::Item(name)) = tool {
        visitor(context, NameSite::Reference, NameRole::Item, name);
    }
}
