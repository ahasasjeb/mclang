use super::expressions::holder_names;
use super::{NameContext, NameRole, NameSite};
use crate::ast::*;

pub(super) fn entity_command_names(
    command: &mut EntityCommand,
    visitor: &mut impl FnMut(&NameContext<'_>, NameSite, NameRole, &mut String),
    context: &NameContext<'_>,
) {
    match command {
        EntityCommand::Kill(target)
        | EntityCommand::Swing { target, .. }
        | EntityCommand::GameMode { target, .. }
        | EntityCommand::SpawnPoint { target, .. } => {
            if let Some(target) = target {
                holder_names(target, visitor, context);
            }
        }
        EntityCommand::Tag { target, .. }
        | EntityCommand::Enchant { target, .. }
        | EntityCommand::Attribute { target, .. }
        | EntityCommand::Spread { target, .. } => holder_names(target, visitor, context),
        EntityCommand::Damage { target, source, .. } => {
            holder_names(target, visitor, context);
            if let Some(DamageOrigin::By { entity, cause }) = source {
                holder_names(entity, visitor, context);
                if let Some(cause) = cause {
                    holder_names(cause, visitor, context);
                }
            }
        }
        EntityCommand::Ride { target, vehicle } => {
            holder_names(target, visitor, context);
            if let Some(vehicle) = vehicle {
                holder_names(vehicle, visitor, context);
            }
        }
        EntityCommand::Rotate { target, facing } => {
            holder_names(target, visitor, context);
            facing_names(facing, visitor, context);
        }
        EntityCommand::Spectate { target, player } => {
            for holder in [target, player].into_iter().flatten() {
                holder_names(holder, visitor, context);
            }
        }
        EntityCommand::Trigger { objective, .. } => {
            visitor(context, NameSite::Reference, NameRole::Objective, objective)
        }
        EntityCommand::Team(operation) => match operation {
            TeamOperation::Add {
                display: Some(component),
                ..
            } => super::component_names(component, visitor, context),
            TeamOperation::Join {
                members: Some(TeamMembers::Entities(holder)),
                ..
            }
            | TeamOperation::Leave(TeamMembers::Entities(holder)) => {
                holder_names(holder, visitor, context)
            }
            TeamOperation::Modify {
                option:
                    TeamOption::DisplayName(component)
                    | TeamOption::Prefix(component)
                    | TeamOption::Suffix(component),
                ..
            } => super::component_names(component, visitor, context),
            _ => {}
        },
        EntityCommand::Waypoint(operation) => match operation {
            WaypointOperation::List => {}
            WaypointOperation::Color { target, .. }
            | WaypointOperation::Hex { target, .. }
            | WaypointOperation::ResetColor(target)
            | WaypointOperation::Style { target, .. }
            | WaypointOperation::ResetStyle(target) => holder_names(target, visitor, context),
        },
        EntityCommand::DefaultGameMode(_)
        | EntityCommand::Difficulty(_)
        | EntityCommand::WorldSpawn { .. }
        | EntityCommand::List { .. } => {}
    }
}

pub(super) fn facing_names(
    facing: &mut Facing,
    visitor: &mut impl FnMut(&NameContext<'_>, NameSite, NameRole, &mut String),
    context: &NameContext<'_>,
) {
    if let Facing::Entity { target, .. } = facing {
        holder_names(target, visitor, context);
    }
}
