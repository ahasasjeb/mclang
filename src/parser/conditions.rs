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
        if self.condition_group_starts() {
            self.expect(TokenKind::LeftParen, "这里需要 `(`")?;
            let condition = self.condition()?;
            self.expect(TokenKind::RightParen, "条件分组缺少 `)`")?;
            return Ok(condition);
        }
        self.comparison_condition()
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
            left,
            comparison,
            right,
        })
    }
}
