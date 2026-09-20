use super::{Compiler, emit::entity_query_as_clause};
use crate::ast::*;

impl Compiler<'_> {
    pub(super) fn entity_command(&mut self, command: &EntityCommand, owner: &str) -> String {
        self.capture_command_targets(&entity_holders(command), owner, |c| {
            c.entity_command_text(command)
        })
    }

    pub(super) fn core_command(&mut self, command: &CoreCommand, owner: &str) -> String {
        self.capture_command_targets(&core_holders(command), owner, |c| {
            c.core_command_text(command)
        })
    }

    pub(super) fn ui_command(&mut self, command: &UiCommand, owner: &str) -> String {
        self.capture_command_targets(&ui_holders(command), owner, |c| {
            c.compile_ui_command(command)
        })
    }

    /// Invoke a multi-target command once: an `each` loop would change tag
    /// intersections, loot rolls and result counts. Preserve failure after cleanup.
    pub(super) fn capture_command_targets(
        &mut self,
        holders: &[&Holder],
        owner: &str,
        emit: impl FnOnce(&Self) -> String,
    ) -> String {
        let mut queries = std::collections::BTreeSet::new();
        for holder in holders {
            if let Holder::Query(name, _) = holder
                && self.query(name).item.is_some()
            {
                queries.insert(name.as_str());
            }
        }
        if queries.is_empty() {
            return emit(self);
        }
        let mut commands = Vec::new();
        let mut cleanup = Vec::new();
        for name in queries {
            let objective = self
                .selector_objectives
                .entry(name.to_owned())
                .or_insert_with(|| {
                    super::names::objective_name(&format!(
                        "{}:selector:{name}",
                        self.program.namespace
                    ))
                })
                .clone();
            let query = self.query(name);
            let single = if query.limit == Some(1) {
                ",limit=1"
            } else {
                ""
            };
            let player = if query.entity_type == "minecraft:player" {
                ",type=minecraft:player"
            } else {
                ""
            };
            let selector = format!("@e[scores={{{objective}=1}}{player}{single}]");
            let remove = format!("scoreboard players reset * {objective}");
            commands.push(remove.clone());
            commands.push(format!(
                "execute {} run scoreboard players set @s {objective} 1",
                entity_query_as_clause(query)
            ));
            cleanup.push(remove);
            self.selector_overrides.insert(name.to_owned(), selector);
        }
        let native = emit(self);
        self.selector_overrides.clear();
        let result = self.temporary();
        let success = self.temporary();
        commands.push(format!(
            "scoreboard players set {result} {} 0",
            self.objective
        ));
        commands.push(format!(
            "scoreboard players set {success} {} 0",
            self.objective
        ));
        commands.push(format!(
            "execute store result score {result} {} store success score {success} {} run {native}",
            self.objective, self.objective
        ));
        commands.extend(cleanup);
        commands.push(format!(
            "execute if score {success} {} matches 0 run return fail",
            self.objective
        ));
        commands.push(format!(
            "return run scoreboard players get {result} {}",
            self.objective
        ));
        let path = self.next_helper_path(owner);
        self.functions.insert(path.clone(), commands);
        format!("function {}:{path}", self.program.namespace)
    }
}

fn ui_holders(command: &UiCommand) -> Vec<&Holder> {
    match command {
        UiCommand::Title { targets, .. }
        | UiCommand::Dialog { targets, .. }
        | UiCommand::StopSound { targets, .. }
        | UiCommand::PrivateMessage { targets, .. } => vec![targets],
        UiCommand::BossBar(BossBarAction::Set {
            property: BossBarProperty::Players(Some(targets)),
            ..
        }) => vec![targets],
        UiCommand::Particle(particle) => particle.viewers.iter().collect(),
        UiCommand::PostEffect(action) => match action {
            PostEffectAction::Add { targets, .. }
            | PostEffectAction::Remove { targets, .. }
            | PostEffectAction::Clear(targets)
            | PostEffectAction::List(targets) => vec![targets],
        },
        UiCommand::BossBar(_) | UiCommand::TeamMessage(_) => Vec::new(),
    }
}

fn facing_holder(facing: &Facing) -> Option<&Holder> {
    if let Facing::Entity { target, .. } = facing {
        Some(target)
    } else {
        None
    }
}

fn entity_holders(command: &EntityCommand) -> Vec<&Holder> {
    match command {
        EntityCommand::Kill(target)
        | EntityCommand::Swing { target, .. }
        | EntityCommand::GameMode { target, .. }
        | EntityCommand::SpawnPoint { target, .. } => target.iter().collect(),
        EntityCommand::Tag { target, .. }
        | EntityCommand::Enchant { target, .. }
        | EntityCommand::Attribute { target, .. }
        | EntityCommand::Spread { target, .. } => vec![target],
        EntityCommand::Damage { target, source, .. } => {
            let mut holders = vec![target];
            if let Some(DamageOrigin::By { entity, cause }) = source {
                holders.push(entity);
                holders.extend(cause);
            }
            holders
        }
        EntityCommand::Ride { target, vehicle } => std::iter::once(target).chain(vehicle).collect(),
        EntityCommand::Rotate { target, facing } => std::iter::once(target)
            .chain(facing_holder(facing))
            .collect(),
        EntityCommand::Spectate { target, player } => target.iter().chain(player).collect(),
        EntityCommand::Team(
            TeamOperation::Join {
                members: Some(TeamMembers::Entities(target)),
                ..
            }
            | TeamOperation::Leave(TeamMembers::Entities(target)),
        ) => vec![target],
        EntityCommand::Waypoint(operation) => match operation {
            WaypointOperation::Color { target, .. }
            | WaypointOperation::Hex { target, .. }
            | WaypointOperation::ResetColor(target)
            | WaypointOperation::Style { target, .. }
            | WaypointOperation::ResetStyle(target) => vec![target],
            WaypointOperation::List => Vec::new(),
        },
        _ => Vec::new(),
    }
}

fn core_holders(command: &CoreCommand) -> Vec<&Holder> {
    let mut holders = Vec::new();
    match command {
        CoreCommand::Recipe { target, .. } => holders.push(target),
        CoreCommand::Loot { target, source } => {
            match target {
                LootTarget::Give(target)
                | LootTarget::Replace {
                    target: ItemConditionSource::Entity(target),
                    ..
                } => holders.push(target),
                _ => {}
            }
            if let LootSource::Kill(target) = source.as_ref() {
                holders.push(target);
            }
        }
        _ => {}
    }
    holders
}
