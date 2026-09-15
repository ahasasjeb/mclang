//! 函数体语句：控制流、结构化实体操作、调用、赋值与返回。

use crate::ast::*;
use crate::diagnostic::Diagnostic;
use crate::lexer::TokenKind;

use super::Parser;
use super::keywords::{
    boolean_word, message_target, self_method, sound_source, text_color, time_unit, word_matches,
};

impl Parser {
    pub(super) fn block(&mut self) -> Result<(Vec<Statement>, Span), Diagnostic> {
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

    fn statement(&mut self) -> Result<Statement, Diagnostic> {
        let start = self.current().span;
        let kind = if self.take_word("each").is_some() {
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
            self.expect(TokenKind::RightParen, "实体类型后需要 `)`")?;
            let (body, _) = self.block()?;
            StatementKind::Spawn { entity_type, body }
        } else if self.take_word("self").is_some() {
            StatementKind::SelfAction(self.self_action()?)
        } else if self.take_word("message").is_some() {
            self.message_statement()?
        } else if self.take_word("sound").is_some() {
            self.sound_statement()?
        } else if self.take_word("run").is_some() {
            let (command, span) = self.string("run 后需要命令字符串")?;
            if command.trim().is_empty() {
                return Err(Diagnostic::new("run 命令不能为空", span));
            }
            if command.starts_with('/') {
                return Err(Diagnostic::new(
                    "Minecraft 函数中的命令不能以 `/` 开头",
                    span,
                ));
            }
            if command.contains(['\n', '\r']) {
                return Err(Diagnostic::new("一条 run 语句只能包含一行命令", span));
            }
            self.expect(TokenKind::Semicolon, "命令后需要 `;`")?;
            StatementKind::Run(command)
        } else if self.take_word("call").is_some() {
            let (name, _) = self.ident("被调用函数名称")?;
            let arguments = self.call_arguments()?;
            self.expect(TokenKind::Semicolon, "函数调用后需要 `;`")?;
            StatementKind::Call {
                function: name,
                arguments,
            }
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
        } else if self.take_word("execute").is_some() {
            let (clauses, span) = self.string("execute 后需要子句字符串")?;
            if clauses.trim().is_empty()
                || clauses.contains(['\n', '\r'])
                || clauses.starts_with("execute ")
                || clauses.ends_with(" run")
            {
                return Err(Diagnostic::new(
                    "execute 字符串应只包含子句，例如 `as @a at @s`",
                    span,
                ));
            }
            let (body, _) = self.block()?;
            StatementKind::Execute { clauses, body }
        } else if self.take_word("return").is_some() {
            let value = if self.take(&TokenKind::Semicolon).is_some() {
                None
            } else {
                let value = self.expression()?;
                self.expect(TokenKind::Semicolon, "return 表达式后需要 `;`")?;
                Some(value)
            };
            StatementKind::Return(value)
        } else if self.take_word("let").is_some() {
            let (name, _) = self.ident("局部变量名称")?;
            self.expect(TokenKind::Equal, "局部变量需要初始值")?;
            let value = self.expression()?;
            self.expect(TokenKind::Semicolon, "局部变量声明后需要 `;`")?;
            StatementKind::Let { name, value }
        } else {
            let (name, _) = self.ident("语句")?;
            if self.check(&TokenKind::LeftParen) {
                let arguments = self.call_arguments()?;
                self.expect(TokenKind::Semicolon, "函数调用后需要 `;`")?;
                StatementKind::Call {
                    function: name,
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

    fn give_statement(&mut self) -> Result<StatementKind, Diagnostic> {
        self.expect(TokenKind::LeftParen, "give 后需要 `(`")?;
        let (target, _) = self.ident("give 需要玩家查询名称")?;
        self.expect(TokenKind::Comma, "查询名称后需要 `,`")?;
        let (item, _) = self.ident("give 需要物品定义名称")?;
        let (count, count_span) = self.optional_count("give 数量")?;
        self.expect(TokenKind::RightParen, "give 调用缺少 `)`")?;
        self.expect(TokenKind::Semicolon, "give 调用后需要 `;`")?;
        Ok(StatementKind::Give {
            target,
            item,
            count,
            count_span,
        })
    }

    fn optional_count(&mut self, name: &str) -> Result<(Option<u32>, Option<Span>), Diagnostic> {
        if self.take(&TokenKind::Comma).is_none() {
            return Ok((None, None));
        }
        let (count, span) = self.unsigned_with_span(name)?;
        Ok((Some(count), Some(span)))
    }

    fn self_action(&mut self) -> Result<SelfAction, Diagnostic> {
        self.expect(TokenKind::Dot, "self 后需要 `.`")?;
        let (method, span) = self.ident("self 方法名称")?;
        let Some(method_kind) = self_method(&method) else {
            return Err(Diagnostic::new(format!("未知 self 方法 `{method}`"), span));
        };
        self.expect(TokenKind::LeftParen, "self 方法后需要 `(`")?;
        let action = match method_kind {
            "add_tag" | "remove_tag" => {
                let (tag, _) = self.string("标签需要字符串")?;
                if method_kind == "add_tag" {
                    SelfAction::AddTag(tag)
                } else {
                    SelfAction::RemoveTag(tag)
                }
            }
            "set_invulnerable" => {
                let (value, value_span) = self.ident("true 或 false")?;
                let value = match boolean_word(&value) {
                    Some("true") => true,
                    Some("false") => false,
                    _ => return Err(Diagnostic::new("这里需要 true 或 false", value_span)),
                };
                SelfAction::SetInvulnerable(value)
            }
            "save_items" | "restore_items" | "remove_preserving_items" => {
                let (reference, _) = self.ident("物品存储名称")?;
                match method_kind {
                    "save_items" => SelfAction::SaveItems(reference),
                    "restore_items" => SelfAction::RestoreItems(reference),
                    "remove_preserving_items" => SelfAction::RemovePreservingItems(reference),
                    _ => unreachable!(),
                }
            }
            "give_item" => {
                let (item, _) = self.ident("物品定义名称")?;
                let (count, count_span) = self.optional_count("给予物品数量")?;
                SelfAction::GiveItem {
                    item,
                    count,
                    count_span,
                }
            }
            "clear_items" => SelfAction::ClearItems,
            "remove" => SelfAction::Remove,
            "consume" => SelfAction::Consume,
            "return_to_owner" => SelfAction::ReturnToOwner,
            _ => unreachable!(),
        };
        self.expect(TokenKind::RightParen, "self 方法缺少 `)`")?;
        self.expect(TokenKind::Semicolon, "self 方法调用后需要 `;`")?;
        Ok(action)
    }

    fn message_statement(&mut self) -> Result<StatementKind, Diagnostic> {
        self.expect(TokenKind::Dot, "message 后需要 `.`")?;
        let (method, span) = self.ident("消息目标")?;
        let Some(method_kind) = message_target(&method) else {
            return Err(Diagnostic::new("消息目标只能是 all、self 或 nearest", span));
        };
        self.expect(TokenKind::LeftParen, "消息目标后需要 `(`")?;
        let (target, text) = match method_kind {
            "all" | "self" => {
                let (text, _) = self.string("消息需要文本字符串")?;
                let target = if method_kind == "all" {
                    MessageTarget::All
                } else {
                    MessageTarget::SelfEntity
                };
                (target, text)
            }
            "nearest" => {
                let within = self.unsigned("message.nearest 范围")?;
                self.expect(TokenKind::Comma, "范围后需要 `,`")?;
                let (text, _) = self.string("message.nearest 需要文本字符串")?;
                (MessageTarget::Nearest { within }, text)
            }
            _ => unreachable!(),
        };
        let color = if self.take(&TokenKind::Comma).is_some() {
            let (color, _) = self.ident("消息颜色")?;
            Some(text_color(&color).unwrap_or(&color).to_owned())
        } else {
            None
        };
        self.expect(TokenKind::RightParen, "消息调用缺少 `)`")?;
        self.expect(TokenKind::Semicolon, "消息调用后需要 `;`")?;
        Ok(StatementKind::Message {
            target,
            text,
            color,
        })
    }

    fn sound_statement(&mut self) -> Result<StatementKind, Diagnostic> {
        self.expect(TokenKind::Dot, "sound 后需要 `.`")?;
        let (target, span) = self.ident("声音目标")?;
        if !word_matches(&target, "self") {
            return Err(Diagnostic::new("声音目标目前只能是 self", span));
        }
        self.expect(TokenKind::LeftParen, "sound.self 后需要 `(`")?;
        let (sound, _) = self.string("sound.self 需要声音资源位置")?;
        self.expect(TokenKind::Comma, "声音资源位置后需要 `,`")?;
        let (source, _) = self.ident("声音分类")?;
        let source = sound_source(&source).unwrap_or(&source).to_owned();
        self.expect(TokenKind::RightParen, "声音调用缺少 `)`")?;
        self.expect(TokenKind::Semicolon, "声音调用后需要 `;`")?;
        Ok(StatementKind::PlaySound { sound, source })
    }

    fn schedule_statement(&mut self) -> Result<StatementKind, Diagnostic> {
        let (function, _) = self.ident("被调度函数名称")?;
        self.empty_arguments()?;
        self.expect_word("after")?;
        let number_token = self.advance().clone();
        let TokenKind::Number(number) = number_token.kind else {
            return Err(Diagnostic::new("调度延迟需要正整数", number_token.span));
        };
        if !(1..=i64::from(i32::MAX)).contains(&number) {
            return Err(Diagnostic::new(
                "调度延迟必须是 1 到 2147483647 之间的整数",
                number_token.span,
            ));
        }
        let (unit, unit_span) = self.ident("时间单位 t、s 或 d")?;
        let Some(unit) = time_unit(&unit) else {
            return Err(Diagnostic::new("时间单位只能是 t、s 或 d", unit_span));
        };
        let mode = if self.take_word("append").is_some() {
            ScheduleMode::Append
        } else {
            self.take_word("replace");
            ScheduleMode::Replace
        };
        self.expect(TokenKind::Semicolon, "调度语句后需要 `;`")?;
        Ok(StatementKind::Schedule {
            function,
            delay: format!("{number}{unit}"),
            mode,
        })
    }

    fn assignment_operator(&mut self) -> Result<AssignOp, Diagnostic> {
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

    fn empty_arguments(&mut self) -> Result<(), Diagnostic> {
        self.expect(TokenKind::LeftParen, "函数名称后需要 `(`")?;
        self.expect(TokenKind::RightParen, "调度函数暂不支持参数，需要 `)`")?;
        Ok(())
    }
}
