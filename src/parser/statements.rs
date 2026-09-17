//! 函数体语句：控制流、结构化实体操作、调用、赋值与返回。

use crate::ast::*;
use crate::diagnostic::Diagnostic;
use crate::lexer::TokenKind;

use super::Parser;
use super::keywords::{
    advancement_method, boolean_word, data_method, effect_method, item_method, message_target,
    score_operation, scoreboard_method, self_method, sound_source, stopwatch_method, text_color,
    time_unit, word_matches, xp_kind, xp_method,
};

impl Parser {
    pub(super) fn block(&mut self) -> Result<(Vec<Statement>, Span), Diagnostic> {
        self.expect(TokenKind::LeftBrace, "这里需要 `{`")?;
        let mut statements = Vec::new();
        while !self.check(&TokenKind::RightBrace) {
            if self.check(&TokenKind::Eof) {
                return Err(Diagnostic::new("代码块缺少 `}`", self.current().span));
            }
            statements.push(self.statement()?);
        }
        let end = self.advance().span;
        Ok((statements, end))
    }

    fn statement(&mut self) -> Result<Statement, Diagnostic> {
        let start = self.current().span;
        let kind = if self.take_word("each").is_some() {
            self.expect(TokenKind::LeftParen, "each 后需要 `(`")?;
            let (query, _) = self.ident("实体查询名称")?;
            self.expect(TokenKind::RightParen, "查询名称后需要 `)`")?;
            let (body, _) = self.block()?;
            StatementKind::Each { query, body }
        } else if self.take_word("give").is_some() {
            self.give_statement()?
        } else if self.take_word("in_dimension").is_some() {
            self.expect(TokenKind::LeftParen, "in_dimension 后需要 `(`")?;
            let (dimension, _) = self.string("in_dimension 需要维度资源位置")?;
            self.expect(TokenKind::RightParen, "维度资源位置后需要 `)`")?;
            let (body, _) = self.block()?;
            StatementKind::InDimension { dimension, body }
        } else if self.take_word("spawn").is_some() {
            self.expect(TokenKind::LeftParen, "spawn 后需要 `(`")?;
            let (entity_type, _) = self.string("spawn 需要实体类型资源位置")?;
            let position = if self.take(&TokenKind::Comma).is_some() {
                Some(self.position_value("spawn 召唤坐标")?)
            } else {
                None
            };
            self.expect(TokenKind::RightParen, "实体类型后需要 `)`")?;
            let (body, _) = self.block()?;
            StatementKind::Spawn {
                entity_type,
                position,
                body,
            }
        } else if self.take_word("self").is_some() {
            StatementKind::SelfAction(self.self_action()?)
        } else if self.take_word("message").is_some() {
            self.message_statement()?
        } else if self.take_word("sound").is_some() {
            self.sound_statement()?
        } else if self.take_word("run").is_some() {
            let (command, _) = self.command_string("run")?;
            self.expect(TokenKind::Semicolon, "命令后需要 `;`")?;
            StatementKind::Run(command)
        } else if self.take_word("call").is_some() {
            let target = self.call_target("被调用函数名称")?;
            let arguments = self.call_arguments()?;
            self.expect(TokenKind::Semicolon, "函数调用后需要 `;`")?;
            StatementKind::Call { target, arguments }
        } else if self.take_word("schedule").is_some() {
            self.schedule_statement()?
        } else if self.take_word("if").is_some() {
            let condition = self.condition()?;
            let (then_body, _) = self.block()?;
            let else_body = if self.take_word("else").is_some() {
                self.block()?.0
            } else {
                Vec::new()
            };
            StatementKind::If {
                condition,
                then_body,
                else_body,
            }
        } else if self.take_word("while").is_some() {
            let condition = self.condition()?;
            let (body, _) = self.block()?;
            StatementKind::While { condition, body }
        } else if self.take_word("execute").is_some() {
            let (clauses, span) = self.string("execute 后需要子句字符串")?;
            if clauses.trim().is_empty()
                || clauses.contains(['\n', '\r'])
                || clauses.starts_with("execute ")
                || clauses.starts_with("run ")
                || clauses.ends_with(" run")
            {
                return Err(Diagnostic::new(
                    "execute 字符串应只包含子句，例如 `as @a at @s`",
                    span,
                ));
            }
            let (body, _) = self.block()?;
            StatementKind::Execute { clauses, body }
        } else if self.take_word("return").is_some() {
            let kind = if self.take(&TokenKind::Semicolon).is_some() {
                ReturnKind::Void
            } else if self.take_word("fail").is_some() {
                self.expect(TokenKind::Semicolon, "return fail 后需要 `;`")?;
                ReturnKind::Fail
            } else if self.take_word("run").is_some() {
                let (command, _) = self.command_string("return run")?;
                self.expect(TokenKind::Semicolon, "return run 后需要 `;`")?;
                ReturnKind::Run(command)
            } else {
                let value = self.expression()?;
                self.expect(TokenKind::Semicolon, "return 表达式后需要 `;`")?;
                ReturnKind::Value(value)
            };
            StatementKind::Return(kind)
        } else if self.take_word("let").is_some() {
            let (name, name_span) = self.ident("局部变量名称")?;
            self.expect(TokenKind::Equal, "局部变量需要初始值")?;
            let value = self.expression()?;
            self.expect(TokenKind::Semicolon, "局部变量声明后需要 `;`")?;
            StatementKind::Let {
                name,
                name_span,
                value,
            }
        } else if self.take_word("effect").is_some() {
            self.effect_statement()?
        } else if self.take_word("xp").is_some() {
            self.xp_statement()?
        } else if self.take_word("clear").is_some() {
            self.clear_statement()?
        } else if self.take_word("stopwatch").is_some() {
            self.stopwatch_statement()?
        } else if self.take_word("scoreboard").is_some() {
            self.scoreboard_statement()?
        } else if self.take_word("teleport").is_some() {
            self.teleport_statement()?
        } else if self.take_word("set_block").is_some() {
            self.set_block_statement()?
        } else if self.take_word("fill_biome").is_some() {
            self.fill_biome_statement()?
        } else if self.take_word("fill").is_some() {
            self.fill_statement()?
        } else if self.take_word("clone").is_some() {
            self.clone_statement()?
        } else if self.take_word("place").is_some() {
            self.place_statement()?
        } else if self.take_word("forceload").is_some() {
            self.forceload_statement()?
        } else if self.take_word("time").is_some() {
            self.time_statement()?
        } else if self.take_word("weather").is_some() {
            self.weather_statement()?
        } else if self.take_word("gamerule").is_some() {
            self.gamerule_statement()?
        } else if self.take_word("worldborder").is_some() {
            self.worldborder_statement()?
        } else if self.take_word("locate").is_some() {
            self.locate_statement()?
        } else if self.take_word("advancement").is_some() {
            self.advancement_statement()?
        } else if self.check_word("data") && matches!(self.peek_kind(1).kind, TokenKind::Dot) {
            self.take_word("data");
            self.data_statement()?
        } else if self.check_word("item") && matches!(self.peek_kind(1).kind, TokenKind::Dot) {
            self.take_word("item");
            self.item_statement()?
        } else if self.check_word("nbt") {
            let nbt = self.nbt_compound_with_aliases("nbt 语句")?;
            // 块风格语句，结尾分号可选；物品属性里的 `custom_data = nbt {...};` 仍需要分号。
            self.take(&TokenKind::Semicolon);
            StatementKind::NbtMerge { nbt }
        } else {
            let (name, _) = self.ident("语句")?;
            if self.check(&TokenKind::LeftParen) {
                let arguments = self.call_arguments()?;
                self.expect(TokenKind::Semicolon, "函数调用后需要 `;`")?;
                StatementKind::Call {
                    target: CallTarget::Function(name),
                    arguments,
                }
            } else {
                let operation = self.assignment_operator()?;
                let value = self.expression()?;
                self.expect(TokenKind::Semicolon, "赋值后需要 `;`")?;
                StatementKind::Assign {
                    target: name,
                    operation,
                    value,
                }
            }
        };
        let end = self.previous().span;
        Ok(Statement {
            kind,
            span: start.merge(end),
        })
    }

