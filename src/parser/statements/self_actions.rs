use crate::ast::*;
use crate::diagnostic::Diagnostic;
use crate::lexer::TokenKind;

use crate::parser::Parser;
use crate::parser::keywords::{
    boolean_word, effect_method, self_method, word_matches, xp_kind, xp_method,
};

impl Parser {
    pub(super) fn give_statement(&mut self) -> Result<StatementKind, Diagnostic> {
        self.expect(TokenKind::LeftParen, "give 后需要 `(`")?;
        let target = if self.take_word("origin").is_some() {
            GiveTarget::Origin
        } else {
            let (name, _) = self.ident("give 需要玩家查询名称或 origin/投掷者")?;
            GiveTarget::Query(name)
        };
        self.expect(TokenKind::Comma, "give 目标后需要 `,`")?;
        let item = if self.take_word("self").is_some() {
            self.expect(TokenKind::Dot, "self 后需要 `.`")?;
            let (member, span) = self.ident("self 的物品成员")?;
            if !word_matches(&member, "item") {
                return Err(Diagnostic::new(
                    "原样给予的写法是 `self.item`（中文 `自身.物品`）",
                    span,
                ));
            }
            GiveItem::SelfItem
        } else {
            let (name, _) = self.ident("give 需要物品定义名称或 self.item/自身.物品")?;
            GiveItem::Definition(name)
        };
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

    pub(super) fn optional_count(
        &mut self,
        name: &str,
    ) -> Result<(Option<u32>, Option<Span>), Diagnostic> {
        if self.take(&TokenKind::Comma).is_none() {
            return Ok((None, None));
        }
        let (count, span) = self.unsigned_with_span(name)?;
        Ok((Some(count), Some(span)))
    }

    pub(super) fn self_action(&mut self) -> Result<SelfAction, Diagnostic> {
        self.expect(TokenKind::Dot, "self 后需要 `.`")?;
        let (method, span) = self.ident("self 方法名称")?;
        if word_matches(&method, "nbt") {
            return Err(Diagnostic::new(
                "实体 NBT 合并直接写成 `nbt { ... }`（中文 `数据 { ... }`），不需要 self 前缀",
                span,
            ));
        }
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
            "set_invulnerable" | "set_no_gravity" => {
                let (value, value_span) = self.ident("true 或 false")?;
                let value = match boolean_word(&value) {
                    Some("true") => true,
                    Some("false") => false,
                    _ => return Err(Diagnostic::new("这里需要 true 或 false", value_span)),
                };
                if method_kind == "set_invulnerable" {
                    SelfAction::SetInvulnerable(value)
                } else {
                    SelfAction::SetNoGravity(value)
                }
            }
            "save_items" | "restore_items" => {
                let (reference, _) = self.ident("物品存储名称")?;
                match method_kind {
                    "save_items" => SelfAction::SaveItems(reference),
                    "restore_items" => SelfAction::RestoreItems(reference),
                    _ => unreachable!(),
                }
            }
            "remove_preserving_items" => {
                let (reference, reference_span) = self.ident("物品存储名称或数据槽名称")?;
                if self.take(&TokenKind::Comma).is_some() {
                    let (query, query_span) = self.ident("实体查询名称")?;
                    SelfAction::RemovePreservingSlot {
                        slot: reference,
                        slot_span: reference_span,
                        query,
                        query_span,
                    }
                } else {
                    SelfAction::RemovePreservingItems(reference)
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
            "deposit" | "withdraw" => {
                let (slot, slot_span) = self.ident("数据槽名称")?;
                self.expect(TokenKind::Comma, "数据槽后需要 `,`")?;
                let (query, query_span) = self.ident("实体查询名称")?;
                if method_kind == "deposit" {
                    SelfAction::DataStore {
                        slot,
                        slot_span,
                        query,
                        query_span,
                    }
                } else {
                    SelfAction::DataLoad {
                        slot,
                        slot_span,
                        query,
                        query_span,
                    }
                }
            }
            "remove_data" => {
                let (slot, slot_span) = self.ident("数据槽名称")?;
                SelfAction::DataClear { slot, slot_span }
            }
            "clear_items" => SelfAction::ClearItems,
            "remove" => SelfAction::Remove,
            _ => unreachable!(),
        };
        self.expect(TokenKind::RightParen, "self 方法缺少 `)`")?;
        self.expect(TokenKind::Semicolon, "self 方法调用后需要 `;`")?;
        Ok(action)
    }
    pub(super) fn effect_statement(&mut self) -> Result<StatementKind, Diagnostic> {
        self.expect(TokenKind::Dot, "effect 后需要 `.`")?;
        let (method, method_span) = self.ident("effect 方法")?;
        let Some(method) = effect_method(&method) else {
            return Err(Diagnostic::new(
                format!("未知 effect 方法 `{method}`"),
                method_span,
            ));
        };
        self.expect(TokenKind::LeftParen, "effect 方法后需要 `(`")?;
        let (target, _) = self.ident("effect 目标查询名称")?;
        let kind = match method {
            "give" | "give_infinite" => {
                self.expect(TokenKind::Comma, "目标后需要 `,`")?;
                let (effect, _) = self.string("effect 需要效果资源位置")?;
                let duration = if method == "give" {
                    self.expect(TokenKind::Comma, "效果资源位置后需要 `,`")?;
                    EffectDuration::Seconds(self.unsigned("effect 持续秒数")?)
                } else {
                    EffectDuration::Infinite
                };
                let mut amplifier = None;
                let mut hide_particles = false;
                if self.take(&TokenKind::Comma).is_some() {
                    amplifier = Some(self.unsigned("effect 等级")?);
                    if self.take(&TokenKind::Comma).is_some() {
                        let (value, value_span) = self.ident("true 或 false")?;
                        hide_particles = match boolean_word(&value) {
                            Some("true") => true,
                            Some("false") => false,
                            _ => return Err(Diagnostic::new("这里需要 true 或 false", value_span)),
                        };
                    }
                }
                StatementKind::EffectGive {
                    target,
                    effect,
                    duration,
                    amplifier,
                    hide_particles,
                }
            }
            "clear" => {
                let effect = if self.take(&TokenKind::Comma).is_some() {
                    Some(self.string("effect 需要效果资源位置")?.0)
                } else {
                    None
                };
                StatementKind::EffectClear { target, effect }
            }
            _ => unreachable!(),
        };
        self.expect(TokenKind::RightParen, "effect 调用缺少 `)`")?;
        self.expect(TokenKind::Semicolon, "effect 调用后需要 `;`")?;
        Ok(kind)
    }

    pub(super) fn xp_statement(&mut self) -> Result<StatementKind, Diagnostic> {
        self.expect(TokenKind::Dot, "xp 后需要 `.`")?;
        let (method, method_span) = self.ident("xp 方法")?;
        let Some(method) = xp_method(&method) else {
            return Err(Diagnostic::new(
                format!("未知 xp 方法 `{method}`"),
                method_span,
            ));
        };
        if method == "query" {
            return Err(Diagnostic::new(
                "xp.query 只能出现在表达式里，例如 `let level = xp.query(players, levels);`",
                method_span,
            ));
        }
        self.expect(TokenKind::LeftParen, "xp 方法后需要 `(`")?;
        let (target, _) = self.ident("xp 目标查询名称")?;
        self.expect(TokenKind::Comma, "目标后需要 `,`")?;
        let (kind, kind_span) = self.ident("xp 类型 points 或 levels")?;
        let Some(kind) = xp_kind(&kind) else {
            return Err(Diagnostic::new(
                "xp 类型只能是 points/点数 或 levels/等级",
                kind_span,
            ));
        };
        self.expect(TokenKind::Comma, "xp 类型后需要 `,`")?;
        let amount = self.signed("xp 数量")?;
        self.expect(TokenKind::RightParen, "xp 调用缺少 `)`")?;
        self.expect(TokenKind::Semicolon, "xp 调用后需要 `;`")?;
        Ok(StatementKind::XpChange {
            target,
            kind,
            operation: if method == "add" {
                XpOperation::Add
            } else {
                XpOperation::Set
            },
            amount,
        })
    }

    pub(super) fn clear_statement(&mut self) -> Result<StatementKind, Diagnostic> {
        self.expect(TokenKind::LeftParen, "clear 后需要 `(`")?;
        let (target, _) = self.ident("clear 目标查询名称")?;
        let item = if self.take(&TokenKind::Comma).is_some() {
            Some(self.string("clear 需要物品资源位置")?.0)
        } else {
            None
        };
        let max_count = if self.take(&TokenKind::Comma).is_some() {
            if item.is_none() {
                return Err(Diagnostic::new(
                    "clear 的数量需要先写出物品，例如 `clear(players, \"minecraft:diamond\", 5);`",
                    self.previous().span,
                ));
            }
            Some(self.unsigned("clear 最大数量")?)
        } else {
            None
        };
        self.expect(TokenKind::RightParen, "clear 调用缺少 `)`")?;
        self.expect(TokenKind::Semicolon, "clear 调用后需要 `;`")?;
        Ok(StatementKind::ClearInventory {
            target,
            item,
            max_count,
        })
    }
}
