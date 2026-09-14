//! 算术表达式、括号分组和实参列表。

use crate::ast::*;
use crate::diagnostic::Diagnostic;
use crate::lexer::TokenKind;

use super::Parser;

impl Parser {
    pub(super) fn expression(&mut self) -> Result<Expr, Diagnostic> {
        self.additive()
    }

    fn additive(&mut self) -> Result<Expr, Diagnostic> {
        let mut expression = self.multiplicative()?;
        loop {
            let operation = if self.take(&TokenKind::Plus).is_some() {
                Some(BinaryOp::Add)
            } else if self.take(&TokenKind::Minus).is_some() {
                Some(BinaryOp::Subtract)
            } else {
                None
            };
            let Some(operation) = operation else { break };
            let right = self.multiplicative()?;
            let span = expression.span.merge(right.span);
            expression = Expr {
                kind: ExprKind::Binary {
                    left: Box::new(expression),
                    operation,
                    right: Box::new(right),
                },
                span,
            };
        }
        Ok(expression)
    }

    fn multiplicative(&mut self) -> Result<Expr, Diagnostic> {
        let mut expression = self.unary()?;
        loop {
            let operation = if self.take(&TokenKind::Star).is_some() {
                Some(BinaryOp::Multiply)
            } else if self.take(&TokenKind::Slash).is_some() {
                Some(BinaryOp::Divide)
            } else if self.take(&TokenKind::Percent).is_some() {
                Some(BinaryOp::Modulo)
            } else {
                None
            };
            let Some(operation) = operation else { break };
            let right = self.unary()?;
            let span = expression.span.merge(right.span);
            expression = Expr {
                kind: ExprKind::Binary {
                    left: Box::new(expression),
                    operation,
                    right: Box::new(right),
                },
                span,
            };
        }
        Ok(expression)
    }

    fn unary(&mut self) -> Result<Expr, Diagnostic> {
        if let Some(minus) = self.take(&TokenKind::Minus) {
            if let TokenKind::Number(number) = &self.current().kind {
                let number = *number;
                let number_span = self.advance().span;
                let signed = number
                    .checked_neg()
                    .and_then(|value| i32::try_from(value).ok())
                    .ok_or_else(|| Diagnostic::new("整数超出 32 位范围", number_span))?;
                return Ok(Expr {
                    kind: ExprKind::Integer(signed),
                    span: minus.span.merge(number_span),
                });
            }
            let value = self.unary()?;
            let span = minus.span.merge(value.span);
            return Ok(Expr {
                kind: ExprKind::Negate(Box::new(value)),
                span,
            });
        }
        self.primary()
    }

    fn primary(&mut self) -> Result<Expr, Diagnostic> {
        let token = self.advance().clone();
        match token.kind {
            TokenKind::Number(number) => {
                let value = i32::try_from(number)
                    .map_err(|_| Diagnostic::new("整数超出 32 位范围", token.span))?;
                Ok(Expr {
                    kind: ExprKind::Integer(value),
                    span: token.span,
                })
            }
            TokenKind::Ident(name) => {
                if self.check(&TokenKind::LeftParen) {
                    let arguments = self.call_arguments()?;
                    let span = token.span.merge(self.previous().span);
                    Ok(Expr {
                        kind: ExprKind::Call {
                            function: name,
                            arguments,
                        },
                        span,
                    })
                } else {
                    Ok(Expr {
                        kind: ExprKind::Score(name),
                        span: token.span,
                    })
                }
            }
            TokenKind::LeftParen => {
                let expression = self.expression()?;
                let end = self.expect(TokenKind::RightParen, "表达式缺少 `)`")?.span;
                Ok(Expr {
                    span: token.span.merge(end),
                    ..expression
                })
            }
            _ => Err(Diagnostic::new(
                "这里需要整数、计分变量或括号表达式",
                token.span,
            )),
        }
    }

    pub(super) fn call_arguments(&mut self) -> Result<Vec<Expr>, Diagnostic> {
        self.expect(TokenKind::LeftParen, "函数名称后需要 `(`")?;
        let mut arguments = Vec::new();
        if !self.check(&TokenKind::RightParen) {
            loop {
                arguments.push(self.expression()?);
                if self.take(&TokenKind::Comma).is_none() {
                    break;
                }
            }
        }
        self.expect(TokenKind::RightParen, "实参列表缺少 `)`")?;
        Ok(arguments)
    }
}