    fn give_statement(&mut self) -> Result<StatementKind, Diagnostic> {
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

    fn optional_count(&mut self, name: &str) -> Result<(Option<u32>, Option<Span>), Diagnostic> {
        if self.take(&TokenKind::Comma).is_none() {
            return Ok((None, None));
        }
        let (count, span) = self.unsigned_with_span(name)?;
        Ok((Some(count), Some(span)))
    }

    fn self_action(&mut self) -> Result<SelfAction, Diagnostic> {
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

    fn message_statement(&mut self) -> Result<StatementKind, Diagnostic> {
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

    fn sound_statement(&mut self) -> Result<StatementKind, Diagnostic> {
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

    fn schedule_statement(&mut self) -> Result<StatementKind, Diagnostic> {
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
    fn schedule_clear_statement(&mut self) -> Result<StatementKind, Diagnostic> {
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
    pub(super) fn time_argument(
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

    pub(super) fn call_target(&mut self, label: &str) -> Result<CallTarget, Diagnostic> {
        if self.take(&TokenKind::Hash).is_some() {
            let (name, _) = self.ident("函数标签名称")?;
            Ok(CallTarget::Tag(name))
        } else {
            let (name, _) = self.ident(label)?;
            Ok(CallTarget::Function(name))
        }
    }

    fn effect_statement(&mut self) -> Result<StatementKind, Diagnostic> {
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

    fn xp_statement(&mut self) -> Result<StatementKind, Diagnostic> {
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

    fn clear_statement(&mut self) -> Result<StatementKind, Diagnostic> {
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

    /// `scoreboard.set(持有者, 目标, 值);` 与 `scoreboard.reset(持有者, 目标);`
    ///
    /// `scoreboard.get` 有返回值，只能在表达式里使用，语句形式会给出引导性诊断。
    /// `item.replace/fill/override/modify(...)`。
    fn item_statement(&mut self) -> Result<StatementKind, Diagnostic> {
        self.expect(TokenKind::Dot, "item 后需要 `.`")?;
        let (method, method_span) = self.ident("item 方法")?;
        let Some(method) = item_method(&method) else {
            return Err(Diagnostic::new(
                format!(
                    "未知 item 方法 `{method}`，可用 replace/替换、fill/填充、override/覆盖、modify/修改"
                ),
                method_span,
            ));
        };
        let method = match method {
            "replace" => ItemMethod::Replace,
            "fill" => ItemMethod::Fill,
            "override" => ItemMethod::Override,
            _ => ItemMethod::Modify,
        };
        self.expect(TokenKind::LeftParen, "item 方法后需要 `(`")?;
        let target = self.item_condition_source("item 目标")?;
        self.expect(TokenKind::Comma, "item 目标后需要 `,`")?;
        let (slots, slots_span) = self.string("item 需要槽位字符串")?;
        self.expect(TokenKind::Comma, "槽位后需要 `,`")?;
        let action = if method == ItemMethod::Modify {
            let (modifier, span) = self.string("item.modify 需要修饰器资源位置字符串")?;
            ItemActionKind::Modifier(modifier, span)
        } else {
            let (kind, kind_span) = self.ident("item 内容 with 或 from")?;
            match kind.as_str() {
                "with" | "用" => {
                    self.expect(TokenKind::LeftParen, "with 后需要 `(`")?;
                    let (item, span) = self.ident("with 需要已声明的物品名称")?;
                    self.expect(TokenKind::RightParen, "with 缺少 `)`")?;
                    ItemActionKind::With(item, span)
                }
                "from" | "来源" => {
                    self.expect(TokenKind::LeftParen, "from 后需要 `(`")?;
                    let source = self.item_condition_source("from 来源")?;
                    self.expect(TokenKind::Comma, "from 来源后需要 `,`")?;
                    let (source_slots, source_slots_span) =
                        self.string("from 需要来源槽位字符串")?;
                    let modifier = if self.take(&TokenKind::Comma).is_some() {
                        Some(self.string("from 修饰器资源位置字符串")?.0)
                    } else {
                        None
                    };
                    self.expect(TokenKind::RightParen, "from 缺少 `)`")?;
                    ItemActionKind::From {
                        source,
                        slots: source_slots,
                        slots_span: source_slots_span,
                        modifier,
                    }
                }
                _ => {
                    return Err(Diagnostic::new(
                        format!("item 内容只能是 with/用 或 from/来源，实际为 `{kind}`"),
                        kind_span,
                    ));
                }
            }
        };
        self.expect(TokenKind::RightParen, "item 调用缺少 `)`")?;
        self.expect(TokenKind::Semicolon, "item 调用后需要 `;`")?;
        Ok(StatementKind::ItemAction {
            method,
            target,
            slots,
            slots_span,
            action,
        })
    }

    /// `data.merge/remove/modify`；`data.get` 只能出现在表达式里。
    fn data_statement(&mut self) -> Result<StatementKind, Diagnostic> {
        self.expect(TokenKind::Dot, "data 后需要 `.`")?;
        let (method, method_span) = self.ident("data 方法")?;
        let Some(method) = data_method(&method) else {
            return Err(Diagnostic::new(
                format!(
                    "未知 data 方法 `{method}`，可用 merge/合并、remove/移除、modify/修改；get/取 是表达式"
                ),
                method_span,
            ));
        };
        if method == "get" {
            return Err(Diagnostic::new(
                "data.get 只能出现在表达式里，例如 `let health = data.get(entity, self, \"Health\");`",
                method_span,
            ));
        }
        self.expect(TokenKind::LeftParen, "data 方法后需要 `(`")?;
        let target = self.nbt_source_value("data 目标")?;
        match method {
            "merge" => {
                self.expect(TokenKind::Comma, "data 目标后需要 `,`")?;
                let nbt = if matches!(target, NbtComponentSource::Entity(_)) {
                    self.nbt_compound_with_aliases("data.merge 数据")?
                } else {
                    self.nbt_compound("data.merge 数据")?
                };
                self.expect(TokenKind::RightParen, "data.merge 调用缺少 `)`")?;
                self.expect(TokenKind::Semicolon, "data.merge 调用后需要 `;`")?;
                Ok(StatementKind::DataMerge { target, nbt })
            }
            "remove" => {
                self.expect(TokenKind::Comma, "data 目标后需要 `,`")?;
                let (path, path_span) = self.string("data.remove 需要 NBT 路径字符串")?;
                self.expect(TokenKind::RightParen, "data.remove 调用缺少 `)`")?;
                self.expect(TokenKind::Semicolon, "data.remove 调用后需要 `;`")?;
                Ok(StatementKind::DataRemove {
                    target,
                    path,
                    path_span,
                })
            }
            _ => {
                self.expect(TokenKind::Comma, "data 目标后需要 `,`")?;
                let (path, path_span) = self.string("data.modify 需要 NBT 路径字符串")?;
                self.expect(TokenKind::Comma, "路径后需要 `,`")?;
                let (operation, operation_span) =
                    self.ident("data.modify 操作 insert/prepend/append/set/merge")?;
                let kind = match operation.as_str() {
                    "insert" | "插入" => DataOperationKind::Insert,
                    "prepend" | "前插" => DataOperationKind::Prepend,
                    "append" | "追加" => DataOperationKind::Append,
                    "set" | "设置" => DataOperationKind::Set,
                    "merge" | "合并" => DataOperationKind::Merge,
                    _ => {
                        return Err(Diagnostic::new(
                            format!(
                                "未知 data.modify 操作 `{operation}`，可用 insert、prepend、append、set、merge"
                            ),
                            operation_span,
                        ));
                    }
                };
                let index = if kind == DataOperationKind::Insert {
                    self.expect(TokenKind::Comma, "insert 后需要 `,`")?;
                    Some(self.signed("insert 下标")?)
                } else {
                    None
                };
                self.expect(TokenKind::Comma, "操作后需要 `,`")?;
                let source = self.data_source()?;
                self.expect(TokenKind::RightParen, "data.modify 调用缺少 `)`")?;
                self.expect(TokenKind::Semicolon, "data.modify 调用后需要 `;`")?;
                Ok(StatementKind::DataModify {
                    target,
                    path,
                    path_span,
                    operation: DataOperation {
                        kind,
                        index,
                        source,
                    },
                })
            }
        }
    }

    /// `from(...)`、`value(nbt { ... })`、`string(...)` 或 `compute(...)`。
    fn data_source(&mut self) -> Result<DataSource, Diagnostic> {
        let (name, span) = self.ident("数据来源 from、value、string 或 compute")?;
        match name.as_str() {
            "from" | "来源" => {
                self.expect(TokenKind::LeftParen, "from 后需要 `(`")?;
                let target = self.nbt_source_value("from 来源")?;
                self.expect(TokenKind::Comma, "from 来源后需要 `,`")?;
                let (path, path_span) = self.string("from 需要 NBT 路径字符串")?;
                self.expect(TokenKind::RightParen, "from 缺少 `)`")?;
                Ok(DataSource::From {
                    target,
                    path,
                    path_span,
                })
            }
            "value" | "值" => {
                self.expect(TokenKind::LeftParen, "value 后需要 `(`")?;
                // `value(nbt { ... })` 也可写成 `value({ ... })`，两者等价。
                self.take_word("nbt");
                let nbt = self.nbt_value()?;
                self.expect(TokenKind::RightParen, "value 缺少 `)`")?;
                Ok(DataSource::Value(nbt))
            }
            "string" | "字符串" => {
                self.expect(TokenKind::LeftParen, "string 后需要 `(`")?;
                let target = self.nbt_source_value("string 来源")?;
                self.expect(TokenKind::Comma, "string 来源后需要 `,`")?;
                let (path, path_span) = self.string("string 需要 NBT 路径字符串")?;
                let start = if self.take(&TokenKind::Comma).is_some() {
                    Some(self.signed("string 起始下标")?)
                } else {
                    None
                };
                let end = if start.is_some() && self.take(&TokenKind::Comma).is_some() {
                    Some(self.signed("string 结束下标")?)
                } else {
                    None
                };
                self.expect(TokenKind::RightParen, "string 缺少 `)`")?;
                Ok(DataSource::String {
                    target,
                    path,
                    path_span,
                    start,
                    end,
                })
            }
            "compute" | "计算" => {
                self.expect(TokenKind::LeftParen, "compute 后需要 `(`")?;
                let (source, kind, provider, provider_span, scale) = self.compute_payload()?;
                self.expect(TokenKind::RightParen, "compute 缺少 `)`")?;
                Ok(DataSource::Compute {
                    source,
                    kind,
                    provider,
                    provider_span,
                    scale,
                })
            }
            _ => Err(Diagnostic::new(
                format!(
                    "未知数据来源 `{name}`，可用 from/来源、value/值、string/字符串、compute/计算"
                ),
                span,
            )),
        }
    }

    fn scoreboard_statement(&mut self) -> Result<StatementKind, Diagnostic> {
        self.expect(TokenKind::Dot, "scoreboard 后需要 `.`")?;
        let (method, method_span) = self.ident("scoreboard 方法")?;
        let Some(method) = scoreboard_method(&method) else {
            return Err(Diagnostic::new(
                format!("未知 scoreboard 方法 `{method}`"),
                method_span,
            ));
        };
        if method == "display" {
            return self.scoreboard_display_statement();
        }
        let target = self.score_target("scoreboard 方法")?;
        match method {
            "set" => {
                self.expect(TokenKind::Comma, "计分目标后需要 `,`")?;
                let value = self.expression()?;
                self.expect(TokenKind::RightParen, "scoreboard.set 调用缺少 `)`")?;
                self.expect(TokenKind::Semicolon, "scoreboard.set 调用后需要 `;`")?;
                Ok(StatementKind::ScoreSet { target, value })
            }
            "reset" => {
                self.expect(TokenKind::RightParen, "scoreboard.reset 调用缺少 `)`")?;
                self.expect(TokenKind::Semicolon, "scoreboard.reset 调用后需要 `;`")?;
                Ok(StatementKind::ScoreReset { target })
            }
            "enable" => {
                self.expect(TokenKind::RightParen, "scoreboard.enable 调用缺少 `)`")?;
                self.expect(TokenKind::Semicolon, "scoreboard.enable 调用后需要 `;`")?;
                Ok(StatementKind::ScoreboardEnable { target })
            }
            "operation" => {
                self.expect(TokenKind::Comma, "结果计分目标后需要 `,`")?;
                let (operation, operation_span) =
                    self.ident("运算名称 set/add/subtract/multiply/divide/modulo/min/max/swap")?;
                let Some(operation) = score_operation(&operation) else {
                    return Err(Diagnostic::new(
                        format!(
                            "未知运算 `{operation}`，可用 set、add、subtract、multiply、divide、modulo、min、max、swap"
                        ),
                        operation_span,
                    ));
                };
                let operation = match operation {
                    "set" => ScoreboardOp::Set,
                    "add" => ScoreboardOp::Add,
                    "subtract" => ScoreboardOp::Subtract,
                    "multiply" => ScoreboardOp::Multiply,
                    "divide" => ScoreboardOp::Divide,
                    "modulo" => ScoreboardOp::Modulo,
                    "min" => ScoreboardOp::Min,
                    "max" => ScoreboardOp::Max,
                    _ => ScoreboardOp::Swap,
                };
                self.expect(TokenKind::Comma, "运算名称后需要 `,`")?;
                let source = self.score_target_body("来源计分目标")?;
                self.expect(TokenKind::RightParen, "scoreboard.operation 调用缺少 `)`")?;
                self.expect(TokenKind::Semicolon, "scoreboard.operation 调用后需要 `;`")?;
                Ok(StatementKind::ScoreboardOperation {
                    result: target,
                    operation,
                    source,
                })
            }
            "get" => Err(Diagnostic::new(
                "scoreboard.get 只能出现在表达式里，例如 `let id = scoreboard.get(self, box_key);`",
                method_span,
            )),
            _ => unreachable!("scoreboard_method 只返回已知方法"),
        }
    }

    /// `scoreboard.display("侧边栏"[, 目标]);`
    fn scoreboard_display_statement(&mut self) -> Result<StatementKind, Diagnostic> {
        self.expect(TokenKind::LeftParen, "scoreboard.display 后需要 `(`")?;
        let (slot, slot_span) = self.string("scoreboard.display 需要显示槽字符串")?;
        let objective = if self.take(&TokenKind::Comma).is_some() {
            Some(self.ident("scoreboard.display 需要已声明的目标名称")?)
        } else {
            None
        };
        self.expect(TokenKind::RightParen, "scoreboard.display 调用缺少 `)`")?;
        self.expect(TokenKind::Semicolon, "scoreboard.display 调用后需要 `;`")?;
        Ok(StatementKind::ScoreboardDisplay {
            slot,
            slot_span,
            objective,
        })
    }

    /// `teleport(持有者, 坐标或实体查询);`
    ///
    /// 落点写成 `pos`/`block_pos`/`vec3` 时传送到该坐标（可选 `rotation(...)`），
    /// 写成查询名称时跟随该单个实体的位置与朝向。
    fn teleport_statement(&mut self) -> Result<StatementKind, Diagnostic> {
        self.expect(TokenKind::LeftParen, "teleport 后需要 `(`")?;
        let targets = self.score_holder()?;
        self.expect(TokenKind::Comma, "teleport 目标后需要 `,`")?;
        let destination =
            if self.check_word("pos") || self.check_word("block_pos") || self.check_word("vec3") {
                TeleportDestination::Position(self.position_value("传送坐标")?)
            } else {
                let (query, query_span) = self.ident("实体查询名称")?;
                TeleportDestination::Entity { query, query_span }
            };
        let rotation = if self.take(&TokenKind::Comma).is_some() {
            Some(self.rotation_value("传送朝向")?)
        } else {
            None
        };
        self.expect(TokenKind::RightParen, "teleport 调用缺少 `)`")?;
        self.expect(TokenKind::Semicolon, "teleport 调用后需要 `;`")?;
        Ok(StatementKind::Teleport {
            targets,
            destination,
            rotation,
        })
    }

    /// `advancement.grant(目标, 进度[, 准则]);` 及其余四种作用范围的方法。
    ///
    /// `everything` 不需要进度参数；只有 `only` 可以带准则名。
    fn advancement_statement(&mut self) -> Result<StatementKind, Diagnostic> {
        self.expect(TokenKind::Dot, "advancement 后需要 `.`")?;
        let (method, method_span) = self.ident("advancement 方法")?;
        let Some((operation, scope)) = advancement_method(&method) else {
            return Err(Diagnostic::new(
                format!("未知 advancement 方法 `{method}`"),
                method_span,
            ));
        };
        self.expect(TokenKind::LeftParen, "advancement 方法后需要 `(`")?;
        let targets = self.holder("advancement 目标")?;
        let mut advancement = None;
        let mut criterion = None;
        let mut criterion_span = None;
        if scope != "everything" {
            self.expect(TokenKind::Comma, "advancement 目标后需要 `,`")?;
            advancement = Some(self.advancement_reference("进度")?);
        }
        if scope == "only" && self.take(&TokenKind::Comma).is_some() {
            let (value, span) = self.ident("准则名称")?;
            criterion = Some(value);
            criterion_span = Some(span);
        }
        self.expect(TokenKind::RightParen, "advancement 调用缺少 `)`")?;
        self.expect(TokenKind::Semicolon, "advancement 调用后需要 `;`")?;
        Ok(StatementKind::AdvancementAction {
            operation: if operation == "grant" {
                AdvancementOperation::Grant
            } else {
                AdvancementOperation::Revoke
            },
            scope: match scope {
                "only" => AdvancementScope::Only,
                "through" => AdvancementScope::Through,
                "from" => AdvancementScope::From,
                "until" => AdvancementScope::Until,
                _ => AdvancementScope::Everything,
            },
            targets,
            advancement,
            criterion,
            criterion_span,
        })
    }

    /// 读取计分持有者：`self`/`自身`、`origin`/`投掷者` 或实体查询名称。
    pub(super) fn score_holder(&mut self) -> Result<Holder, Diagnostic> {
        self.holder("计分持有者")
    }

    /// 读取实体持有者：`self`/`自身`、`origin`/`投掷者` 或实体查询名称。
    pub(super) fn holder(&mut self, label: &str) -> Result<Holder, Diagnostic> {
        if self.take_word("self").is_some() {
            return Ok(Holder::SelfEntity);
        }
        if self.take_word("origin").is_some() {
            return Ok(Holder::Origin);
        }
        let (name, span) =
            self.ident(&format!("{label} self/自身、origin/投掷者 或实体查询名称"))?;
        Ok(Holder::Query(name, span))
    }

    /// 读取计分目标的 `(持有者, 目标)` 部分，右括号留给调用方。
    pub(super) fn score_target(&mut self, label: &str) -> Result<ScoreTarget, Diagnostic> {
        self.expect(TokenKind::LeftParen, &format!("{label} 后需要 `(`"))?;
        self.score_target_body(label)
    }

    /// `(持有者, 目标)` 的内部形式：`scoreboard.operation` 的来源参数不带括号。
    pub(super) fn score_target_body(&mut self, _label: &str) -> Result<ScoreTarget, Diagnostic> {
        let holder = self.score_holder()?;
        self.expect(TokenKind::Comma, "计分持有者后需要 `,`")?;
        let (objective, objective_span) = self.ident("计分板目标名称")?;
        Ok(ScoreTarget {
            holder,
            objective,
            objective_span,
        })
    }

    /// `stopwatch.create/restart/remove("命名空间:id")`。
    ///
    /// `stopwatch.query` 有返回值，只能在表达式里使用，语句形式会给出引导性诊断。
    fn stopwatch_statement(&mut self) -> Result<StatementKind, Diagnostic> {
        self.expect(TokenKind::Dot, "stopwatch 后需要 `.`")?;
        let (method, method_span) = self.ident("stopwatch 方法")?;
        let Some(method) = stopwatch_method(&method) else {
            return Err(Diagnostic::new(
                format!("未知 stopwatch 方法 `{method}`"),
                method_span,
            ));
        };
        if method == "query" {
            return Err(Diagnostic::new(
                "stopwatch.query 只能出现在表达式里，例如 `let seconds = stopwatch.query(\"demo:timer\");`",
                method_span,
            ));
        }
        self.expect(TokenKind::LeftParen, "stopwatch 方法后需要 `(`")?;
        let (id, _) = self.string("stopwatch 需要秒表资源位置")?;
        self.expect(TokenKind::RightParen, "stopwatch 调用缺少 `)`")?;
        self.expect(TokenKind::Semicolon, "stopwatch 调用后需要 `;`")?;
        Ok(StatementKind::StopwatchAction {
            operation: match method {
                "create" => StopwatchOperation::Create,
                "restart" => StopwatchOperation::Restart,
                _ => StopwatchOperation::Remove,
            },
            id,
        })
    }

    /// 读取一条底层命令字符串，执行与 `run` 相同的空值、斜杠和换行检查。
    fn command_string(&mut self, label: &str) -> Result<(String, Span), Diagnostic> {
        let (command, span) = self.string(&format!("{label} 后需要命令字符串"))?;
        if command.trim().is_empty() {
            return Err(Diagnostic::new(format!("{label} 命令不能为空"), span));
        }
        if command.starts_with('/') {
            return Err(Diagnostic::new(
                "Minecraft 函数中的命令不能以 `/` 开头",
                span,
            ));
        }
        if command.contains(['\n', '\r']) {
            return Err(Diagnostic::new(
                format!("一条 {label} 语句只能包含一行命令"),
                span,
            ));
        }
        Ok((command, span))
    }

    fn assignment_operator(&mut self) -> Result<AssignOp, Diagnostic> {
        let token = self.advance().clone();
        match token.kind {
            TokenKind::Equal => Ok(AssignOp::Set),
            TokenKind::PlusEqual => Ok(AssignOp::Add),
            TokenKind::MinusEqual => Ok(AssignOp::Subtract),
            TokenKind::StarEqual => Ok(AssignOp::Multiply),
            TokenKind::SlashEqual => Ok(AssignOp::Divide),
            TokenKind::PercentEqual => Ok(AssignOp::Modulo),
            _ => Err(Diagnostic::new(
                "这里需要赋值运算符 =、+=、-=、*=、/= 或 %=",
                token.span,
            )),
        }
    }

    fn empty_arguments(&mut self) -> Result<(), Diagnostic> {
        self.expect(TokenKind::LeftParen, "函数名称后需要 `(`")?;
        self.expect(TokenKind::RightParen, "调度函数暂不支持参数，需要 `)`")?;
        Ok(())
    }
}
