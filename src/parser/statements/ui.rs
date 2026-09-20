use crate::ast::*;
use crate::diagnostic::Diagnostic;
use crate::lexer::TokenKind;

use crate::parser::Parser;

impl Parser {
    pub(super) fn ui_command_root(&self) -> Option<&'static str> {
        [
            "title",
            "bossbar",
            "dialog",
            "particle",
            "stopsound",
            "posteffect",
            "msg",
            "teammsg",
        ]
        .into_iter()
        .find(|root| self.check_word(root))
    }

    pub(super) fn ui_command(&mut self, root: &str) -> Result<UiCommand, Diagnostic> {
        let method = if matches!(root, "title" | "bossbar" | "dialog" | "posteffect") {
            self.command_method()?
        } else {
            String::new()
        };
        self.expect(TokenKind::LeftParen, "界面命令需要 `(`")?;
        let command = match root {
            "title" => self.title_command(&method)?,
            "bossbar" => self.bossbar_command(&method)?,
            "dialog" => self.dialog_command(&method)?,
            "particle" => UiCommand::Particle(Box::new(self.particle_command()?)),
            "stopsound" => self.stopsound_command()?,
            "posteffect" => self.posteffect_command(&method)?,
            "msg" => {
                let targets = self.holder("私聊目标")?;
                self.command_comma()?;
                let message = self.string("私聊消息")?.0;
                UiCommand::PrivateMessage { targets, message }
            }
            "teammsg" => UiCommand::TeamMessage(self.string("队伍消息")?.0),
            _ => unreachable!(),
        };
        self.expect(TokenKind::RightParen, "界面命令缺少 `)`")?;
        Ok(command)
    }

    fn title_command(&mut self, method: &str) -> Result<UiCommand, Diagnostic> {
        let targets = self.holder("标题目标")?;
        let action = match method {
            "title" | "subtitle" | "actionbar" => {
                self.command_comma()?;
                let component = self.text_component_or_string("标题文本")?;
                let channel = match method {
                    "title" => TitleChannel::Title,
                    "subtitle" => TitleChannel::Subtitle,
                    _ => TitleChannel::Actionbar,
                };
                TitleAction::Text {
                    channel,
                    component: Box::new(component),
                }
            }
            "times" => {
                self.command_comma()?;
                let fade_in = self.time_argument(0, "标题淡入时间")?;
                self.command_comma()?;
                let stay = self.time_argument(0, "标题停留时间")?;
                self.command_comma()?;
                let fade_out = self.time_argument(0, "标题淡出时间")?;
                TitleAction::Times {
                    fade_in,
                    stay,
                    fade_out,
                }
            }
            "clear" => TitleAction::Clear,
            "reset" => TitleAction::Reset,
            _ => return self.unknown_command_method("title", method),
        };
        Ok(UiCommand::Title { targets, action })
    }

    fn bossbar_command(&mut self, method: &str) -> Result<UiCommand, Diagnostic> {
        let action = match method {
            "add" => {
                let id = self.string("首领栏资源位置")?.0;
                self.command_comma()?;
                let name = self.text_component_or_string("首领栏名称")?;
                BossBarAction::Add {
                    id,
                    name: Box::new(name),
                }
            }
            "remove" => BossBarAction::Remove(self.string("首领栏资源位置")?.0),
            "list" => BossBarAction::List,
            "set_name" | "set_color" | "set_style" | "set_value" | "set_max" | "set_visible"
            | "set_players" => {
                let id = self.string("首领栏资源位置")?.0;
                let property = if method == "set_players" && self.check(&TokenKind::RightParen) {
                    BossBarProperty::Players(None)
                } else {
                    self.command_comma()?;
                    match method {
                        "set_name" => BossBarProperty::Name(Box::new(
                            self.text_component_or_string("首领栏名称")?,
                        )),
                        "set_color" => BossBarProperty::Color(self.command_word()?),
                        "set_style" => BossBarProperty::Style(self.command_word()?),
                        "set_value" => BossBarProperty::Value(self.unsigned("首领栏数值")?),
                        "set_max" => BossBarProperty::Max(self.unsigned("首领栏最大值")?),
                        "set_visible" => BossBarProperty::Visible(self.command_boolean()?),
                        "set_players" => BossBarProperty::Players(Some(self.holder("首领栏玩家")?)),
                        _ => unreachable!(),
                    }
                };
                BossBarAction::Set { id, property }
            }
            _ => return self.unknown_command_method("bossbar", method),
        };
        Ok(UiCommand::BossBar(action))
    }

    fn dialog_command(&mut self, method: &str) -> Result<UiCommand, Diagnostic> {
        let targets = self.holder("对话框目标")?;
        let dialog = match method {
            "show" => {
                self.command_comma()?;
                Some(self.advancement_reference("对话框")?)
            }
            "clear" => None,
            _ => return self.unknown_command_method("dialog", method),
        };
        Ok(UiCommand::Dialog { targets, dialog })
    }

    fn particle_command(&mut self) -> Result<ParticleCommand, Diagnostic> {
        let name = self.string("粒子资源位置")?.0;
        let mut options = None;
        let mut position = None;
        let mut delta = None;
        let mut speed = None;
        let mut count = None;
        let mut force = None;
        let mut viewers = None;
        if self.command_optional_comma() {
            if self.check_word("nbt") {
                options = Some(self.nbt_compound_with_aliases("粒子选项")?);
                if self.command_optional_comma() {
                    position = Some(self.position_value("粒子坐标")?);
                }
            } else {
                position = Some(self.position_value("粒子坐标")?);
            }
        }
        if position.is_some() && self.command_optional_comma() {
            delta = Some(self.vec3_value("粒子偏移")?);
            self.command_comma()?;
            speed = Some(self.signed_number_text("粒子速度")?);
            self.command_comma()?;
            count = Some(self.unsigned("粒子数量")?);
            if self.command_optional_comma() {
                force = Some(self.command_choice(&["force", "normal"])? == "force");
                if self.command_optional_comma() {
                    viewers = Some(self.holder("粒子观众")?);
                }
            }
        }
        Ok(ParticleCommand {
            name,
            options,
            position,
            delta,
            speed,
            count,
            force,
            viewers,
        })
    }

    fn stopsound_command(&mut self) -> Result<UiCommand, Diagnostic> {
        let targets = self.holder("停止声音目标")?;
        let mut source = None;
        let mut sound = None;
        if self.command_optional_comma() {
            if self.take(&TokenKind::Star).is_none() {
                let source_word = self.command_word()?;
                source = Some(
                    crate::parser::keywords::sound_source(&source_word)
                        .unwrap_or(&source_word)
                        .to_owned(),
                );
            }
            if self.command_optional_comma() {
                sound = Some(self.string("声音资源位置")?.0);
            }
        }
        Ok(UiCommand::StopSound {
            targets,
            source,
            sound,
        })
    }

    fn posteffect_command(&mut self, method: &str) -> Result<UiCommand, Diagnostic> {
        let targets = self.holder("后处理效果目标")?;
        let action = match method {
            "add" | "remove" => {
                self.command_comma()?;
                let effect = self.string("后处理效果资源位置")?.0;
                if method == "add" {
                    PostEffectAction::Add { targets, effect }
                } else {
                    PostEffectAction::Remove { targets, effect }
                }
            }
            "clear" => PostEffectAction::Clear(targets),
            "list" => PostEffectAction::List(targets),
            _ => return self.unknown_command_method("posteffect", method),
        };
        Ok(UiCommand::PostEffect(action))
    }
}
