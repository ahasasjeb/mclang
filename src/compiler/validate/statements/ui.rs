use super::*;
use crate::ast::{
    BossBarAction, BossBarProperty, ParticleCommand, PostEffectAction, TitleAction, UiCommand,
};

mod particle;

pub(super) fn validate_ui_command(
    command: &UiCommand,
    span: Span,
    ctx: ValidationContext<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match command {
        UiCommand::Title { targets, action } => {
            player_target(targets, false, span, ctx, diagnostics);
            if let TitleAction::Text { component, .. } = action {
                crate::compiler::validate::components::validate_component(
                    component,
                    ctx,
                    diagnostics,
                );
            }
        }
        UiCommand::BossBar(action) => validate_bossbar(action, span, ctx, diagnostics),
        UiCommand::Dialog { targets, dialog } => {
            player_target(targets, false, span, ctx, diagnostics);
            if let Some(dialog) = dialog {
                if dialog.external {
                    validate_id("dialog", "对话框", &dialog.name, dialog.span, diagnostics);
                } else if !ctx.symbols.dialogs.contains(dialog.name.as_str()) {
                    diagnostics.push(Diagnostic::new(
                        format!("找不到 dialog 资源 `{}`", dialog.name),
                        dialog.span,
                    ));
                }
            }
        }
        UiCommand::Particle(particle) => validate_particle(particle, span, ctx, diagnostics),
        UiCommand::StopSound {
            targets,
            source,
            sound,
        } => {
            player_target(targets, false, span, ctx, diagnostics);
            if let Some(source) = source {
                validate_enum("sound_source", "声音分类", source, span, diagnostics);
            }
            if let Some(sound) = sound {
                validate_id("sound", "声音", sound, span, diagnostics);
            }
        }
        UiCommand::PostEffect(action) => match action {
            PostEffectAction::Add { targets, effect }
            | PostEffectAction::Remove { targets, effect } => {
                player_target(targets, false, span, ctx, diagnostics);
                validate_id("post_effect", "后处理效果", effect, span, diagnostics);
            }
            PostEffectAction::Clear(targets) => {
                player_target(targets, false, span, ctx, diagnostics)
            }
            PostEffectAction::List(target) => player_target(target, true, span, ctx, diagnostics),
        },
        UiCommand::PrivateMessage { targets, message } => {
            player_target(targets, false, span, ctx, diagnostics);
            validate_chat_message(message, span, diagnostics);
        }
        UiCommand::TeamMessage(message) => {
            if !ctx.context.is_entity() {
                diagnostics.push(Diagnostic::new("teammsg 需要实体执行上下文", span));
            }
            validate_chat_message(message, span, diagnostics);
        }
    }
}

fn player_target(
    target: &Holder,
    single: bool,
    span: Span,
    ctx: ValidationContext<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match target {
        Holder::SelfEntity => {
            if !ctx.context.satisfies(ExecutionContext::Player) {
                diagnostics.push(Diagnostic::new(
                    "目标 self/自身 需要 @player 玩家执行上下文",
                    span,
                ));
            }
        }
        Holder::Origin => diagnostics.push(Diagnostic::new(
            "界面命令不支持 origin/投掷者 目标；请在 execute on origin 块中使用 self",
            span,
        )),
        Holder::Query(name, query_span) => {
            if let Some(query) = require_player_query(name, *query_span, ctx, diagnostics)
                && single
                && query.limit != Some(1)
            {
                diagnostics.push(Diagnostic::new(
                    format!("目标查询 `{name}` 需要 limit(1) 的单个玩家"),
                    *query_span,
                ));
            }
        }
    }
}

fn validate_bossbar(
    action: &BossBarAction,
    span: Span,
    ctx: ValidationContext<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let id = match action {
        BossBarAction::Add { id, name } => {
            crate::compiler::validate::components::validate_component(name, ctx, diagnostics);
            id
        }
        BossBarAction::Remove(id) => id,
        BossBarAction::List => return,
        BossBarAction::Set { id, property } => {
            match property {
                BossBarProperty::Name(name) => {
                    crate::compiler::validate::components::validate_component(
                        name,
                        ctx,
                        diagnostics,
                    )
                }
                BossBarProperty::Color(color) => {
                    validate_enum("bossbar_color", "首领栏颜色", color, span, diagnostics)
                }
                BossBarProperty::Style(style) => {
                    validate_enum("bossbar_style", "首领栏样式", style, span, diagnostics)
                }
                BossBarProperty::Value(value) if *value > i32::MAX as u32 => {
                    diagnostics.push(Diagnostic::new("首领栏数值不能超过 2147483647", span))
                }
                BossBarProperty::Max(value) if *value == 0 || *value > i32::MAX as u32 => {
                    diagnostics.push(Diagnostic::new("首领栏最大值必须是 1 到 2147483647", span))
                }
                BossBarProperty::Players(Some(targets)) => {
                    player_target(targets, false, span, ctx, diagnostics)
                }
                _ => {}
            }
            id
        }
    };
    if !valid_resource_location(id) {
        diagnostics.push(Diagnostic::new(
            format!("`{id}` 不是有效的首领栏资源位置"),
            span,
        ));
    }
}

fn validate_particle(
    particle: &ParticleCommand,
    span: Span,
    ctx: ValidationContext<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    validate_id("particle", "粒子", &particle.name, span, diagnostics);
    particle::validate_options(particle, span, diagnostics);
    if let Some(position) = &particle.position {
        crate::compiler::validate::world::validate_position_value(position, diagnostics);
    }
    // `delta` 是扩散宽度，不是世界坐标；解析器已检查三个分量的数值形状。
    if let Some(speed) = &particle.speed
        && !speed
            .parse::<f64>()
            .is_ok_and(|value| value >= 0.0 && value <= f32::MAX as f64)
    {
        diagnostics.push(Diagnostic::new("粒子速度必须是非负的有限单精度数", span));
    }
    if particle.count.is_some_and(|count| count > i32::MAX as u32) {
        diagnostics.push(Diagnostic::new("粒子数量不能超过 2147483647", span));
    }
    if let Some(viewers) = &particle.viewers {
        player_target(viewers, false, span, ctx, diagnostics);
    }
}

fn validate_chat_message(message: &str, span: Span, diagnostics: &mut Vec<Diagnostic>) {
    if message.trim().is_empty() || message.contains(['\n', '\r']) {
        diagnostics.push(Diagnostic::new("聊天消息必须是非空的单行文本", span));
    }
}
