use crate::ast::*;
use crate::diagnostic::Diagnostic;
use crate::lexer::TokenKind;

use crate::parser::Parser;
use crate::parser::keywords::{message_target, sound_source, text_color, time_unit, word_matches};

impl Parser {
    pub(super) fn message_statement(&mut self) -> Result<StatementKind, Diagnostic> {
        self.expect(TokenKind::Dot, "message 后需要 `.`")?;
        let (method, span) = self.ident("消息目标")?;
        let Some(method_kind) = message_target(&method) else {
            return Err(Diagnostic::new(
                "消息目标只能是 all、self、nearest 或 player",
                span,
            ));
        };
        self.expect(TokenKind::LeftParen, "消息目标后需要 `(`")?;
        let (target, component) = match method_kind {
            "all" | "self" => {
                let target = if method_kind == "all" {
                    MessageTarget::All
                } else {
                    MessageTarget::SelfEntity
                };
                (target, self.text_component_or_string("消息内容")?)
            }
            "nearest" => {
                let within = self.unsigned("message.nearest 范围")?;
                self.expect(TokenKind::Comma, "范围后需要 `,`")?;
                let component = self.text_component_or_string("消息内容")?;
                (MessageTarget::Nearest { within }, component)
            }
            "player" => {
                let (name, name_span) = self.ident("message.player 需要实体查询名称")?;
                self.expect(TokenKind::Comma, "查询后需要 `,`")?;
                let component = self.text_component_or_string("消息内容")?;
                (MessageTarget::Query { name, name_span }, component)
            }
            _ => unreachable!(),
        };
        // 兼容旧写法：纯文本消息后面可以再跟一个颜色标识符。
        let component = if self.take(&TokenKind::Comma).is_some() {
            let (color, color_span) = self.ident("消息颜色")?;
            if !matches!(component.kind, TextComponentKind::Text(_)) {
                return Err(Diagnostic::new(
                    "结构化文本组件请在样式块里写 color，例如 text(\"你好\") { color = \"red\"; }",
                    color_span,
                ));
            }
            let color = text_color(&color).unwrap_or(&color).to_owned();
            TextComponent {
                style: TextStyle {
                    color: Some(color),
                    ..component.style
                },
                ..component
            }
        } else {
            component
        };
        self.expect(TokenKind::RightParen, "消息调用缺少 `)`")?;
        self.expect(TokenKind::Semicolon, "消息调用后需要 `;`")?;
        Ok(StatementKind::Message { target, component })
    }

    pub(super) fn sound_statement(&mut self) -> Result<StatementKind, Diagnostic> {
        self.expect(TokenKind::Dot, "sound 后需要 `.`")?;
        let (method, span) = self.ident("声音方法")?;
        if word_matches(&method, "self") {
            self.expect(TokenKind::LeftParen, "sound.self 后需要 `(`")?;
            let (sound, _) = self.string("sound.self 需要声音资源位置")?;
            self.expect(TokenKind::Comma, "声音资源位置后需要 `,`")?;
            let (source, _) = self.ident("声音分类")?;
            let source = sound_source(&source).unwrap_or(&source).to_owned();
            self.expect(TokenKind::RightParen, "声音调用缺少 `)`")?;
            self.expect(TokenKind::Semicolon, "声音调用后需要 `;`")?;
            return Ok(StatementKind::PlaySound {
                sound,
                source,
                targets: None,
                position: None,
                volume: None,
                pitch: None,
                min_volume: None,
            });
        }
        if !word_matches(&method, "play") {
            return Err(Diagnostic::new(
                "声音方法只能是 self（中文 自身）或 play（中文 播放）",
                span,
            ));
        }
        self.expect(TokenKind::LeftParen, "sound.play 后需要 `(`")?;
        let (sound, _) = self.string("sound.play 需要声音资源位置")?;
        self.expect(TokenKind::Comma, "声音资源位置后需要 `,`")?;
        let (source, _) = self.ident("声音分类")?;
        let source = sound_source(&source).unwrap_or(&source).to_owned();
        self.expect(TokenKind::Comma, "声音分类后需要 `,`")?;
        let (targets, _) = self.ident("sound.play 需要玩家查询名称")?;
        let mut position = None;
        let mut volume = None;
        let mut pitch = None;
        let mut min_volume = None;
        if self.take(&TokenKind::Comma).is_some() {
            if self.check_word("pos") || self.check_word("block_pos") || self.check_word("vec3") {
                position = Some(self.position_value("声音播放坐标")?);
                if self.take(&TokenKind::Comma).is_some() {
                    volume = Some(self.signed_number_text("声音音量")?);
                }
            } else {
                // 省略坐标时可以直接写音量。
                volume = Some(self.signed_number_text("声音音量")?);
            }
        }
        if volume.is_some() && self.take(&TokenKind::Comma).is_some() {
            pitch = Some(self.signed_number_text("声音音调")?);
        }
        if pitch.is_some() && self.take(&TokenKind::Comma).is_some() {
            min_volume = Some(self.signed_number_text("声音最小音量")?);
        }
        self.expect(TokenKind::RightParen, "sound.play 调用缺少 `)`")?;
        self.expect(TokenKind::Semicolon, "声音调用后需要 `;`")?;
        Ok(StatementKind::PlaySound {
            sound,
            source,
            targets: Some(targets),
            position,
            volume,
            pitch,
            min_volume,
        })
    }

