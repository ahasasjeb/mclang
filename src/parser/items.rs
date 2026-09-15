//! 物品定义解析：物品堆资源位置与全部组件属性。

use crate::ast::*;
use crate::diagnostic::Diagnostic;
use crate::lexer::TokenKind;

use super::Parser;
use super::keywords::{boolean_word, item_stack_property, rarity_value};

impl Parser {
    pub(super) fn item_stack(&mut self) -> Result<ItemStackDecl, Diagnostic> {
        let start = self.expect_word("item")?.span;
        let (name, _) = self.ident("物品定义名称")?;
        self.expect(TokenKind::Equal, "物品定义名称后需要 `=`")?;
        self.expect_word("item_stack")?;
        self.expect(TokenKind::LeftParen, "item_stack 后需要 `(`")?;
        let (item_id, _) = self.string("item_stack 需要物品资源位置")?;
        self.expect(TokenKind::RightParen, "物品资源位置后需要 `)`")?;
        self.expect(TokenKind::LeftBrace, "物品定义需要 `{`")?;

        let mut count = 1;
        let mut has_count = false;
        let mut custom_name = None;
        let mut item_name = None;
        let mut lore = Vec::new();
        let mut enchantments = Vec::new();
        let mut stored_enchantments = Vec::new();
        let mut damage = None;
        let mut max_damage = None;
        let mut max_stack_size = None;
        let mut rarity = None;
        let mut item_model = None;
        let mut dyed_color = None;
        let mut enchantment_glint_override = None;
        let mut unbreakable = false;
        let mut has_unbreakable = false;
        while !self.check(&TokenKind::RightBrace) {
            if self.check(&TokenKind::Eof) {
                return Err(Diagnostic::new("物品定义缺少 `}`", self.current().span));
            }
            let (property, span) = self.ident("物品定义属性")?;
            let Some(property_kind) = item_stack_property(&property) else {
                return Err(Diagnostic::new(
                    format!("未知物品定义属性 `{property}`"),
                    span,
                ));
            };
            match property_kind {
                "count" => {
                    if has_count {
                        return Err(Diagnostic::new("物品数量只能声明一次", span));
                    }
                    has_count = true;
                    self.expect(TokenKind::Equal, "count 后需要 `=`")?;
                    count = self.unsigned("物品数量")?;
                    self.expect(TokenKind::Semicolon, "物品数量后需要 `;`")?;
                }
                "custom_name" => {
                    if custom_name.is_some() {
                        return Err(Diagnostic::new("物品自定义名称只能声明一次", span));
                    }
                    self.expect(TokenKind::Equal, "custom_name 后需要 `=`")?;
                    custom_name = Some(self.string("custom_name 需要文本字符串")?.0);
                    self.expect(TokenKind::Semicolon, "物品自定义名称后需要 `;`")?;
                }
                "item_name" => {
                    if item_name.is_some() {
                        return Err(Diagnostic::new("物品名称只能声明一次", span));
                    }
                    self.expect(TokenKind::Equal, "item_name 后需要 `=`")?;
                    item_name = Some(self.string("item_name 需要文本字符串")?.0);
                    self.expect(TokenKind::Semicolon, "物品名称后需要 `;`")?;
                }
                "rarity" => {
                    if rarity.is_some() {
                        return Err(Diagnostic::new("物品稀有度只能声明一次", span));
                    }
                    self.expect(TokenKind::Equal, "rarity 后需要 `=`")?;
                    let (value, value_span) = self.ident("稀有度名称")?;
                    let Some(value) = rarity_value(&value) else {
                        return Err(Diagnostic::new(
                            "稀有度只能是 common、uncommon、rare 或 epic",
                            value_span,
                        ));
                    };
                    rarity = Some(value);
                    self.expect(TokenKind::Semicolon, "稀有度后需要 `;`")?;
                }
                "item_model" => {
                    if item_model.is_some() {
                        return Err(Diagnostic::new("物品模型只能声明一次", span));
                    }
                    self.expect(TokenKind::Equal, "item_model 后需要 `=`")?;
                    item_model = Some(self.string("item_model 需要物品模型资源位置")?.0);
                    self.expect(TokenKind::Semicolon, "物品模型后需要 `;`")?;
                }
                "lore" => {
                    self.expect(TokenKind::LeftParen, "lore 后需要 `(`")?;
                    lore.push(self.string("lore 需要文本字符串")?.0);
                    self.expect(TokenKind::RightParen, "lore 文本后需要 `)`")?;
                    self.expect(TokenKind::Semicolon, "lore 后需要 `;`")?;
                }
                "enchantment" | "stored_enchantment" => {
                    self.expect(TokenKind::LeftParen, "enchantment 后需要 `(`")?;
                    let (enchantment_id, enchantment_span) =
                        self.string("enchantment 需要附魔资源位置")?;
                    self.expect(TokenKind::Comma, "附魔资源位置后需要 `,`")?;
                    let level = self.unsigned("附魔等级")?;
                    self.expect(TokenKind::RightParen, "附魔等级后需要 `)`")?;
                    self.expect(TokenKind::Semicolon, "enchantment 后需要 `;`")?;
                    let enchantment = ItemEnchantment {
                        enchantment_id,
                        level,
                        span: enchantment_span,
                    };
                    if property_kind == "enchantment" {
                        enchantments.push(enchantment);
                    } else {
                        stored_enchantments.push(enchantment);
                    }
                }
                "damage" => {
                    if damage.is_some() {
                        return Err(Diagnostic::new("物品损伤值只能声明一次", span));
                    }
                    self.expect(TokenKind::Equal, "damage 后需要 `=`")?;
                    damage = Some(self.unsigned("物品损伤值")?);
                    self.expect(TokenKind::Semicolon, "物品损伤值后需要 `;`")?;
                }
                "max_damage" => {
                    if max_damage.is_some() {
                        return Err(Diagnostic::new("物品最大损伤值只能声明一次", span));
                    }
                    self.expect(TokenKind::Equal, "max_damage 后需要 `=`")?;
                    max_damage = Some(self.unsigned("物品最大损伤值")?);
                    self.expect(TokenKind::Semicolon, "物品最大损伤值后需要 `;`")?;
                }
                "max_stack_size" => {
                    if max_stack_size.is_some() {
                        return Err(Diagnostic::new("物品最大堆叠数只能声明一次", span));
                    }
                    self.expect(TokenKind::Equal, "max_stack_size 后需要 `=`")?;
                    max_stack_size = Some(self.unsigned("物品最大堆叠数")?);
                    self.expect(TokenKind::Semicolon, "物品最大堆叠数后需要 `;`")?;
                }
                "dyed_color" => {
                    if dyed_color.is_some() {
                        return Err(Diagnostic::new("物品染色只能声明一次", span));
                    }
                    self.expect(TokenKind::Equal, "dyed_color 后需要 `=`")?;
                    dyed_color = Some(self.unsigned("物品染色 RGB 值")?);
                    self.expect(TokenKind::Semicolon, "物品染色后需要 `;`")?;
                }
                "enchantment_glint_override" => {
                    if enchantment_glint_override.is_some() {
                        return Err(Diagnostic::new("附魔光效覆盖只能声明一次", span));
                    }
                    self.expect(TokenKind::Equal, "enchantment_glint_override 后需要 `=`")?;
                    let (value, value_span) = self.ident("true 或 false")?;
                    enchantment_glint_override = Some(match boolean_word(&value) {
                        Some("true") => true,
                        Some("false") => false,
                        _ => return Err(Diagnostic::new("这里需要 true 或 false", value_span)),
                    });
                    self.expect(TokenKind::Semicolon, "附魔光效覆盖后需要 `;`")?;
                }
                "unbreakable" => {
                    if has_unbreakable {
                        return Err(Diagnostic::new("unbreakable 只能声明一次", span));
                    }
                    has_unbreakable = true;
                    self.expect(TokenKind::Equal, "unbreakable 后需要 `=`")?;
                    let (value, value_span) = self.ident("true 或 false")?;
                    unbreakable = match boolean_word(&value) {
                        Some("true") => true,
                        Some("false") => false,
                        _ => return Err(Diagnostic::new("这里需要 true 或 false", value_span)),
                    };
                    self.expect(TokenKind::Semicolon, "unbreakable 后需要 `;`")?;
                }
                _ => unreachable!(),
            }
        }
        let end = self.advance().span;
        self.take(&TokenKind::Semicolon);
        Ok(ItemStackDecl {
            name,
            item_id,
            count,
            custom_name,
            item_name,
            lore,
            enchantments,
            stored_enchantments,
            damage,
            max_damage,
            max_stack_size,
            rarity,
            item_model,
            dyed_color,
            enchantment_glint_override,
            unbreakable,
            span: start.merge(end),
        })
    }
}
