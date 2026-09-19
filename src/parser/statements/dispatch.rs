use crate::ast::*;
use crate::diagnostic::Diagnostic;
use crate::lexer::TokenKind;

use crate::parser::Parser;

impl Parser {
    pub(in crate::parser) fn block(&mut self) -> Result<(Vec<Statement>, Span), Diagnostic> {
        self.expect(TokenKind::LeftBrace, "这里需要 `{`")?;
        let mut statements = Vec::new();
        while !self.check(&TokenKind::RightBrace) {
            if self.check(&TokenKind::Eof) {
                return Err(Diagnostic::new("代码块缺少 `}`", self.current().span));
            }
            statements.push(self.statement()?);
        }
        let end = self.advance().span;
        Ok((statements, end))
    }

    pub(super) fn statement(&mut self) -> Result<Statement, Diagnostic> {
        let start = self.current().span;
        let kind = if let Some(root) = self.core_command_root() {
            self.advance();
            let command = self.core_command(root)?;
            self.expect(TokenKind::Semicolon, "命令调用后需要 `;`")?;
            StatementKind::CoreCommand(Box::new(command))
        } else if let Some(root) = self.entity_command_root() {
            self.advance();
            let command = self.entity_command(root)?;
            self.expect(TokenKind::Semicolon, "命令调用后需要 `;`")?;
            StatementKind::EntityCommand(Box::new(command))
        } else if self.take_word("each").is_some() {
            self.expect(TokenKind::LeftParen, "each 后需要 `(`")?;
            let (query, _) = self.ident("实体查询名称")?;
            self.expect(TokenKind::RightParen, "查询名称后需要 `)`")?;
            let (body, _) = self.block()?;
            StatementKind::Each { query, body }
        } else if self.take_word("give").is_some() {
            self.give_statement()?
        } else if self.take_word("in_dimension").is_some() {
            self.expect(TokenKind::LeftParen, "in_dimension 后需要 `(`")?;
            let (dimension, _) = self.string("in_dimension 需要维度资源位置")?;
            self.expect(TokenKind::RightParen, "维度资源位置后需要 `)`")?;
            let (body, _) = self.block()?;
            StatementKind::InDimension { dimension, body }
        } else if self.take_word("spawn").is_some() {
            self.expect(TokenKind::LeftParen, "spawn 后需要 `(`")?;
            let (entity_type, _) = self.string("spawn 需要实体类型资源位置")?;
            let position = if self.take(&TokenKind::Comma).is_some() {
                Some(self.position_value("spawn 召唤坐标")?)
            } else {
                None
            };
            self.expect(TokenKind::RightParen, "实体类型后需要 `)`")?;
            let (body, _) = self.block()?;
            StatementKind::Spawn {
                entity_type,
                position,
                body,
            }
        } else if self.take_word("self").is_some() {
            StatementKind::SelfAction(self.self_action()?)
        } else if self.take_word("message").is_some() {
            self.message_statement()?
        } else if self.take_word("sound").is_some() {
            self.sound_statement()?
        } else if self.take_word("run").is_some() {
            let (command, _) = self.command_string("run")?;
            self.expect(TokenKind::Semicolon, "命令后需要 `;`")?;
            StatementKind::Run(command)
        } else if self.take_word("call").is_some() {
            self.function_call_statement()?
        } else if self.take_word("schedule").is_some() {
            self.schedule_statement()?
        } else if self.take_word("if").is_some() {
            let condition = self.condition()?;
            let (then_body, _) = self.block()?;
            let else_body = if self.take_word("else").is_some() {
                self.block()?.0
            } else {
                Vec::new()
            };
            StatementKind::If {
                condition,
                then_body,
                else_body,
            }
        } else if self.take_word("while").is_some() {
            let condition = self.condition()?;
            let (body, _) = self.block()?;
            StatementKind::While { condition, body }
        } else if self.take_word("for").is_some() {
            let (variable, variable_span) = self.ident("循环变量名称")?;
            self.expect_word("in")?;
            let start = self.expression()?;
            self.expect(TokenKind::DotDot, "for 区间需要 `..`")?;
            let end = self.expression()?;
            let (body, _) = self.block()?;
            StatementKind::For {
                variable,
                variable_span,
                start,
                end,
                body,
            }
        } else if self.take_word("break").is_some() {
            self.expect(TokenKind::Semicolon, "break 后需要 `;`")?;
            StatementKind::Break
        } else if self.take_word("continue").is_some() {
            self.expect(TokenKind::Semicolon, "continue 后需要 `;`")?;
            StatementKind::Continue
        } else if self.take_word("execute").is_some() {
            self.execute_statement()?
        } else if self.take_word("return").is_some() {
            let kind = if self.take(&TokenKind::Semicolon).is_some() {
                ReturnKind::Void
            } else if self.take_word("fail").is_some() {
                self.expect(TokenKind::Semicolon, "return fail 后需要 `;`")?;
                ReturnKind::Fail
            } else if self.take_word("run").is_some() {
                let (command, _) = self.command_string("return run")?;
                self.expect(TokenKind::Semicolon, "return run 后需要 `;`")?;
                ReturnKind::Run(command)
            } else {
                let value = self.expression()?;
                self.expect(TokenKind::Semicolon, "return 表达式后需要 `;`")?;
                ReturnKind::Value(value)
            };
            StatementKind::Return(kind)
        } else if self.take_word("let").is_some() {
            let (name, name_span) = self.ident("局部变量名称")?;
            self.expect(TokenKind::Equal, "局部变量需要初始值")?;
            let value = self.expression()?;
            self.expect(TokenKind::Semicolon, "局部变量声明后需要 `;`")?;
            StatementKind::Let {
                name,
                name_span,
                value,
            }
        } else if self.take_word("effect").is_some() {
            self.effect_statement()?
        } else if self.take_word("xp").is_some() {
            self.xp_statement()?
        } else if self.take_word("clear").is_some() {
            self.clear_statement()?
        } else if self.take_word("stopwatch").is_some() {
            self.stopwatch_statement()?
        } else if self.take_word("scoreboard").is_some() {
            self.scoreboard_statement()?
        } else if self.take_word("teleport").is_some() {
            self.teleport_statement()?
        } else if self.take_word("set_block").is_some() {
            self.set_block_statement()?
        } else if self.take_word("fill_biome").is_some() {
            self.fill_biome_statement()?
        } else if self.take_word("fill").is_some() {
            self.fill_statement()?
        } else if self.take_word("clone").is_some() {
            self.clone_statement()?
        } else if self.take_word("place").is_some() {
            self.place_statement()?
        } else if self.take_word("forceload").is_some() {
            self.forceload_statement()?
        } else if self.take_word("time").is_some() {
            self.time_statement()?
        } else if self.take_word("weather").is_some() {
            self.weather_statement()?
        } else if self.take_word("gamerule").is_some() {
            self.gamerule_statement()?
        } else if self.take_word("worldborder").is_some() {
            self.worldborder_statement()?
        } else if self.take_word("locate").is_some() {
            self.locate_statement()?
        } else if self.take_word("advancement").is_some() {
            self.advancement_statement()?
        } else if self.check_word("data") && matches!(self.peek_kind(1).kind, TokenKind::Dot) {
            self.take_word("data");
            self.data_statement()?
        } else if self.check_word("item") && matches!(self.peek_kind(1).kind, TokenKind::Dot) {
            self.take_word("item");
            self.item_statement()?
        } else if self.check_word("nbt") {
            let nbt = self.nbt_compound_with_aliases("nbt 语句")?;
            // 块风格语句，结尾分号可选；物品属性里的 `custom_data = nbt {...};` 仍需要分号。
            self.take(&TokenKind::Semicolon);
            StatementKind::NbtMerge { nbt }
        } else {
            let (name, _) = self.ident("语句")?;
            if self.check(&TokenKind::LeftParen) {
                let arguments = self.call_arguments()?;
                self.expect(TokenKind::Semicolon, "函数调用后需要 `;`")?;
                StatementKind::Call {
                    target: CallTarget::Function(name),
                    arguments,
                }
            } else {
                let operation = self.assignment_operator()?;
                let value = self.expression()?;
                self.expect(TokenKind::Semicolon, "赋值后需要 `;`")?;
                StatementKind::Assign {
                    target: name,
                    operation,
                    value,
                }
            }
        };
        let end = self.previous().span;
        Ok(Statement {
            kind,
            span: start.merge(end),
        })
    }
    /// 读取一条底层命令字符串，执行与 `run` 相同的空值、斜杠和换行检查。
    pub(super) fn command_string(&mut self, label: &str) -> Result<(String, Span), Diagnostic> {
        let (command, span) = self.string(&format!("{label} 后需要命令字符串"))?;
        if command.trim().is_empty() {
            return Err(Diagnostic::new(format!("{label} 命令不能为空"), span));
        }
        if command.starts_with('/') {
            return Err(Diagnostic::new(
                "Minecraft 函数中的命令不能以 `/` 开头",
                span,
            ));
        }
        if command.contains(['\n', '\r']) {
            return Err(Diagnostic::new(
                format!("一条 {label} 语句只能包含一行命令"),
                span,
            ));
        }
        Ok((command, span))
    }

    pub(super) fn assignment_operator(&mut self) -> Result<AssignOp, Diagnostic> {
        let token = self.advance().clone();
        match token.kind {
            TokenKind::Equal => Ok(AssignOp::Set),
            TokenKind::PlusEqual => Ok(AssignOp::Add),
            TokenKind::MinusEqual => Ok(AssignOp::Subtract),
            TokenKind::StarEqual => Ok(AssignOp::Multiply),
            TokenKind::SlashEqual => Ok(AssignOp::Divide),
            TokenKind::PercentEqual => Ok(AssignOp::Modulo),
            _ => Err(Diagnostic::new(
                "这里需要赋值运算符 =、+=、-=、*=、/= 或 %=",
                token.span,
            )),
        }
    }

    pub(super) fn empty_arguments(&mut self) -> Result<(), Diagnostic> {
        self.expect(TokenKind::LeftParen, "函数名称后需要 `(`")?;
        self.expect(TokenKind::RightParen, "调度函数暂不支持参数，需要 `)`")?;
        Ok(())
    }
}