    pub(super) fn schedule_statement(&mut self) -> Result<StatementKind, Diagnostic> {
        if self.take(&TokenKind::Dot).is_some() {
            return self.schedule_clear_statement();
        }
        let target = self.call_target("被调度函数名称")?;
        self.empty_arguments()?;
        self.expect_word("after")?;
        let delay = self.time_argument(1, "调度延迟")?;
        let mode = if self.take_word("append").is_some() {
            ScheduleMode::Append
        } else {
            self.take_word("replace");
            ScheduleMode::Replace
        };
        self.expect(TokenKind::Semicolon, "调度语句后需要 `;`")?;
        Ok(StatementKind::Schedule {
            target,
            delay,
            mode,
        })
    }

    /// `schedule.clear(函数)`：取消尚未执行的同名调度。
    ///
    /// 26.3 的 `schedule clear` 只接受普通资源位置，标签调度无法取消，因此这里也
    /// 只允许函数名。
    pub(super) fn schedule_clear_statement(&mut self) -> Result<StatementKind, Diagnostic> {
        let (method, method_span) = self.ident("schedule 方法")?;
        if !word_matches(&method, "clear") {
            return Err(Diagnostic::new(
                "schedule 只支持 `函数() after <时间>` 或 `schedule.clear(函数)`",
                method_span,
            ));
        }
        self.expect(TokenKind::LeftParen, "schedule.clear 后需要 `(`")?;
        let (function, _) = self.ident("被清除调度的函数名称")?;
        self.expect(TokenKind::RightParen, "schedule.clear 缺少 `)`")?;
        self.expect(TokenKind::Semicolon, "schedule.clear 后需要 `;`")?;
        Ok(StatementKind::ScheduleClear { function })
    }

    /// 读取 `[符号]<数值><单位>`，换算为游戏刻并生成规范化文本。
    ///
    /// 原版 `TimeArgument` 使用浮点数并按单位四舍五入，因此 `1.5 s` 与 `30 t`
    /// 等价；这里沿用同一换算，并在编译期按 `minimum` 拒绝过小或超出 32 位范围的
    /// 时间。调度、天气持续时间和世界时钟命令共用本函数。
    pub(in crate::parser) fn time_argument(
        &mut self,
        minimum: i32,
        label: &str,
    ) -> Result<String, Diagnostic> {
        let negative = self.negative_sign();
        let token = self.advance().clone();
        let (number, text) = match token.kind {
            TokenKind::Number(value) => {
                let signed = if negative { -value } else { value };
                if !(i64::from(minimum)..=i64::from(i32::MAX)).contains(&signed) {
                    return Err(Diagnostic::new(
                        format!("{label}必须是 {minimum} 到 2147483647 之间的整数"),
                        token.span,
                    ));
                }
                (signed as f64, signed.to_string())
            }
            TokenKind::Decimal(value) => {
                let value = if negative { -value } else { value };
                if !value.is_finite() {
                    return Err(Diagnostic::new(
                        format!("{label}必须是有限数字"),
                        token.span,
                    ));
                }
                (value, format!("{value}"))
            }
            _ => {
                return Err(Diagnostic::new(
                    format!("{label}需要数字，例如 `1 s` 或 `1.5 s`"),
                    token.span,
                ));
            }
        };
        // 括号调用里写作 `time.set(1000, t)`，schedule 里写作 `after 20 t`；
        // 两种写法都接受一个可选逗号。
        self.take(&TokenKind::Comma);
        let (unit, unit_span) = self.ident("时间单位 t、s 或 d")?;
        let Some(unit) = time_unit(&unit) else {
            return Err(Diagnostic::new("时间单位只能是 t、s 或 d", unit_span));
        };
        let factor = match unit {
            "t" => 1.0,
            "s" => 20.0,
            "d" => 24000.0,
            _ => unreachable!("time_unit 只返回 t、s、d"),
        };
        // Java 的 Math.round 对负数同样向正无穷取半，`(x + 0.5).floor()` 与它一致。
        let ticks = (number * factor + 0.5).floor();
        if ticks < f64::from(minimum) {
            return Err(Diagnostic::new(
                format!("{label}至少为 {minimum} 游戏刻"),
                token.span.merge(unit_span),
            ));
        }
        if ticks > f64::from(i32::MAX) {
            return Err(Diagnostic::new(
                format!("{label}超出 32 位游戏刻范围"),
                token.span.merge(unit_span),
            ));
        }
        Ok(format!("{text}{unit}"))
    }

    pub(in crate::parser) fn call_target(&mut self, label: &str) -> Result<CallTarget, Diagnostic> {
        if self.take(&TokenKind::Hash).is_some() {
            let (name, _) = self.ident("函数标签名称")?;
            Ok(CallTarget::Tag(name))
        } else {
            let (name, _) = self.ident(label)?;
            Ok(CallTarget::Function(name))
        }
    }
}
