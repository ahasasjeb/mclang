use super::{
    Compiler,
    emit::{item_stack_argument, reference_id},
    world::{position_text, position_value_text},
};
use crate::ast::*;

impl Compiler<'_> {
    pub(super) fn core_command_text(&self, command: &CoreCommand) -> String {
        match command {
            CoreCommand::Reload => "reload".to_owned(),
            CoreCommand::Help(command) => format!("help{}", optional(command.as_ref())),
            CoreCommand::Version => "version".to_owned(),
            CoreCommand::Seed => "seed".to_owned(),
            CoreCommand::Say(message) => format!("say {}", message.text),
            CoreCommand::Me(action) => format!("me {}", action.text),
            CoreCommand::FetchProfile(target) => match target {
                FetchProfileTarget::Name(name) => format!("fetchprofile name {name}"),
                FetchProfileTarget::Id(id) => format!("fetchprofile id {id}"),
                FetchProfileTarget::Entity(holder) => {
                    format!("fetchprofile entity {}", self.component_holder(holder))
                }
            },
            CoreCommand::Test(command) => test_command_text(command),
            CoreCommand::Recipe {
                give,
                target,
                recipe,
            } => format!(
                "recipe {} {} {}",
                if *give { "give" } else { "take" },
                self.component_holder(target),
                recipe.as_ref().map_or_else(
                    || "*".to_owned(),
                    |r| reference_id(&self.program.namespace, r)
                )
            ),
            CoreCommand::Random {
                roll,
                min,
                max,
                sequence,
            } => format!(
                "random {} {min}..{max}{}",
                if *roll { "roll" } else { "value" },
                optional(sequence.as_ref())
            ),
            CoreCommand::RandomReset {
                sequence,
                seed,
                world_seed,
                sequence_id,
            } => format!(
                "random reset {sequence}{}{}{}",
                optional(seed.as_ref()),
                optional(world_seed.as_ref()),
                optional(sequence_id.as_ref())
            ),
            CoreCommand::Datapack(operation) => match operation {
                DatapackOperation::Disable(name) => format!("datapack disable {}", quote(name)),
                DatapackOperation::List(kind) => {
                    format!("datapack list{}", optional(kind.as_ref()))
                }
                DatapackOperation::Enable { name, order } => format!(
                    "datapack enable {}{}",
                    quote(name),
                    match order {
                        None => String::new(),
                        Some(PackOrder::First) => " first".to_owned(),
                        Some(PackOrder::Last) => " last".to_owned(),
                        Some(PackOrder::Before(other)) => format!(" before {}", quote(other)),
                        Some(PackOrder::After(other)) => format!(" after {}", quote(other)),
                    }
                ),
            },
            CoreCommand::Loot { target, source } => format!(
                "loot {} {}",
                self.loot_target_text(target),
                self.loot_source_text(source)
            ),
        }
    }

    fn loot_target_text(&self, target: &LootTarget) -> String {
        match target {
            LootTarget::Give(holder) => format!("give {}", self.component_holder(holder)),
            LootTarget::Insert(pos) => format!("insert {}", position_text(pos)),
            LootTarget::Spawn(pos) => format!("spawn {}", position_value_text(pos)),
            LootTarget::Replace {
                target,
                slot,
                count,
            } => format!(
                "replace {} {slot}{}",
                self.item_condition_source_text(target),
                optional(count.as_ref())
            ),
        }
    }

    fn loot_source_text(&self, source: &LootSource) -> String {
        match source {
            LootSource::Table(table) => {
                format!("loot {}", reference_id(&self.program.namespace, table))
            }
            LootSource::Kill(holder) => format!("kill {}", self.component_holder(holder)),
            LootSource::Fish {
                table,
                position,
                tool,
            } => format!(
                "fish {} {}{}",
                reference_id(&self.program.namespace, table),
                position_text(position),
                self.loot_tool_text(tool)
            ),
            LootSource::Mine { position, tool } => format!(
                "mine {}{}",
                position_text(position),
                self.loot_tool_text(tool)
            ),
        }
    }

    fn loot_tool_text(&self, tool: &Option<LootTool>) -> String {
        match tool {
            None => String::new(),
            Some(LootTool::Hand(hand)) => format!(" {hand}"),
            Some(LootTool::Item(name)) => format!(
                " {}",
                item_stack_argument(
                    self.program
                        .item_stacks
                        .iter()
                        .find(|item| item.name == *name)
                        .expect("validated item")
                )
            ),
        }
    }
}

fn test_command_text(command: &TestCommand) -> String {
    match command {
        TestCommand::Run {
            method,
            tests,
            only_required,
            times,
            until_failed,
            rotation,
            per_row,
        } => format!(
            "test {method}{}{}{}{}{}{}",
            optional(tests.as_ref()),
            optional(only_required.as_ref()),
            optional(times.as_ref()),
            optional(until_failed.as_ref()),
            optional(rotation.as_ref()),
            optional(per_row.as_ref()),
        ),
        TestCommand::RunMultiple { tests, amount } => {
            format!("test runmultiple {tests}{}", optional(amount.as_ref()))
        }
        TestCommand::Verify(tests) => format!("test verify {tests}"),
        TestCommand::Locate(tests) => format!("test locate {tests}"),
        TestCommand::Simple(method) => format!("test {method}"),
        TestCommand::ClearAll(radius) => format!("test clearall{}", optional(radius.as_ref())),
        TestCommand::Pos(variable) => format!("test pos{}", optional(variable.as_ref())),
        TestCommand::Create { id, dimensions } => {
            let dimensions = dimensions
                .iter()
                .map(|v| format!(" {v}"))
                .collect::<String>();
            format!("test create {id}{dimensions}")
        }
    }
}

fn optional(value: Option<&impl std::fmt::Display>) -> String {
    value.map_or_else(String::new, |v| format!(" {v}"))
}

// Brigadier quoted strings only escape the delimiter and backslash, unlike JSON.
fn quote(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}
