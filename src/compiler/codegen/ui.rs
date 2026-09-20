use crate::ast::{
    BossBarAction, BossBarProperty, ParticleCommand, PostEffectAction, TitleAction, TitleChannel,
    UiCommand,
};

use super::Compiler;
use super::emit::{nbt_text, reference_id};
use super::world;

impl Compiler<'_> {
    pub(super) fn compile_ui_command(&self, command: &UiCommand) -> String {
        match command {
            UiCommand::Title { targets, action } => {
                let target = self.component_holder(targets);
                match action {
                    TitleAction::Text { channel, component } => {
                        let channel = match channel {
                            TitleChannel::Title => "title",
                            TitleChannel::Subtitle => "subtitle",
                            TitleChannel::Actionbar => "actionbar",
                        };
                        format!(
                            "title {target} {channel} {}",
                            self.component_json(component)
                        )
                    }
                    TitleAction::Times {
                        fade_in,
                        stay,
                        fade_out,
                    } => {
                        format!("title {target} times {fade_in} {stay} {fade_out}")
                    }
                    TitleAction::Clear => format!("title {target} clear"),
                    TitleAction::Reset => format!("title {target} reset"),
                }
            }
            UiCommand::BossBar(action) => self.compile_bossbar(action),
            UiCommand::Dialog { targets, dialog } => {
                let target = self.component_holder(targets);
                match dialog {
                    Some(dialog) => format!(
                        "dialog show {target} {}",
                        reference_id(&self.program.namespace, dialog)
                    ),
                    None => format!("dialog clear {target}"),
                }
            }
            UiCommand::Particle(particle) => self.compile_particle(particle),
            UiCommand::StopSound {
                targets,
                source,
                sound,
            } => {
                let mut command = format!("stopsound {}", self.component_holder(targets));
                if source.is_some() || sound.is_some() {
                    command.push_str(&format!(" {}", source.as_deref().unwrap_or("*")));
                }
                if let Some(sound) = sound {
                    command.push_str(&format!(" {sound}"));
                }
                command
            }
            UiCommand::PostEffect(action) => match action {
                PostEffectAction::Add { targets, effect } => {
                    format!("posteffect add {} {effect}", self.component_holder(targets))
                }
                PostEffectAction::Clear(targets) => {
                    format!("posteffect clear {}", self.component_holder(targets))
                }
                PostEffectAction::List(target) => {
                    format!("posteffect list {}", self.component_holder(target))
                }
                PostEffectAction::Remove { targets, effect } => format!(
                    "posteffect remove {} {effect}",
                    self.component_holder(targets)
                ),
            },
            UiCommand::PrivateMessage { targets, message } => {
                format!("msg {} {message}", self.component_holder(targets))
            }
            UiCommand::TeamMessage(message) => format!("teammsg {message}"),
        }
    }

    fn compile_bossbar(&self, action: &BossBarAction) -> String {
        match action {
            BossBarAction::Add { id, name } => {
                format!("bossbar add {id} {}", self.component_json(name))
            }
            BossBarAction::Remove(id) => format!("bossbar remove {id}"),
            BossBarAction::List => "bossbar list".to_owned(),
            BossBarAction::Set { id, property } => {
                let value = match property {
                    BossBarProperty::Name(name) => format!("name {}", self.component_json(name)),
                    BossBarProperty::Color(color) => format!("color {color}"),
                    BossBarProperty::Style(style) => format!("style {style}"),
                    BossBarProperty::Value(value) => format!("value {value}"),
                    BossBarProperty::Max(max) => format!("max {max}"),
                    BossBarProperty::Visible(visible) => format!("visible {visible}"),
                    BossBarProperty::Players(targets) => match targets {
                        Some(targets) => format!("players {}", self.component_holder(targets)),
                        None => "players".to_owned(),
                    },
                };
                format!("bossbar set {id} {value}")
            }
        }
    }

    fn compile_particle(&self, particle: &ParticleCommand) -> String {
        let mut command = format!("particle {}", particle.name);
        if let Some(options) = &particle.options {
            command.push_str(&nbt_text(options));
        }
        if let Some(position) = &particle.position {
            command.push_str(&format!(" {}", world::position_value_text(position)));
        }
        if let Some(delta) = &particle.delta {
            command.push_str(&format!(
                " {} {} {}",
                world::vec3_text(delta),
                particle.speed.as_deref().unwrap_or("0"),
                particle.count.unwrap_or(0)
            ));
        }
        if let Some(force) = particle.force {
            command.push_str(if force { " force" } else { " normal" });
        }
        if let Some(viewers) = &particle.viewers {
            command.push_str(&format!(" {}", self.component_holder(viewers)));
        }
        command
    }
}
