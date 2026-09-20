use super::Parser;
use crate::{ast::*, diagnostic::Diagnostic, lexer::TokenKind};

impl Parser {
    pub(in crate::parser) fn record_macro_coordinate(
        &mut self,
        coordinate: &Coordinate,
        kind: MacroCoordinateKind,
    ) {
        if let Coordinate::Macro(text) = coordinate {
            self.active_macro_coordinates
                .push((text[2..text.len() - 1].to_owned(), kind));
        }
    }
    pub(super) fn macro_parameters(&mut self) -> Result<Vec<MacroParameter>, Diagnostic> {
        let mut parameters = Vec::new();
        if self.check(&TokenKind::RightParen) {
            return Err(Diagnostic::new(
                "宏函数需要至少一个具名参数",
                self.current().span,
            ));
        }
        loop {
            let kind = match self
                .command_choice(&["integer", "decimal", "text", "resource", "nbt"])?
                .as_str()
            {
                "integer" => MacroType::Integer,
                "decimal" => MacroType::Decimal,
                "text" => MacroType::Text,
                "resource" => MacroType::Resource,
                _ => MacroType::Nbt,
            };
            let (name, span) = self.ident("宏参数名称")?;
            self.active_macro_parameters.insert(name.clone(), kind);
            parameters.push(MacroParameter { name, kind, span });
            if !self.command_optional_comma() {
                break;
            }
        }
        Ok(parameters)
    }

    pub(in crate::parser) fn macro_coordinate(
        &mut self,
        integer: bool,
    ) -> Result<Option<Coordinate>, Diagnostic> {
        if !self.check_word("macro") {
            return Ok(None);
        }
        self.advance();
        self.expect(TokenKind::LeftParen, "macro 坐标分量需要 `(`")?;
        let (name, span) = self.ident("宏坐标参数")?;
        let kind = self.active_macro_parameters.get(&name);
        if !matches!(kind, Some(MacroType::Integer))
            && (integer || !matches!(kind, Some(MacroType::Decimal)))
        {
            return Err(Diagnostic::new(
                format!(
                    "宏坐标 `{name}` 需要声明为 {} 参数",
                    if integer {
                        "integer"
                    } else {
                        "integer 或 decimal"
                    }
                ),
                span,
            ));
        }
        self.expect(TokenKind::RightParen, "macro 坐标分量缺少 `)`")?;
        self.active_macro_uses.push((name.clone(), span));
        Ok(Some(Coordinate::Macro(format!("$({name})"))))
    }

    pub(super) fn function_call_statement(&mut self) -> Result<StatementKind, Diagnostic> {
        let target = self.call_target("被调用函数名称")?;
        if self.check_word("with") {
            self.advance();
            self.expect(TokenKind::LeftParen, "with 后需要 `(`")?;
            let source = self.nbt_source_value("宏参数来源")?;
            let path = if self.command_optional_comma() {
                Some(self.string("宏参数 NBT 路径")?)
            } else {
                None
            };
            self.expect(TokenKind::RightParen, "with 缺少 `)`")?;
            self.expect(TokenKind::Semicolon, "宏调用后需要 `;`")?;
            return Ok(StatementKind::MacroCall {
                target,
                arguments: MacroArguments::With { source, path },
            });
        }
        if self.check(&TokenKind::LeftParen)
            && matches!(&self.peek_kind(1).kind, TokenKind::Ident(name) if super::keywords::word_matches(name, "nbt"))
        {
            self.advance();
            let arguments = MacroArguments::Literal(self.nbt_compound("宏参数")?);
            self.expect(TokenKind::RightParen, "宏参数后需要 `)`")?;
            self.expect(TokenKind::Semicolon, "宏调用后需要 `;`")?;
            return Ok(StatementKind::MacroCall { target, arguments });
        }
        let arguments = self.call_arguments()?;
        self.expect(TokenKind::Semicolon, "函数调用后需要 `;`")?;
        Ok(StatementKind::Call { target, arguments })
    }
}

pub(super) fn template_uses(text: &str, span: Span) -> Result<Vec<(String, Span)>, Diagnostic> {
    let mut uses = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find("$(") {
        rest = &rest[start + 2..];
        let Some(end) = rest.find(')') else {
            return Err(Diagnostic::new("宏占位符缺少 `)`", span));
        };
        let name = &rest[..end];
        if name.is_empty() || !name.chars().all(|c| c.is_alphanumeric() || c == '_') {
            return Err(Diagnostic::new(format!("无效宏参数名 `{name}`"), span));
        }
        uses.push((name.to_owned(), span));
        rest = &rest[end + 1..];
    }
    Ok(uses)
}
