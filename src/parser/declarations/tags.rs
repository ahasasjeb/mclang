use crate::ast::*;
use crate::diagnostic::Diagnostic;
use crate::lexer::TokenKind;

use crate::parser::Parser;
use crate::parser::keywords::{boolean_word, function_tag_property, resource_kind};

impl Parser {
    /// `fn_tag 名称 { value(函数或#标签); replace = 真; }`
    pub(in crate::parser) fn function_tag(&mut self) -> Result<FunctionTagDecl, Diagnostic> {
        let start = self.expect_word("fn_tag")?.span;
        let (name, name_span) = self.ident("函数标签名称")?;
        self.expect(TokenKind::LeftBrace, "函数标签需要 `{`")?;
        let mut values = Vec::new();
        let mut replace = false;
        let mut replace_span = None;
        while !self.check(&TokenKind::RightBrace) {
            if self.check(&TokenKind::Eof) {
                return Err(Diagnostic::new("函数标签缺少 `}`", self.current().span));
            }
            let (property, property_span) = self.ident("函数标签属性")?;
            let Some(property_kind) = function_tag_property(&property) else {
                return Err(Diagnostic::new(
                    format!("未知函数标签属性 `{property}`"),
                    property_span,
                ));
            };
            match property_kind {
                "value" => {
                    self.expect(TokenKind::LeftParen, "value 后需要 `(`")?;
                    values.push(self.function_tag_entry()?);
                    self.expect(TokenKind::RightParen, "标签条目后需要 `)`")?;
                    self.expect(TokenKind::Semicolon, "标签条目后需要 `;`")?;
                }
                "replace" => {
                    if replace_span.is_some() {
                        return Err(Diagnostic::new(
                            "函数标签只能声明一次 replace",
                            property_span,
                        ));
                    }
                    self.expect(TokenKind::Equal, "replace 后需要 `=`")?;
                    let (value, value_span) = self.ident("true 或 false")?;
                    replace = match boolean_word(&value) {
                        Some("true") => true,
                        Some("false") => false,
                        _ => return Err(Diagnostic::new("这里需要 true 或 false", value_span)),
                    };
                    self.expect(TokenKind::Semicolon, "replace 后需要 `;`")?;
                    replace_span = Some(property_span);
                }
                _ => unreachable!(),
            }
        }
        let end = self.advance().span;
        self.take(&TokenKind::Semicolon);
        Ok(FunctionTagDecl {
            exported: false,
            name,
            name_span,
            values,
            replace,
            span: start.merge(end),
        })
    }

    /// 标签条目：本命名空间函数、`#` 本命名空间标签或字符串形式的外部资源位置。
    fn function_tag_entry(&mut self) -> Result<FunctionTagEntry, Diagnostic> {
        if let Some(hash) = self.take(&TokenKind::Hash) {
            let token = self.advance().clone();
            match token.kind {
                TokenKind::Ident(value) => {
                    Ok(FunctionTagEntry::Tag(value, hash.span.merge(token.span)))
                }
                TokenKind::String(value) => Ok(FunctionTagEntry::External(
                    format!("#{value}"),
                    hash.span.merge(token.span),
                )),
                _ => Err(Diagnostic::new(
                    "`#` 后需要函数标签名称或字符串资源位置",
                    token.span,
                )),
            }
        } else {
            let token = self.advance().clone();
            match token.kind {
                TokenKind::Ident(value) => Ok(FunctionTagEntry::Function(value, token.span)),
                TokenKind::String(value) => Ok(FunctionTagEntry::External(value, token.span)),
                _ => Err(Diagnostic::new(
                    "标签条目需要函数名称、`#标签` 或字符串资源位置",
                    token.span,
                )),
            }
        }
    }

    pub(in crate::parser) fn resource(&mut self) -> Result<ResourceDecl, Diagnostic> {
        let start = self.expect_word("resource")?.span;
        let (kind, _) = self.resource_path("资源类型")?;
        let kind = resource_kind(&kind).unwrap_or(&kind).to_owned();
        let (name, name_span) = self.resource_path("资源名称")?;
        self.expect(TokenKind::Equal, "资源名称后需要 `=`")?;
        let (json, _) = self.string("资源内容需要 JSON 字符串")?;
        let end = self
            .expect(TokenKind::Semicolon, "资源声明后需要 `;`")?
            .span;
        Ok(ResourceDecl {
            exported: false,
            kind,
            name,
            name_span,
            json,
            span: start.merge(end),
        })
    }

    pub(in crate::parser) fn resource_path(
        &mut self,
        expected: &str,
    ) -> Result<(String, Span), Diagnostic> {
        let token = self.advance().clone();
        match token.kind {
            TokenKind::Ident(value) | TokenKind::String(value) => Ok((value, token.span)),
            _ => Err(Diagnostic::new(
                format!("这里需要{expected}标识符或字符串"),
                token.span,
            )),
        }
    }
}
