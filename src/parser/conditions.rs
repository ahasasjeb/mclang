//! 布尔条件：`||`、`&&`、`!`、predicate 和比较。

use crate::ast::*;
use crate::diagnostic::Diagnostic;
use crate::lexer::TokenKind;

use super::Parser;

impl Parser {
    pub(super) fn condition(&mut self) -> Result<Condition, Diagnostic> {
        self.condition_or()
    }

    fn condition_or(&mut self) -> Result<Condition, Diagnostic> {
        let mut condition = self.condition_and()?;
        while self.take(&TokenKind::OrOr).is_some() {
            let right = self.condition_and()?;
            condition = Condition::Or(Box::new(condition), Box::new(right));
        }
        Ok(condition)
    }

    fn condition_and(&mut self) -> Result<Condition, Diagnostic> {
        let mut condition = self.condition_not()?;
        while self.take(&TokenKind::AndAnd).is_some() {
            let right = self.condition_not()?;
            condition = Condition::And(Box::new(condition), Box::new(right));
        }
        Ok(condition)
    }

    fn condition_not(&mut self) -> Result<Condition, Diagnostic> {
        if self.take(&TokenKind::Bang).is_some() {
            return Ok(Condition::Not(Box::new(self.condition_not()?)));
        }
        if let Some(start) = self.take_word("predicate") {
            self.expect(TokenKind::LeftParen, "predicate 后需要 `(`")?;
            let (name, _) = self.resource_path("predicate 资源名称")?;
            let end = self
                .expect(TokenKind::RightParen, "predicate 资源名称后需要 `)`")?
                .span;
            return Ok(Condition::Predicate {
                name,
                span: start.span.merge(end),
            });
        }
        if let Some(start) = self.take_word_call("block") {
            self.expect(TokenKind::LeftParen, "block 条件后需要 `(`")?;
            let pos = self.block_position("if block 坐标")?;
            self.expect(TokenKind::Comma, "if block 坐标后需要 `,`")?;
            let block = self.block_state_value("if block 方块谓词")?;
            let end = self
                .expect(TokenKind::RightParen, "block 条件缺少 `)`")?
                .span;
            return Ok(Condition::Block {
                pos,
                block,
                span: start.span.merge(end),
            });
        }
        if let Some(start) = self.take_word_call("blocks") {
            self.expect(TokenKind::LeftParen, "blocks 条件后需要 `(`")?;
            let first = self.block_position("if blocks 起点")?;
            self.expect(TokenKind::Comma, "if blocks 起点后需要 `,`")?;
            let second = self.block_position("if blocks 终点")?;
            self.expect(TokenKind::Comma, "if blocks 终点后需要 `,`")?;
            let destination = self.block_position("if blocks 目标位置")?;
            let masked = if self.take(&TokenKind::Comma).is_some() {
                let (mode, mode_span) = self.ident("if blocks 模式 all 或 masked")?;
                match mode.as_str() {
                    "masked" | "遮罩" => true,
                    "all" | "全部" => false,
                    _ => {
                        return Err(Diagnostic::new(
                            format!("if blocks 模式只能是 all 或 masked，实际为 `{mode}`"),
                            mode_span,
                        ));
                    }
                }
            } else {
                false
            };
            let end = self
                .expect(TokenKind::RightParen, "blocks 条件缺少 `)`")?
                .span;
            return Ok(Condition::Blocks {
                start: first,
                end: second,
                destination,
                masked,
                span: start.span.merge(end),
            });
        }
        if let Some(start) = self.take_word_call("biome") {
            self.expect(TokenKind::LeftParen, "biome 条件后需要 `(`")?;
            let pos = self.block_position("if biome 坐标")?;
            self.expect(TokenKind::Comma, "if biome 坐标后需要 `,`")?;
            let (biome, biome_span) = self.string("if biome 需要生物群系或 #标签字符串")?;
            let end = self
                .expect(TokenKind::RightParen, "biome 条件缺少 `)`")?
                .span;
            return Ok(Condition::Biome {
                pos,
                biome,
                biome_span,
                span: start.span.merge(end),
            });
        }
        if let Some(start) = self.take_word_call("loaded") {
            self.expect(TokenKind::LeftParen, "loaded 条件后需要 `(`")?;
            let pos = self.block_position("if loaded 坐标")?;
            let end = self
                .expect(TokenKind::RightParen, "loaded 条件缺少 `)`")?
                .span;
            return Ok(Condition::Loaded {
                pos,
                span: start.span.merge(end),
            });
        }
        if let Some(start) = self.take_word_call("dimension") {
            self.expect(TokenKind::LeftParen, "dimension 条件后需要 `(`")?;
            let (dimension, dimension_span) = self.string("if dimension 需要维度资源位置字符串")?;
            let end = self
                .expect(TokenKind::RightParen, "dimension 条件缺少 `)`")?
                .span;
            return Ok(Condition::Dimension {
                dimension,
                dimension_span,
                span: start.span.merge(end),
            });
        }
        if let Some(start) = self.take_word_call("entity") {
            self.expect(TokenKind::LeftParen, "entity 条件后需要 `(`")?;
            let (query, query_span) = self.ident("if entity 需要实体查询名称")?;
            let end = self
                .expect(TokenKind::RightParen, "entity 条件缺少 `)`")?
                .span;
            return Ok(Condition::Entity {
                query,
                query_span,
                span: start.span.merge(end),
            });
        }
        if let Some(start) = self.take_word_call("data") {
            self.expect(TokenKind::LeftParen, "data 条件后需要 `(`")?;
            let source = self.nbt_source_value("if data 来源")?;
            self.expect(TokenKind::Comma, "if data 来源后需要 `,`")?;
            let (path, path_span) = self.string("if data 需要 NBT 路径字符串")?;
            let end = self
                .expect(TokenKind::RightParen, "data 条件缺少 `)`")?
                .span;
            return Ok(Condition::Data {
                source,
                path,
                path_span,
                span: start.span.merge(end),
            });
        }
        if let Some(start) = self.take_word_call("items") {
            self.expect(TokenKind::LeftParen, "items 条件后需要 `(`")?;
            let source = self.item_condition_source("if items 来源")?;
            self.expect(TokenKind::Comma, "物品条件来源后需要 `,`")?;
            let (slots, slots_span) = self.string("if items 需要槽位字符串")?;
            self.expect(TokenKind::Comma, "if items 槽位后需要 `,`")?;
            let item = self.item_predicate()?;
            let end = self
                .expect(TokenKind::RightParen, "items 条件缺少 `)`")?
                .span;
            return Ok(Condition::Items {
                source,
                slots,
                slots_span,
                item,
                span: start.span.merge(end),
            });
        }
        if let Some(start) = self.take_word_call("slots") {
            self.expect(TokenKind::LeftParen, "slots 条件后需要 `(`")?;
            let source = self.item_condition_source("if slots 来源")?;
            self.expect(TokenKind::Comma, "物品条件来源后需要 `,`")?;
            let (slots, slots_span) = self.string("if slots 需要槽位字符串")?;
            let end = self
                .expect(TokenKind::RightParen, "slots 条件缺少 `)`")?
                .span;
            return Ok(Condition::Slots {
                source,
                slots,
                slots_span,
                span: start.span.merge(end),
            });
        }
        if let Some(start) = self.take_word_call("function") {
            self.expect(TokenKind::LeftParen, "function 条件后需要 `(`")?;
            let target = self.call_target("if function 函数名称或 #标签")?;
            let end = self
                .expect(TokenKind::RightParen, "function 条件缺少 `)`")?
                .span;
            return Ok(Condition::Function {
                target,
                span: start.span.merge(end),
            });
        }
        if let Some(start) = self.take_word_call("stopwatch") {
            self.expect(TokenKind::LeftParen, "stopwatch 条件后需要 `(`")?;
            let (id, _) = self.string("if stopwatch 需要秒表资源位置字符串")?;
            let end = self
                .expect(TokenKind::RightParen, "stopwatch 条件缺少 `)`")?
                .span;
            return Ok(Condition::Stopwatch {
                id,
                span: start.span.merge(end),
            });
        }
        if self.condition_group_starts() {
            self.expect(TokenKind::LeftParen, "这里需要 `(`")?;
            let condition = self.condition()?;
            self.expect(TokenKind::RightParen, "条件分组缺少 `)`")?;
            return Ok(condition);
        }
        self.comparison_condition()
    }

