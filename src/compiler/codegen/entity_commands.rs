use super::{
    Compiler,
    names::user_objective_name,
    world::{position_text, position_value_text, rotation_text, vec2_text},
};
use crate::ast::*;

mod teams;

impl Compiler<'_> {
    pub(super) fn entity_command_text(&self, command: &EntityCommand) -> String {
        match command {
            EntityCommand::Kill(target) => {
                format!("kill{}", self.optional_entity_text(target.as_ref()))
            }
            EntityCommand::Tag { target, operation } => {
                let op = match operation {
                    TagOperation::Add(tag) => format!("add {tag}"),
                    TagOperation::Remove(tag) => format!("remove {tag}"),
                    TagOperation::List => "list".to_owned(),
                };
                format!("tag {} {op}", self.component_holder(target))
            }
            EntityCommand::Enchant {
                target,
                enchantment,
                level,
            } => format!(
                "enchant {} {enchantment}{}",
                self.component_holder(target),
                optional_text(level.as_ref())
            ),
            EntityCommand::Damage {
                target,
                amount,
                damage_type,
                source,
            } => {
                let mut command = format!(
                    "damage {} {amount}{}",
                    self.component_holder(target),
                    optional_text(damage_type.as_ref())
                );
                match source {
                    Some(DamageOrigin::At(pos)) => {
                        command.push_str(&format!(" at {}", position_value_text(pos)))
                    }
                    Some(DamageOrigin::By { entity, cause }) => {
                        command.push_str(&format!(" by {}", self.component_holder(entity)));
                        if let Some(cause) = cause {
                            command.push_str(&format!(" from {}", self.component_holder(cause)));
                        }
                    }
                    None => {}
                }
                command
            }
            EntityCommand::Attribute {
                target,
                attribute,
                operation,
            } => format!(
                "attribute {} {attribute} {}",
                self.component_holder(target),
                attribute_text(operation)
            ),
            EntityCommand::Ride { target, vehicle } => format!(
                "ride {} {}",
                self.component_holder(target),
                vehicle.as_ref().map_or_else(
                    || "dismount".to_owned(),
                    |v| format!("mount {}", self.component_holder(v))
                )
            ),
            EntityCommand::Rotate { target, facing } => format!(
                "rotate {} {}",
                self.component_holder(target),
                self.facing_text(facing)
            ),
            EntityCommand::Spread {
                center,
                spread,
                range,
                under,
                teams,
                target,
            } => format!(
                "spreadplayers {} {spread} {range}{} {teams} {}",
                vec2_text(center),
                under.map_or_else(String::new, |v| format!(" under {v}")),
                self.component_holder(target)
            ),
            EntityCommand::Spectate { target, player } => format!(
                "spectate{}{}",
                self.optional_entity_text(target.as_ref()),
                self.optional_entity_text(player.as_ref())
            ),
            EntityCommand::Swing {
                target,
                hand,
                animation,
                duration,
            } => format!(
                "swing{}{}{}{}",
                self.optional_entity_text(target.as_ref()),
                optional_text(hand.as_ref()),
                optional_text(animation.as_ref()),
                optional_text(duration.as_ref())
            ),
            EntityCommand::Trigger {
                objective,
                operation,
            } => format!(
                "trigger {}{}",
                user_objective_name(&self.program.namespace, objective),
                operation.map_or_else(String::new, |(add, value)| format!(
                    " {} {value}",
                    if add { "add" } else { "set" }
                ))
            ),
            EntityCommand::GameMode { mode, target } => format!(
                "gamemode {mode}{}",
                self.optional_entity_text(target.as_ref())
            ),
            EntityCommand::DefaultGameMode(mode) => format!("defaultgamemode {mode}"),
            EntityCommand::Difficulty(mode) => {
                format!("difficulty{}", optional_text(mode.as_ref()))
            }
            EntityCommand::SpawnPoint {
                target,
                position,
                rotation,
            } => format!(
                "spawnpoint{}{}{}",
                self.optional_entity_text(target.as_ref()),
                optional_text(position.as_ref().map(position_text).as_ref()),
                optional_text(rotation.as_ref().map(rotation_text).as_ref())
            ),
            EntityCommand::WorldSpawn { position, rotation } => format!(
                "setworldspawn{}{}",
                optional_text(position.as_ref().map(position_text).as_ref()),
                optional_text(rotation.as_ref().map(rotation_text).as_ref())
            ),
            EntityCommand::Team(operation) => self.team_command_text(operation),
            EntityCommand::Waypoint(operation) => self.waypoint_command_text(operation),
            EntityCommand::List { uuids } => if *uuids { "list uuids" } else { "list" }.to_owned(),
        }
    }

    pub(in crate::compiler::codegen) fn facing_text(&self, facing: &Facing) -> String {
        match facing {
            Facing::Rotation(rotation) => rotation_text(rotation),
            Facing::Position(position) => format!("facing {}", position_value_text(position)),
            Facing::Entity { target, anchor } => {
                format!("facing entity {} {anchor}", self.component_holder(target))
            }
        }
    }

    fn optional_entity_text(&self, holder: Option<&Holder>) -> String {
        holder.map_or_else(String::new, |v| format!(" {}", self.component_holder(v)))
    }
}

fn optional_text(value: Option<&impl std::fmt::Display>) -> String {
    value.map_or_else(String::new, |v| format!(" {v}"))
}

fn attribute_text(operation: &AttributeOperation) -> String {
    match operation {
        AttributeOperation::Get(scale) => format!("get{}", optional_text(scale.as_ref())),
        AttributeOperation::BaseGet(scale) => format!("base get{}", optional_text(scale.as_ref())),
        AttributeOperation::BaseSet(value) => format!("base set {value}"),
        AttributeOperation::BaseReset => "base reset".to_owned(),
        AttributeOperation::ModifierAdd {
            id,
            value,
            operation,
        } => format!("modifier add {id} {value} {operation}"),
        AttributeOperation::ModifierRemove(id) => format!("modifier remove {id}"),
        AttributeOperation::ModifierGet { id, scale } => {
            format!("modifier value get {id}{}", optional_text(scale.as_ref()))
        }
    }
}
