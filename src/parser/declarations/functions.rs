use crate::ast::*;
use crate::diagnostic::Diagnostic;
use crate::lexer::TokenKind;

use crate::parser::Parser;
use crate::parser::keywords::attribute_word;

impl Parser {
    pub(in crate::parser) fn function(&mut self) -> Result<Function, Diagnostic> {
        let mut attributes = Vec::new();
        let start = self.current().span;
        while self.take(&TokenKind::At).is_some() {
            let (attribute, span) = self.ident("属性名称")?;
            let Some(attribute) = attribute_word(&attribute) else {
                return Err(Diagnostic::new(
                    format!("未知函数属性 `@{attribute}`"),
                    span,
                ));
            };
            attributes.push(attribute);
        }
        self.expect_word("fn")?;
        let (name, name_span) = self.ident("函数名称")?;
        self.expect(TokenKind::LeftParen, "函数名称后需要 `(`")?;
        let mut parameters = Vec::new();
        if !self.check(&TokenKind::RightParen) {
            loop {
                let (name, span) = self.ident("参数名称")?;
                parameters.push(Parameter { name, span });
                if self.take(&TokenKind::Comma).is_none() {
                    break;
                }
            }
        }
        self.expect(TokenKind::RightParen, "参数列表缺少 `)`")?;
        let returns_score = if self.take(&TokenKind::Arrow).is_some() {
            self.expect_word("score")?;
            true
        } else {
            false
        };
        let (body, end) = self.block()?;
        Ok(Function {
            exported: false,
            name,
            name_span,
            parameters,
            returns_score,
            attributes,
            body,
            span: start.merge(end),
        })
    }

    pub(in crate::parser) fn unsigned_call(&mut self, name: &str) -> Result<u32, Diagnostic> {
        self.expect(TokenKind::LeftParen, &format!("{name} 后需要 `(`"))?;
        let value = self.unsigned(name)?;
        self.expect(TokenKind::RightParen, &format!("{name} 后需要 `)`"))?;
        self.expect(TokenKind::Semicolon, "查询属性后需要 `;`")?;
        Ok(value)
    }

    pub(in crate::parser) fn unsigned(&mut self, name: &str) -> Result<u32, Diagnostic> {
        self.unsigned_with_span(name).map(|(value, _)| value)
    }

    /// 读取可带负号的整数；供 `xp add` 这类允许减少的命令使用。
    pub(in crate::parser) fn signed(&mut self, name: &str) -> Result<i32, Diagnostic> {
        let minus = self.take(&TokenKind::Minus);
        let token = self.advance().clone();
        let TokenKind::Number(value) = token.kind else {
            return Err(Diagnostic::new(format!("{name} 需要整数"), token.span));
        };
        let signed = if minus.is_some() {
            value
                .checked_neg()
                .ok_or_else(|| Diagnostic::new(format!("{name} 超出 32 位范围"), token.span))?
        } else {
            value
        };
        i32::try_from(signed)
            .map_err(|_| Diagnostic::new(format!("{name} 超出 32 位范围"), token.span))
    }

    /// 读取可带符号的整数或小数并返回规范文本，供原版双精度参数使用。
    ///
    /// 直接保留源文本可以避免把 64 位整数未经检查地转成 `f64`，输出也更贴近手写命令。
    pub(in crate::parser) fn signed_number_text(
        &mut self,
        name: &str,
    ) -> Result<String, Diagnostic> {
        let negative = self.negative_sign();
        let token = self.advance().clone();
        let text = match token.kind {
            TokenKind::Number(value) => value.to_string(),
            TokenKind::Decimal(value) => format!("{value}"),
            _ => return Err(Diagnostic::new(format!("{name} 需要数字"), token.span)),
        };
        Ok(if negative { format!("-{text}") } else { text })
    }

    /// 读取可选的正负号；原版参数接受 `+5`，这里把正号规范化为无符号。
    pub(in crate::parser) fn negative_sign(&mut self) -> bool {
        if self.take(&TokenKind::Minus).is_some() {
            return true;
        }
        self.take(&TokenKind::Plus);
        false
    }

    pub(in crate::parser) fn unsigned_with_span(
        &mut self,
        name: &str,
    ) -> Result<(u32, Span), Diagnostic> {
        let token = self.advance().clone();
        let TokenKind::Number(value) = token.kind else {
            return Err(Diagnostic::new(format!("{name} 需要非负整数"), token.span));
        };
        let value = u32::try_from(value)
            .map_err(|_| Diagnostic::new(format!("{name} 超出范围"), token.span))?;
        Ok((value, token.span))
    }
}