    /// 条件关键字：只有紧跟 `(` 时才按条件解析；`stopwatch.query(...)`、
    /// `data.get(...)` 这类属于表达式，交给比较条件。
    fn take_word_call(&mut self, word: &str) -> Option<crate::lexer::Token> {
        if self.check_word(word) && matches!(self.peek_kind(1).kind, TokenKind::LeftParen) {
            Some(self.advance().clone())
        } else {
            None
        }
    }

    /// 判断 `(` 是条件分组还是带括号的算术表达式。
    ///
    /// 扫描到配对的 `)` 后，若其后的记号是算术运算符，说明这是比较条件的左操作数。
    fn condition_group_starts(&self) -> bool {
        if !self.check(&TokenKind::LeftParen) {
            return false;
        }
        let mut depth = 0_usize;
        for index in self.cursor..self.tokens.len() {
            match self.tokens[index].kind {
                TokenKind::LeftParen => depth += 1,
                TokenKind::RightParen => {
                    depth -= 1;
                    if depth == 0 {
                        let Some(next) = self.tokens.get(index + 1) else {
                            return true;
                        };
                        return !matches!(
                            &next.kind,
                            TokenKind::Plus
                                | TokenKind::Minus
                                | TokenKind::Star
                                | TokenKind::Slash
                                | TokenKind::Percent
                                | TokenKind::EqualEqual
                                | TokenKind::BangEqual
                                | TokenKind::Less
                                | TokenKind::LessEqual
                                | TokenKind::Greater
                                | TokenKind::GreaterEqual
                        );
                    }
                }
                TokenKind::Eof => return true,
                _ => {}
            }
        }
        true
    }

    fn comparison_condition(&mut self) -> Result<Condition, Diagnostic> {
        let left = self.expression()?;
        let token = self.advance().clone();
        let comparison = match token.kind {
            TokenKind::EqualEqual => Comparison::Equal,
            TokenKind::BangEqual => Comparison::NotEqual,
            TokenKind::Less => Comparison::Less,
            TokenKind::LessEqual => Comparison::LessEqual,
            TokenKind::Greater => Comparison::Greater,
            TokenKind::GreaterEqual => Comparison::GreaterEqual,
            _ => {
                return Err(Diagnostic::new(
                    "条件需要 ==、!=、<、<=、> 或 >=",
                    token.span,
                ));
            }
        };
        let right = self.expression()?;
        Ok(Condition::Compare {
            left: Box::new(left),
            comparison,
            right: Box::new(right),
        })
    }
}
