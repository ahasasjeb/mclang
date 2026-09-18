use crate::ast::*;
use crate::diagnostic::Diagnostic;
use crate::lexer::TokenKind;

use super::Parser;
use super::component_coordinate;
use crate::parser::keywords::{
    boolean_word, forceload_method, gamerule_method, locate_kind, time_method, weather_kind,
    worldborder_method,
};

impl Parser {
    /// `forceload.add/remove/remove_all/query(...)`。
    pub(in crate::parser) fn forceload_statement(&mut self) -> Result<StatementKind, Diagnostic> {
        self.expect(TokenKind::Dot, "forceload 后需要 `.`")?;
        let (method, method_span) = self.ident("forceload 方法")?;
        let Some(method) = forceload_method(&method) else {
            return Err(Diagnostic::new(
                format!("未知 forceload 方法 `{method}`，可用 add、remove、remove_all、query"),
                method_span,
            ));
        };
        self.expect(TokenKind::LeftParen, "forceload 方法后需要 `(`")?;
        let operation = match method {
            "add" | "remove" => {
                let label = if method == "add" {
                    "forceload.add 的起点"
                } else {
                    "forceload.remove 的起点"
                };
                let from = self.column_position(label)?;
                let to = self.optional_column_position("forceload 的终点")?;
                if method == "add" {
                    ForceLoadOperation::Add { from, to }
                } else {
                    ForceLoadOperation::Remove { from, to }
                }
            }
            "remove_all" => ForceLoadOperation::RemoveAll,
            _ => {
                let pos = if self.check(&TokenKind::RightParen) {
                    None
                } else {
                    Some(self.column_position("forceload.query 的列坐标")?)
                };
                ForceLoadOperation::Query { pos }
            }
        };
        self.expect(TokenKind::RightParen, "forceload 调用缺少 `)`")?;
        self.expect(TokenKind::Semicolon, "forceload 调用后需要 `;`")?;
        Ok(StatementKind::ForceLoad(operation))
    }

    /// `time.set/add/pause/resume/rate(..., [时钟])`；查询在表达式中使用。
    pub(in crate::parser) fn time_statement(&mut self) -> Result<StatementKind, Diagnostic> {
        self.expect(TokenKind::Dot, "time 后需要 `.`")?;
        let (method, method_span) = self.ident("time 方法")?;
        let Some(method) = time_method(&method) else {
            return Err(Diagnostic::new(
                format!(
                    "未知 time 方法 `{method}`，可用 set、add、pause、resume、rate、query、query_gametime"
                ),
                method_span,
            ));
        };
        if method == "query" || method == "query_gametime" {
            return Err(Diagnostic::new(
                format!("time.{method} 只能出现在表达式里，例如 `let ticks = time.{method}();`"),
                method_span,
            ));
        }
        self.expect(TokenKind::LeftParen, "time 方法后需要 `(`")?;
        let operation = match method {
            "set" => TimeOperation::Set(self.time_argument(0, "time.set 的时间")?),
            "add" => TimeOperation::Add(self.time_argument(i32::MIN, "time.add 的时间")?),
            "pause" => TimeOperation::Pause,
            "resume" => TimeOperation::Resume,
            _ => TimeOperation::Rate(self.signed_number_text("time.rate 速率")?),
        };
        let clock = self.optional_clock("time 的世界时钟")?;
        self.expect(TokenKind::RightParen, "time 调用缺少 `)`")?;
        self.expect(TokenKind::Semicolon, "time 调用后需要 `;`")?;
        Ok(StatementKind::TimeAction { operation, clock })
    }

    /// `weather.clear/rain/thunder([持续时间])`。
    pub(in crate::parser) fn weather_statement(&mut self) -> Result<StatementKind, Diagnostic> {
        self.expect(TokenKind::Dot, "weather 后需要 `.`")?;
        let (method, method_span) = self.ident("weather 方法")?;
        let Some(kind) = weather_kind(&method) else {
            return Err(Diagnostic::new(
                format!("未知 weather 方法 `{method}`，可用 clear、rain、thunder"),
                method_span,
            ));
        };
        self.expect(TokenKind::LeftParen, "weather 方法后需要 `(`")?;
        let duration = if self.check(&TokenKind::RightParen) {
            None
        } else {
            Some(self.time_argument(1, "weather 持续时间")?)
        };
        self.expect(TokenKind::RightParen, "weather 调用缺少 `)`")?;
        self.expect(TokenKind::Semicolon, "weather 调用后需要 `;`")?;
        Ok(StatementKind::Weather { kind, duration })
    }

    /// `gamerule.set(规则, 值)`；`gamerule.query` 在表达式中使用。
    pub(in crate::parser) fn gamerule_statement(&mut self) -> Result<StatementKind, Diagnostic> {
        self.expect(TokenKind::Dot, "gamerule 后需要 `.`")?;
        let (method, method_span) = self.ident("gamerule 方法")?;
        let Some(method) = gamerule_method(&method) else {
            return Err(Diagnostic::new(
                format!("未知 gamerule 方法 `{method}`，可用 set、query"),
                method_span,
            ));
        };
        if method == "query" {
            return Err(Diagnostic::new(
                "gamerule.query 只能出现在表达式里，例如 `let keep = gamerule.query(\"keep_inventory\");`",
                method_span,
            ));
        }
        self.expect(TokenKind::LeftParen, "gamerule.set 后需要 `(`")?;
        let (name, _) = self.string("gamerule.set 需要规则名称字符串")?;
        self.expect(TokenKind::Comma, "规则名称后需要 `,`")?;
        let value = self.game_rule_value()?;
        self.expect(TokenKind::RightParen, "gamerule.set 调用缺少 `)`")?;
        self.expect(TokenKind::Semicolon, "gamerule.set 调用后需要 `;`")?;
        Ok(StatementKind::GameRuleSet { name, value })
    }

