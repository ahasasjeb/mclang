use super::{Compiler, names::user_objective_name};
use crate::ast::*;

impl Compiler<'_> {
    pub(super) fn scoreboard_command(
        &mut self,
        command: &ScoreboardCommand,
        owner: &str,
    ) -> String {
        let holders: Vec<&Holder> = match command {
            ScoreboardCommand::PlayersList(Some(holder))
            | ScoreboardCommand::PlayersResetAll(holder) => vec![holder],
            ScoreboardCommand::PlayersChange { target, .. }
            | ScoreboardCommand::PlayersDisplayName { target, .. }
            | ScoreboardCommand::PlayersNumberFormat { target, .. } => vec![&target.holder],
            _ => Vec::new(),
        };
        self.capture_command_targets(&holders, owner, |compiler| {
            compiler.scoreboard_command_text(command)
        })
    }

    fn scoreboard_command_text(&self, command: &ScoreboardCommand) -> String {
        match command {
            ScoreboardCommand::ObjectivesList => "scoreboard objectives list".to_owned(),
            ScoreboardCommand::ObjectivesRemove((objective, _)) => format!(
                "scoreboard objectives remove {}",
                self.objective_name(objective)
            ),
            ScoreboardCommand::ObjectivesModify { objective, change } => {
                let prefix = format!(
                    "scoreboard objectives modify {}",
                    self.objective_name(&objective.0)
                );
                match change {
                    ObjectiveChange::DisplayName(name) => {
                        format!("{prefix} displayname {}", self.component_json(name))
                    }
                    ObjectiveChange::DisplayAutoUpdate(value) => {
                        format!("{prefix} displayautoupdate {value}")
                    }
                    ObjectiveChange::RenderType(value) => {
                        format!("{prefix} rendertype {value}")
                    }
                    ObjectiveChange::NumberFormat(format) => {
                        format!("{prefix} numberformat{}", self.number_format_text(format))
                    }
                }
            }
            ScoreboardCommand::PlayersList(None) => "scoreboard players list".to_owned(),
            ScoreboardCommand::PlayersList(Some(holder)) => {
                format!("scoreboard players list {}", self.component_holder(holder))
            }
            ScoreboardCommand::PlayersResetAll(holder) => {
                format!("scoreboard players reset {}", self.component_holder(holder))
            }
            ScoreboardCommand::PlayersChange {
                target,
                add,
                amount,
            } => format!(
                "scoreboard players {} {} {} {amount}",
                if *add { "add" } else { "remove" },
                self.component_holder(&target.holder),
                self.objective_name(&target.objective),
            ),
            ScoreboardCommand::PlayersDisplayName { target, name } => format!(
                "scoreboard players display name {} {}{}",
                self.component_holder(&target.holder),
                self.objective_name(&target.objective),
                name.as_ref().map_or_else(String::new, |name| format!(
                    " {}",
                    self.component_json(name)
                ))
            ),
            ScoreboardCommand::PlayersNumberFormat { target, format } => format!(
                "scoreboard players display numberformat {} {}{}",
                self.component_holder(&target.holder),
                self.objective_name(&target.objective),
                self.number_format_text(format)
            ),
        }
    }

    fn objective_name(&self, name: &str) -> String {
        user_objective_name(&self.program.namespace, name)
    }

    fn number_format_text(&self, format: &ScoreNumberFormat) -> String {
        match format {
            ScoreNumberFormat::Reset => String::new(),
            ScoreNumberFormat::Blank => " blank".to_owned(),
            ScoreNumberFormat::Fixed(component) => {
                format!(" fixed {}", self.component_json(component))
            }
            ScoreNumberFormat::Styled(style) => format!(
                " styled {}",
                serde_json::from_str::<serde_json::Value>(style).expect("validated style JSON")
            ),
        }
    }
}
