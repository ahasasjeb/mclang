use super::{Parser, keywords::word_matches};
use crate::ast::*;
use crate::diagnostic::Diagnostic;
use crate::lexer::TokenKind;

mod teams;

impl Parser {
    pub(in crate::parser) fn take_command_word(&mut self, word: &str) -> bool {
        if self.check_word(word)
            || matches!(&self.current().kind, TokenKind::Ident(value) if super::keywords::command_value(value) == Some(word))
        {
            self.advance();
            true
        } else {
            false
        }
    }

    fn expect_command_word(&mut self, word: &str) -> Result<(), Diagnostic> {
        if self.take_command_word(word) {
            Ok(())
        } else {
            Err(Diagnostic::new(
                format!("这里需要 `{word}`"),
                self.current().span,
            ))
        }
    }
    pub(in crate::parser) fn command_boolean(&mut self) -> Result<bool, Diagnostic> {
        let (value, span) = self.ident("布尔值 true/false")?;
        match super::keywords::boolean_word(&value) {
            Some("true") => Ok(true),
            Some("false") => Ok(false),
            _ => Err(Diagnostic::new(
                format!("`{value}` 不是布尔值，需要 true/真 或 false/假"),
                span,
            )),
        }
    }
    pub(super) fn entity_command_root(&self) -> Option<&'static str> {
        [
            "kill",
            "tag",
            "enchant",
            "damage",
            "attribute",
            "ride",
            "rotate",
            "spreadplayers",
            "spectate",
            "swing",
            "trigger",
            "gamemode",
            "defaultgamemode",
            "difficulty",
            "spawnpoint",
            "setworldspawn",
            "team",
            "waypoint",
            "list",
        ]
        .into_iter()
        .find(|root| self.check_word(root))
    }

    pub(super) fn entity_command(&mut self, root: &str) -> Result<EntityCommand, Diagnostic> {
        let method = if ["tag", "attribute", "ride", "rotate", "team", "waypoint"].contains(&root) {
            self.command_method()?
        } else {
            String::new()
        };
        self.expect(TokenKind::LeftParen, "命令调用需要 `(`")?;
        let command = match root {
            "kill" => EntityCommand::Kill(self.optional_holder()?),
            "tag" => {
                let target = self.holder("标签目标")?;
                let operation = match method.as_str() {
                    "add" | "remove" => {
                        self.command_comma()?;
                        let tag = self.string("实体标签")?.0;
                        if method == "add" {
                            TagOperation::Add(tag)
                        } else {
                            TagOperation::Remove(tag)
                        }
                    }
                    "list" => TagOperation::List,
                    _ => return self.unknown_command_method(root, &method),
                };
                EntityCommand::Tag { target, operation }
            }
            "enchant" => {
                let target = self.holder("附魔目标")?;
                self.command_comma()?;
                let enchantment = self.string("附魔资源位置")?.0;
                let level = if self.command_optional_comma() {
                    Some(self.unsigned("附魔等级")?)
                } else {
                    None
                };
                EntityCommand::Enchant {
                    target,
                    enchantment,
                    level,
                }
            }
            "damage" => self.damage_command()?,
            "attribute" => self.attribute_command(&method)?,
            "ride" => {
                let target = self.holder("乘骑目标")?;
                let vehicle = match method.as_str() {
                    "mount" => {
                        self.command_comma()?;
                        Some(self.holder("载具")?)
                    }
                    "dismount" => None,
                    _ => return self.unknown_command_method(root, &method),
                };
                EntityCommand::Ride { target, vehicle }
            }
            "rotate" => {
                let target = self.holder("旋转目标")?;
                self.command_comma()?;
                let facing = match method.as_str() {
                    "to" => Facing::Rotation(self.rotation_value("旋转朝向")?),
                    "facing" => self.command_facing()?,
                    _ => return self.unknown_command_method(root, &method),
                };
                EntityCommand::Rotate { target, facing }
            }
            "spreadplayers" => self.spread_command()?,
            "spectate" => {
                let target = self.optional_holder()?;
                let player = if self.command_optional_comma() {
                    Some(self.holder("旁观玩家")?)
                } else {
                    None
                };
                EntityCommand::Spectate { target, player }
            }
            "swing" => self.swing_command()?,
            "trigger" => {
                let objective = self.ident("trigger 目标")?.0;
                let operation = if self.command_optional_comma() {
                    let op = self.command_choice(&["add", "set"])?;
                    self.command_comma()?;
                    Some((op == "add", self.signed("触发计分值")?))
                } else {
                    None
                };
                EntityCommand::Trigger {
                    objective,
                    operation,
                }
            }
            "gamemode" => {
                let mode =
                    self.command_choice(&["survival", "creative", "adventure", "spectator"])?;
                let target = if self.command_optional_comma() {
                    Some(self.holder("游戏模式玩家")?)
                } else {
                    None
                };
                EntityCommand::GameMode { mode, target }
            }
            "defaultgamemode" => EntityCommand::DefaultGameMode(self.command_choice(&[
                "survival",
                "creative",
                "adventure",
                "spectator",
            ])?),
            "difficulty" => EntityCommand::Difficulty(if self.check(&TokenKind::RightParen) {
                None
            } else {
                Some(self.command_choice(&["peaceful", "easy", "normal", "hard"])?)
            }),
            "spawnpoint" | "setworldspawn" => self.spawnpoint_command(root == "spawnpoint")?,
            "team" => EntityCommand::Team(self.team_command(&method)?),
            "waypoint" => EntityCommand::Waypoint(self.waypoint_command(&method)?),
            "list" => EntityCommand::List {
                uuids: if self.check(&TokenKind::RightParen) {
                    false
                } else {
                    self.command_boolean()?
                },
            },
            _ => unreachable!("entity command dispatch"),
        };
        self.expect(TokenKind::RightParen, "命令调用缺少 `)`")?;
        Ok(command)
    }

    pub(in crate::parser) fn command_method(&mut self) -> Result<String, Diagnostic> {
        self.expect(TokenKind::Dot, "命令需要 `.方法`")?;
        let mut method = self.command_word()?;
        while self.take(&TokenKind::Dot).is_some() {
            method.push('_');
            method.push_str(&self.command_word()?);
        }
        Ok(method)
    }

    pub(in crate::parser) fn command_word(&mut self) -> Result<String, Diagnostic> {
        let (word, _) = self.ident("命令选项")?;
        if let Some(value) = super::keywords::command_value(&word) {
            return Ok(value.to_owned());
        }
        Ok(super::keywords::KEYWORDS
            .iter()
            .find(|k| word_matches(&word, k.english))
            .map_or(word.clone(), |k| k.english.to_owned()))
    }

    pub(in crate::parser) fn command_choice(
        &mut self,
        choices: &[&str],
    ) -> Result<String, Diagnostic> {
        let span = self.current().span;
        let value = self.command_word()?;
        if choices.contains(&value.as_str()) {
            Ok(value)
        } else {
            Err(Diagnostic::new(
                format!("未知选项 `{value}`，可用 {}", choices.join("、")),
                span,
            ))
        }
    }

    pub(in crate::parser) fn command_comma(&mut self) -> Result<(), Diagnostic> {
        self.expect(TokenKind::Comma, "命令参数之间需要 `,`")?;
        Ok(())
    }

    pub(in crate::parser) fn command_optional_comma(&mut self) -> bool {
        self.take(&TokenKind::Comma).is_some()
    }

    fn optional_holder(&mut self) -> Result<Option<Holder>, Diagnostic> {
        if self.check(&TokenKind::RightParen) {
            Ok(None)
        } else {
            self.holder("命令目标").map(Some)
        }
    }

    pub(in crate::parser) fn command_number(&mut self, label: &str) -> Result<String, Diagnostic> {
        self.signed_number_text(label)
    }

    pub(in crate::parser) fn unknown_command_method<T>(
        &self,
        root: &str,
        method: &str,
    ) -> Result<T, Diagnostic> {
        Err(Diagnostic::new(
            format!("未知 {root} 方法 `{method}`"),
            self.previous().span,
        ))
    }

    fn damage_command(&mut self) -> Result<EntityCommand, Diagnostic> {
        let target = self.holder("伤害目标")?;
        self.command_comma()?;
        let amount = self.command_number("伤害值")?;
        let damage_type = if self.command_optional_comma() {
            Some(self.string("伤害类型资源位置")?.0)
        } else {
            None
        };
        let source = if self.command_optional_comma() {
            if self.take_word("at").is_some() {
                Some(DamageOrigin::At(self.position_value("伤害位置")?))
            } else {
                self.expect_command_word("by")?;
                let entity = self.holder("直接伤害实体")?;
                let cause = if self.take_command_word("from") {
                    Some(self.holder("间接伤害实体")?)
                } else {
                    None
                };
                Some(DamageOrigin::By { entity, cause })
            }
        } else {
            None
        };
        Ok(EntityCommand::Damage {
            target,
            amount,
            damage_type,
            source,
        })
    }

    fn attribute_command(&mut self, method: &str) -> Result<EntityCommand, Diagnostic> {
        let target = self.holder("属性目标")?;
        self.command_comma()?;
        let attribute = self.string("属性资源位置")?.0;
        let operation = match method {
            "get" | "base_get" => {
                let scale = if self.command_optional_comma() {
                    Some(self.command_number("属性缩放")?)
                } else {
                    None
                };
                if method == "get" {
                    AttributeOperation::Get(scale)
                } else {
                    AttributeOperation::BaseGet(scale)
                }
            }
            "base_set" => {
                self.command_comma()?;
                AttributeOperation::BaseSet(self.command_number("属性基值")?)
            }
            "base_reset" => AttributeOperation::BaseReset,
            "modifier_add" => {
                self.command_comma()?;
                let id = self.string("修饰器资源位置")?.0;
                self.command_comma()?;
                let value = self.command_number("修饰值")?;
                self.command_comma()?;
                let operation = self.command_choice(&[
                    "add_value",
                    "add_multiplied_base",
                    "add_multiplied_total",
                ])?;
                AttributeOperation::ModifierAdd {
                    id,
                    value,
                    operation,
                }
            }
            "modifier_remove" => {
                self.command_comma()?;
                AttributeOperation::ModifierRemove(self.string("修饰器资源位置")?.0)
            }
            "modifier_get" | "modifier_value_get" => {
                self.command_comma()?;
                let id = self.string("修饰器资源位置")?.0;
                let scale = if self.command_optional_comma() {
                    Some(self.command_number("属性缩放")?)
                } else {
                    None
                };
                AttributeOperation::ModifierGet { id, scale }
            }
            _ => return self.unknown_command_method("attribute", method),
        };
        Ok(EntityCommand::Attribute {
            target,
            attribute,
            operation,
        })
    }

    pub(in crate::parser) fn command_facing(&mut self) -> Result<Facing, Diagnostic> {
        if self.check_word("pos") || self.check_word("vec3") || self.check_word("block_pos") {
            Ok(Facing::Position(self.position_value("朝向坐标")?))
        } else {
            let target = self.holder("朝向实体")?;
            let anchor = if self.command_optional_comma() {
                self.command_choice(&["feet", "eyes"])?
            } else {
                "feet".to_owned()
            };
            Ok(Facing::Entity { target, anchor })
        }
    }

    fn spread_command(&mut self) -> Result<EntityCommand, Diagnostic> {
        let center = self.vec2_value("散布中心")?;
        self.command_comma()?;
        let spread = self.command_number("散布间距")?;
        self.command_comma()?;
        let range = self.command_number("散布范围")?;
        self.command_comma()?;
        let teams = self.command_boolean()?;
        self.command_comma()?;
        let target = self.holder("散布目标")?;
        let under = if self.command_optional_comma() {
            self.expect_command_word("under")?;
            Some(self.signed("散布最高高度")?)
        } else {
            None
        };
        Ok(EntityCommand::Spread {
            center,
            spread,
            range,
            under,
            teams,
            target,
        })
    }

    fn swing_command(&mut self) -> Result<EntityCommand, Diagnostic> {
        let target = self.optional_holder()?;
        let hand = if self.command_optional_comma() {
            Some(self.command_choice(&["mainhand", "offhand"])?)
        } else {
            None
        };
        let animation = if hand.is_some() && self.command_optional_comma() {
            Some(self.command_choice(&["none", "whack", "stab"])?)
        } else {
            None
        };
        let duration = if animation.is_some() && self.command_optional_comma() {
            Some(self.time_argument(1, "挥动时长")?)
        } else {
            None
        };
        Ok(EntityCommand::Swing {
            target,
            hand,
            animation,
            duration,
        })
    }

    fn spawnpoint_command(&mut self, player: bool) -> Result<EntityCommand, Diagnostic> {
        let target = if player {
            self.optional_holder()?
        } else {
            None
        };
        let position = if player {
            if self.command_optional_comma() {
                Some(self.block_position("出生点")?)
            } else {
                None
            }
        } else if self.check(&TokenKind::RightParen) {
            None
        } else {
            Some(self.block_position("世界出生点")?)
        };
        let rotation = if position.is_some() && self.command_optional_comma() {
            Some(self.rotation_value("出生朝向")?)
        } else {
            None
        };
        Ok(if player {
            EntityCommand::SpawnPoint {
                target,
                position,
                rotation,
            }
        } else {
            EntityCommand::WorldSpawn { position, rotation }
        })
    }
}