    /// 规则值是布尔字面量或整数。
    fn game_rule_value(&mut self) -> Result<GameRuleValue, Diagnostic> {
        if matches!(&self.current().kind, TokenKind::Number(_)) || self.check(&TokenKind::Minus) {
            return Ok(GameRuleValue::Integer(self.signed("gamerule 整数值")?));
        }
        let (value, span) = self.ident("true、false 或整数")?;
        match boolean_word(&value) {
            Some("true") => Ok(GameRuleValue::Bool(true)),
            Some("false") => Ok(GameRuleValue::Bool(false)),
            _ => Err(Diagnostic::new(
                "gamerule 的值需要 true、false 或整数",
                span,
            )),
        }
    }

    /// `worldborder.add/set/center/damage_amount/damage_buffer/warning_distance/warning_time(...)`。
    ///
    /// `worldborder.get` 有返回值，只能在表达式里使用。
    pub(in crate::parser) fn worldborder_statement(&mut self) -> Result<StatementKind, Diagnostic> {
        self.expect(TokenKind::Dot, "worldborder 后需要 `.`")?;
        let (method, method_span) = self.ident("worldborder 方法")?;
        let Some(method) = worldborder_method(&method) else {
            return Err(Diagnostic::new(
                format!(
                    "未知 worldborder 方法 `{method}`，可用 add、set、center、damage_amount、damage_buffer、get、warning_distance、warning_time"
                ),
                method_span,
            ));
        };
        if method == "get" {
            return Err(Diagnostic::new(
                "worldborder.get 只能出现在表达式里，例如 `let size = worldborder.get();`",
                method_span,
            ));
        }
        self.expect(TokenKind::LeftParen, "worldborder 方法后需要 `(`")?;
        let operation = match method {
            "add" | "set" => {
                let distance = self.signed_number_text("worldborder 边长")?;
                let time = if self.take(&TokenKind::Comma).is_some() {
                    if self.check(&TokenKind::RightParen) {
                        None
                    } else {
                        Some(self.time_argument(0, "worldborder 过渡时间")?)
                    }
                } else {
                    None
                };
                if method == "add" {
                    WorldBorderOperation::Add { distance, time }
                } else {
                    WorldBorderOperation::Set { distance, time }
                }
            }
            "center" => {
                if self.check_word("vec2") {
                    WorldBorderOperation::Center(self.vec2_value("worldborder.center 坐标")?)
                } else {
                    let start = self.current().span;
                    let x = self.vec2_component("worldborder.center 的 X 坐标")?;
                    self.expect(TokenKind::Comma, "中心 X 坐标后需要 `,`")?;
                    let z = self.vec2_component("worldborder.center 的 Z 坐标")?;
                    WorldBorderOperation::Center(Vec2Value {
                        x: component_coordinate(x),
                        z: component_coordinate(z),
                        span: start.merge(self.current().span),
                    })
                }
            }
            "damage_amount" => {
                WorldBorderOperation::DamageAmount(self.signed_number_text("worldborder 每块伤害")?)
            }
            "damage_buffer" => {
                WorldBorderOperation::DamageBuffer(self.signed_number_text("worldborder 伤害缓冲")?)
            }
            "warning_distance" => {
                WorldBorderOperation::WarningDistance(self.unsigned("worldborder 警告距离")?)
            }
            _ => WorldBorderOperation::WarningTime(self.time_argument(0, "worldborder 警告时间")?),
        };
        self.expect(TokenKind::RightParen, "worldborder 调用缺少 `)`")?;
        self.expect(TokenKind::Semicolon, "worldborder 调用后需要 `;`")?;
        Ok(StatementKind::WorldBorder(operation))
    }

    /// `locate.structure/biome/poi("资源位置或 #标签")`，只产生命令反馈。
    pub(in crate::parser) fn locate_statement(&mut self) -> Result<StatementKind, Diagnostic> {
        self.expect(TokenKind::Dot, "locate 后需要 `.`")?;
        let (method, method_span) = self.ident("locate 方法")?;
        let Some(kind) = locate_kind(&method) else {
            return Err(Diagnostic::new(
                format!("未知 locate 方法 `{method}`，可用 structure、biome、poi"),
                method_span,
            ));
        };
        self.expect(TokenKind::LeftParen, "locate 方法后需要 `(`")?;
        let (target, _) = self.string("locate 需要资源位置或 #标签字符串")?;
        self.expect(TokenKind::RightParen, "locate 调用缺少 `)`")?;
        self.expect(TokenKind::Semicolon, "locate 调用后需要 `;`")?;
        Ok(StatementKind::Locate { kind, target })
    }
}
