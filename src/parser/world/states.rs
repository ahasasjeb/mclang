use crate::ast::*;
use crate::diagnostic::Diagnostic;
use crate::lexer::TokenKind;

use super::Parser;

impl Parser {
    /// `block_state("资源位置") { 属性 = "值"; ... }`；属性块可省略。
    pub(in crate::parser) fn block_state_value(
        &mut self,
        label: &str,
    ) -> Result<BlockStateValue, Diagnostic> {
        let Some(start) = self.take_word("block_state") else {
            return Err(Diagnostic::new(
                format!(
                    "{label}需要 `block_state(\"命名空间:方块\") {{ ... }}`（中文 `方块状态(...)`）"
                ),
                self.current().span,
            ));
        };
        self.expect(TokenKind::LeftParen, "block_state 后需要 `(`")?;
        let (id, _) = self.string("block_state 需要方块资源位置字符串")?;
        let end = self
            .expect(TokenKind::RightParen, "方块资源位置后需要 `)`")?
            .span;
        let mut span = start.span.merge(end);
        let mut properties = Vec::new();
        if self.take(&TokenKind::LeftBrace).is_some() {
            while !self.check(&TokenKind::RightBrace) {
                if self.check(&TokenKind::Eof) {
                    return Err(Diagnostic::new("方块状态缺少 `}`", self.current().span));
                }
                let (name, name_span) = self.ident("方块属性名称")?;
                self.expect(TokenKind::Equal, "方块属性后需要 `=`")?;
                let (value, value_span) = self.string("方块属性值需要字符串，例如 \"east\"")?;
                self.expect(TokenKind::Semicolon, "方块属性后需要 `;`")?;
                if properties
                    .iter()
                    .any(|property: &BlockProperty| property.name == name)
                {
                    return Err(Diagnostic::new(
                        format!("方块属性 `{name}` 重复声明"),
                        name_span,
                    ));
                }
                properties.push(BlockProperty {
                    name,
                    value,
                    span: name_span.merge(value_span),
                });
            }
            let close = self.advance().span;
            self.take(&TokenKind::Semicolon);
            span = span.merge(close);
        }
        Ok(BlockStateValue {
            id,
            properties,
            span,
        })
    }

    /// 时间操作的可选世界时钟参数：`..., "minecraft:overworld"`。
    pub(super) fn optional_clock(&mut self, label: &str) -> Result<Option<String>, Diagnostic> {
        if self.take(&TokenKind::Comma).is_none() {
            return Ok(None);
        }
        Ok(Some(self.string(label)?.0))
    }
}
