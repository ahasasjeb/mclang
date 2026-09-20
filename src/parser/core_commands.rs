use super::Parser;
use crate::{ast::*, diagnostic::Diagnostic, lexer::TokenKind};

impl Parser {
    pub(super) fn core_command_root(&self) -> Option<&'static str> {
        ["reload", "recipe", "datapack", "loot", "random"]
            .into_iter()
            .find(|root| {
                self.check_word(root)
                    && (*root != "random" || self.peek_kind(1).kind == TokenKind::Dot)
            })
    }

    pub(super) fn core_command(&mut self, root: &str) -> Result<CoreCommand, Diagnostic> {
        let method = if root != "reload" {
            self.command_method()?
        } else {
            String::new()
        };
        self.expect(TokenKind::LeftParen, "命令需要 `(`")?;
        let command = match root {
            "reload" => CoreCommand::Reload,
            "recipe" => {
                if !["give", "take"].contains(&method.as_str()) {
                    return self.unknown_command_method(root, &method);
                }
                let target = self.holder("配方玩家")?;
                self.command_comma()?;
                let recipe = if self.take(&TokenKind::Star).is_some() {
                    None
                } else {
                    Some(self.advancement_reference("配方")?)
                };
                CoreCommand::Recipe {
                    give: method == "give",
                    target,
                    recipe,
                }
            }
            "datapack" => CoreCommand::Datapack(self.datapack_command(&method)?),
            "random" => self.random_command(&method)?,
            "loot" => self.loot_command(&method)?,
            _ => unreachable!("core command dispatch"),
        };
        self.expect(TokenKind::RightParen, "命令缺少 `)`")?;
        Ok(command)
    }

    fn datapack_command(&mut self, method: &str) -> Result<DatapackOperation, Diagnostic> {
        Ok(match method {
            "enable" => {
                let name = self.string("数据包名称")?.0;
                let order = if self.command_optional_comma() {
                    Some(
                        match self
                            .command_choice(&["first", "last", "before", "after"])?
                            .as_str()
                        {
                            "first" => PackOrder::First,
                            "last" => PackOrder::Last,
                            "before" => {
                                self.command_comma()?;
                                PackOrder::Before(self.string("参照数据包名称")?.0)
                            }
                            _ => {
                                self.command_comma()?;
                                PackOrder::After(self.string("参照数据包名称")?.0)
                            }
                        },
                    )
                } else {
                    None
                };
                DatapackOperation::Enable { name, order }
            }
            "disable" => DatapackOperation::Disable(self.string("数据包名称")?.0),
            "list" => DatapackOperation::List(if self.check(&TokenKind::RightParen) {
                None
            } else {
                Some(self.command_choice(&["available", "enabled"])?)
            }),
            "create" => {
                return Err(Diagnostic::new(
                    "datapack.create 需要 OWNER 权限（4），默认数据包函数权限为 GAMEMASTER（2），不可达",
                    self.previous().span,
                ));
            }
            _ => return self.unknown_command_method("datapack", method),
        })
    }

    fn random_command(&mut self, method: &str) -> Result<CoreCommand, Diagnostic> {
        Ok(match method {
            "value" | "roll" => {
                let min = self.signed("随机数下界")?;
                self.command_comma()?;
                let max = self.signed("随机数上界")?;
                let sequence = if self.command_optional_comma() {
                    Some(self.string("随机序列资源位置")?.0)
                } else {
                    None
                };
                CoreCommand::Random {
                    roll: method == "roll",
                    min,
                    max,
                    sequence,
                }
            }
            "reset" => {
                let sequence = if self.take(&TokenKind::Star).is_some() {
                    "*".to_owned()
                } else {
                    self.string("随机序列资源位置或 *")?.0
                };
                let seed = if self.command_optional_comma() {
                    Some(self.signed("随机种子盐值")?)
                } else {
                    None
                };
                let world_seed = if seed.is_some() && self.command_optional_comma() {
                    Some(self.command_boolean()?)
                } else {
                    None
                };
                let sequence_id = if world_seed.is_some() && self.command_optional_comma() {
                    Some(self.command_boolean()?)
                } else {
                    None
                };
                CoreCommand::RandomReset {
                    sequence,
                    seed,
                    world_seed,
                    sequence_id,
                }
            }
            _ => return self.unknown_command_method("random", method),
        })
    }

    fn loot_command(&mut self, method: &str) -> Result<CoreCommand, Diagnostic> {
        let target = match method {
            "give" => LootTarget::Give(self.holder("战利品玩家")?),
            "insert" => LootTarget::Insert(self.block_position("战利品容器")?),
            "spawn" => LootTarget::Spawn(self.position_value("战利品掉落位置")?),
            "replace" => {
                let target = self.item_condition_source("战利品替换目标")?;
                self.command_comma()?;
                let slot = self.string("战利品起始槽位")?.0;
                let count = if self.command_optional_comma() {
                    if matches!(self.current().kind, TokenKind::Number(_)) {
                        let count = self.unsigned("战利品槽位数量")?;
                        Some(count)
                    } else {
                        self.cursor -= 1;
                        None
                    }
                } else {
                    None
                };
                LootTarget::Replace {
                    target,
                    slot,
                    count,
                }
            }
            _ => return self.unknown_command_method("loot", method),
        };
        self.command_comma()?;
        self.expect_word("loot")?;
        let method = self.command_method()?;
        self.expect(TokenKind::LeftParen, "战利品来源需要 `(`")?;
        let source = match method.as_str() {
            "table" => LootSource::Table(self.advancement_reference("战利品表")?),
            "kill" => LootSource::Kill(self.holder("战利品来源实体")?),
            "fish" => {
                let table = self.advancement_reference("战利品表")?;
                self.command_comma()?;
                let position = self.block_position("钓鱼位置")?;
                let tool = self.loot_tool()?;
                LootSource::Fish {
                    table,
                    position,
                    tool,
                }
            }
            "mine" => {
                let position = self.block_position("挖掘位置")?;
                let tool = self.loot_tool()?;
                LootSource::Mine { position, tool }
            }
            _ => return self.unknown_command_method("loot 来源", &method),
        };
        self.expect(TokenKind::RightParen, "战利品来源缺少 `)`")?;
        Ok(CoreCommand::Loot {
            target,
            source: Box::new(source),
        })
    }

    fn loot_tool(&mut self) -> Result<Option<LootTool>, Diagnostic> {
        if !self.command_optional_comma() {
            return Ok(None);
        }
        let (value, _) = self.ident("战利品工具物品定义或 mainhand/offhand")?;
        Ok(Some(match super::keywords::command_value(&value) {
            Some(hand @ ("mainhand" | "offhand")) => LootTool::Hand(hand.to_owned()),
            _ => LootTool::Item(value),
        }))
    }
}
