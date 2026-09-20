use super::Parser;
use crate::{ast::*, diagnostic::Diagnostic, lexer::TokenKind};

impl Parser {
    pub(super) fn item_predicate(&mut self) -> Result<ItemPredicate, Diagnostic> {
        let start = self.current().span;
        if matches!(self.current().kind, TokenKind::String(_)) {
            let (item, span) = self.string("物品或标签资源位置")?;
            return Ok(ItemPredicate {
                item,
                clauses: Vec::new(),
                span,
            });
        }
        self.expect_word("item_predicate")?;
        self.expect(TokenKind::LeftParen, "item_predicate 后需要 `(`")?;
        let item = self.string("物品、#标签或 *")?.0;
        if item.contains('[') {
            return Err(Diagnostic::new(
                "item_predicate 的基项只接受物品、#标签或 *，组件条件请写在块内",
                start,
            ));
        }
        self.expect(TokenKind::RightParen, "物品谓词缺少 `)`")?;
        self.expect(TokenKind::LeftBrace, "物品谓词需要 `{`")?;
        let mut clauses = Vec::new();
        while !self.check(&TokenKind::RightBrace) {
            let mut alternatives = Vec::new();
            loop {
                let span = self.current().span;
                let negated = self.take(&TokenKind::Bang).is_some();
                let method = self.command_choice(&["has", "equals", "matches"])?;
                self.expect(TokenKind::LeftParen, "组件条件需要 `(`")?;
                let id = self.string("组件或子谓词资源位置")?.0;
                let kind = if method == "has" {
                    ItemComponentTestKind::Present
                } else {
                    self.command_comma()?;
                    self.take_word("nbt");
                    let value = self.nbt_value()?;
                    if method == "equals" {
                        ItemComponentTestKind::Equal(value)
                    } else {
                        ItemComponentTestKind::Match(value)
                    }
                };
                self.expect(TokenKind::RightParen, "组件条件缺少 `)`")?;
                alternatives.push(ItemComponentTest {
                    id,
                    negated,
                    kind,
                    span,
                });
                if self.take(&TokenKind::OrOr).is_none() {
                    break;
                }
            }
            self.expect(TokenKind::Semicolon, "物品谓词条件后需要 `;`")?;
            clauses.push(alternatives);
        }
        let end = self.advance().span;
        Ok(ItemPredicate {
            item,
            clauses,
            span: start.merge(end),
        })
    }
}
