use crate::ast::*;
use crate::diagnostic::Diagnostic;
use crate::lexer::TokenKind;
use crate::parser::Parser;
use crate::parser::keywords::{number_format_kind, render_type};

impl Parser {
    pub(super) fn scoreboard_objectives_command(&mut self) -> Result<StatementKind, Diagnostic> {
        let method = self.command_method()?;
        self.expect(TokenKind::LeftParen, "scoreboard.objectives 方法后需要 `(`")?;
        let command = match method.as_str() {
            "list" => ScoreboardCommand::ObjectivesList,
            "remove" => ScoreboardCommand::ObjectivesRemove(self.ident("计分板目标名称")?),
            "modify_display_name" => {
                let objective = self.ident("计分板目标名称")?;
                self.command_comma()?;
                let name = self.text_component_or_string("目标显示名")?;
                ScoreboardCommand::ObjectivesModify {
                    objective,
                    change: ObjectiveChange::DisplayName(Box::new(name)),
                }
            }
            "modify_displayautoupdate" => {
                let objective = self.ident("计分板目标名称")?;
                self.command_comma()?;
                ScoreboardCommand::ObjectivesModify {
                    objective,
                    change: ObjectiveChange::DisplayAutoUpdate(self.command_boolean()?),
                }
            }
            "modify_rendertype" => {
                let objective = self.ident("计分板目标名称")?;
                self.command_comma()?;
                let (value, span) = self.ident("渲染类型 integer 或 hearts")?;
                let Some(value) = render_type(&value) else {
                    return Err(Diagnostic::new(
                        "渲染类型只能是 integer/整数 或 hearts/爱心",
                        span,
                    ));
                };
                ScoreboardCommand::ObjectivesModify {
                    objective,
                    change: ObjectiveChange::RenderType(value.to_owned()),
                }
            }
            "modify_numberformat" => {
                let objective = self.ident("计分板目标名称")?;
                let format = if self.command_optional_comma() {
                    self.score_number_format()?
                } else {
                    ScoreNumberFormat::Reset
                };
                ScoreboardCommand::ObjectivesModify {
                    objective,
                    change: ObjectiveChange::NumberFormat(format),
                }
            }
            _ => return self.unknown_command_method("scoreboard.objectives", &method),
        };
        self.expect(TokenKind::RightParen, "scoreboard.objectives 调用缺少 `)`")?;
        self.expect(TokenKind::Semicolon, "scoreboard.objectives 调用后需要 `;`")?;
        Ok(StatementKind::ScoreboardCommand(Box::new(command)))
    }

    pub(super) fn scoreboard_players_command(&mut self) -> Result<StatementKind, Diagnostic> {
        let method = self.command_method()?;
        self.expect(TokenKind::LeftParen, "scoreboard.players 方法后需要 `(`")?;
        let command = match method.as_str() {
            "list" => ScoreboardCommand::PlayersList(if self.check(&TokenKind::RightParen) {
                None
            } else {
                Some(self.holder("计分持有者")?)
            }),
            "reset" => ScoreboardCommand::PlayersResetAll(self.holder("计分持有者")?),
            "add" | "remove" => {
                let target = self.score_target_body("计分目标")?;
                self.command_comma()?;
                let amount = self.signed("分数变化量")?;
                ScoreboardCommand::PlayersChange {
                    target,
                    add: method == "add",
                    amount,
                }
            }
            "display_name" => {
                let target = self.score_target_body("计分目标")?;
                let name = if self.command_optional_comma() {
                    Some(Box::new(self.text_component_or_string("分数显示名")?))
                } else {
                    None
                };
                ScoreboardCommand::PlayersDisplayName { target, name }
            }
            "numberformat" | "display_numberformat" => {
                let target = self.score_target_body("计分目标")?;
                let format = if self.command_optional_comma() {
                    self.score_number_format()?
                } else {
                    ScoreNumberFormat::Reset
                };
                ScoreboardCommand::PlayersNumberFormat { target, format }
            }
            _ => return self.unknown_command_method("scoreboard.players", &method),
        };
        self.expect(TokenKind::RightParen, "scoreboard.players 调用缺少 `)`")?;
        self.expect(TokenKind::Semicolon, "scoreboard.players 调用后需要 `;`")?;
        Ok(StatementKind::ScoreboardCommand(Box::new(command)))
    }

    fn score_number_format(&mut self) -> Result<ScoreNumberFormat, Diagnostic> {
        let (kind, span) = self.ident("数字格式 blank、fixed 或 styled")?;
        let Some(kind) = number_format_kind(&kind) else {
            return Err(Diagnostic::new(
                format!("未知数字格式 `{kind}`，可用 blank、fixed 或 styled"),
                span,
            ));
        };
        Ok(match kind {
            "blank" => ScoreNumberFormat::Blank,
            "fixed" => {
                self.expect(TokenKind::LeftParen, "fixed 后需要 `(`")?;
                let component = self.text_component_or_string("固定显示内容")?;
                self.expect(TokenKind::RightParen, "fixed 后需要 `)`")?;
                ScoreNumberFormat::Fixed(Box::new(component))
            }
            _ => {
                self.expect(TokenKind::LeftParen, "styled 后需要 `(`")?;
                let style = self.string("styled 需要 JSON 样式对象")?.0;
                self.expect(TokenKind::RightParen, "styled 后需要 `)`")?;
                ScoreNumberFormat::Styled(style)
            }
        })
    }
}
