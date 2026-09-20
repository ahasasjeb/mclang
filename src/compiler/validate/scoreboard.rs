use super::{
    components::validate_component,
    statements::{ValidationContext, validate_holder, validate_score_target},
};
use crate::{ast::*, diagnostic::Diagnostic};

pub(super) fn validate_scoreboard_command(
    command: &ScoreboardCommand,
    span: Span,
    ctx: ValidationContext<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match command {
        ScoreboardCommand::ObjectivesList | ScoreboardCommand::PlayersList(None) => {}
        ScoreboardCommand::ObjectivesRemove(objective) => {
            validate_objective(objective, ctx, diagnostics);
        }
        ScoreboardCommand::ObjectivesModify { objective, change } => {
            validate_objective(objective, ctx, diagnostics);
            match change {
                ObjectiveChange::DisplayName(component) => {
                    validate_component(component, ctx, diagnostics);
                }
                ObjectiveChange::NumberFormat(format) => {
                    validate_number_format(format, span, ctx, diagnostics);
                }
                ObjectiveChange::DisplayAutoUpdate(_) | ObjectiveChange::RenderType(_) => {}
            }
        }
        ScoreboardCommand::PlayersList(Some(holder)) => {
            super::entity_commands::entity_target(holder, true, false, span, ctx, diagnostics);
        }
        ScoreboardCommand::PlayersResetAll(holder) => {
            validate_holder(holder, span, ctx, diagnostics);
        }
        ScoreboardCommand::PlayersChange { target, amount, .. } => {
            validate_score_target(target, span, ctx, diagnostics);
            if *amount < 0 {
                diagnostics.push(Diagnostic::new(
                    format!("scoreboard.players.add/remove 的变化量 `{amount}` 不能小于 0"),
                    span,
                ));
            }
        }
        ScoreboardCommand::PlayersDisplayName { target, name } => {
            validate_score_target(target, span, ctx, diagnostics);
            if let Some(name) = name {
                validate_component(name, ctx, diagnostics);
            }
        }
        ScoreboardCommand::PlayersNumberFormat { target, format } => {
            validate_score_target(target, span, ctx, diagnostics);
            validate_number_format(format, span, ctx, diagnostics);
        }
    }
}

fn validate_objective(
    objective: &(String, Span),
    ctx: ValidationContext<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !ctx.symbols.objectives.contains(objective.0.as_str()) {
        diagnostics.push(Diagnostic::new(
            format!(
                "找不到计分板目标 `{}`；先声明 `objective {};`",
                objective.0, objective.0
            ),
            objective.1,
        ));
    }
}

fn validate_number_format(
    format: &ScoreNumberFormat,
    span: Span,
    ctx: ValidationContext<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match format {
        ScoreNumberFormat::Fixed(component) => validate_component(component, ctx, diagnostics),
        ScoreNumberFormat::Styled(style) => {
            if !serde_json::from_str::<serde_json::Value>(style)
                .is_ok_and(|value| value.is_object())
            {
                diagnostics.push(Diagnostic::new(
                    "styled 需要 JSON 样式对象，例如 `\"{\\\"color\\\":\\\"red\\\"}\"`",
                    span,
                ));
            }
        }
        ScoreNumberFormat::Reset | ScoreNumberFormat::Blank => {}
    }
}
