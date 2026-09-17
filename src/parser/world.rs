//! 世界与方块语句：坐标、方块状态值，以及 `set_block`/`fill`/`clone`/`place`、
//! `forceload`、`time`、`weather`、`gamerule`、`worldborder`、`locate` 的参数解析。
//!
//! 位置与方块状态是这些命令共享的具名参数（`pos(...)`、`block_state(...)`），
//! 与查询、物品声明一样在解析期规范化；绝对坐标的范围和资源位置在语义阶段检查。

use crate::ast::*;
use crate::diagnostic::Diagnostic;
use crate::lexer::{Token, TokenKind};

use super::Parser;
use super::keywords::{
    boolean_word, clone_dimension, clone_filter, clone_mode, fill_mode, forceload_method,
    gamerule_method, locate_kind, place_method, set_block_mode, strict_word, template_mirror,
    template_rotation, time_method, weather_kind, worldborder_method,
};

/// `vec2_component` 的文本转成坐标分量：`~` 前缀是相对坐标。
fn component_coordinate(text: String) -> Coordinate {
    if text.starts_with('~') {
        Coordinate::Relative(text)
    } else {
        Coordinate::Absolute(text)
    }
}

/// `place.template` 的可选后缀，按原版顺序收集。
struct TemplateOptions {
    rotation: Option<TemplateRotation>,
    mirror: Option<TemplateMirror>,
    integrity: Option<String>,
    seed: Option<i32>,
    strict: bool,
}

impl Parser {
    /// `set_block(pos, block_state[, mode][, nbt { ... }])`。
    ///
    /// 模式与方块实体数据都是可选参数，可以任意顺序书写，但各自最多一次。
    pub(super) fn set_block_statement(&mut self) -> Result<StatementKind, Diagnostic> {
        self.expect(TokenKind::LeftParen, "set_block 后需要 `(`")?;
        let pos = self.block_position("set_block 的位置参数")?;
        self.expect(TokenKind::Comma, "set_block 位置后需要 `,`")?;
        let block = self.block_state_value("set_block 的方块参数")?;
        let mut mode = SetBlockMode::Replace;
        let mut has_mode = false;
        let mut nbt = None;
        while self.take(&TokenKind::Comma).is_some() {
            if self.check_word("nbt") {
                if nbt.is_some() {
                    return Err(Diagnostic::new(
                        "set_block 的方块实体数据只能声明一次",
                        self.current().span,
                    ));
                }
                nbt = Some(self.nbt_compound_with_aliases("set_block 的方块实体数据")?);
                continue;
            }
            if has_mode {
                return Err(Diagnostic::new(
                    "set_block 的方块放置模式只能声明一次",
                    self.current().span,
                ));
            }
            let (value, span) =
                self.ident("setblock 模式 destroy/keep/replace/strict，或 nbt { ... }")?;
            mode = set_block_mode(&value).ok_or_else(|| {
                Diagnostic::new(
                    format!("未知 setblock 模式 `{value}`，可用 destroy、keep、replace、strict"),
                    span,
                )
            })?;
            has_mode = true;
        }
        self.expect(TokenKind::RightParen, "set_block 调用缺少 `)`")?;
        self.expect(TokenKind::Semicolon, "set_block 调用后需要 `;`")?;
        Ok(StatementKind::SetBlock {
            pos,
            block,
            mode,
            nbt,
        })
    }

    /// `fill(from, to, block_state[, mode][, replace filter][, nbt { ... }])`。
    ///
    /// `nbt { ... }` 可以出现在任意可选参数位置；模式与过滤器保持原版顺序。
    pub(super) fn fill_statement(&mut self) -> Result<StatementKind, Diagnostic> {
        self.expect(TokenKind::LeftParen, "fill 后需要 `(`")?;
        let from = self.block_position("fill 的起点")?;
        self.expect(TokenKind::Comma, "fill 起点后需要 `,`")?;
        let to = self.block_position("fill 的终点")?;
        self.expect(TokenKind::Comma, "fill 终点后需要 `,`")?;
        let block = self.block_state_value("fill 的方块参数")?;
        let mut mode = FillMode::Replace;
        let mut has_mode = false;
        let mut filter = None;
        let mut nbt = None;
        while self.take(&TokenKind::Comma).is_some() {
            if self.check_word("nbt") {
                if nbt.is_some() {
                    return Err(Diagnostic::new(
                        "fill 的方块实体数据只能声明一次",
                        self.current().span,
                    ));
                }
                nbt = Some(self.nbt_compound_with_aliases("fill 的方块实体数据")?);
                continue;
            }
            if !has_mode {
                let (value, span) = self.ident("fill 模式")?;
                mode = fill_mode(&value).ok_or_else(|| {
                    Diagnostic::new(
                        format!(
                            "未知 fill 模式 `{value}`，可用 replace、outline、hollow、destroy、strict、keep；\
                             替换过滤器写成 `fill(起点, 终点, 方块, replace, block_state(\"#标签\"))`"
                        ),
                        span,
                    )
                })?;
                has_mode = true;
                continue;
            }
            if filter.is_some() {
                return Err(Diagnostic::new(
                    "fill 的可选参数已经结束，只允许再写 nbt { ... }",
                    self.current().span,
                ));
            }
            if mode != FillMode::Replace {
                return Err(Diagnostic::new(
                    "fill 的替换过滤器只与 replace 模式一起使用",
                    self.previous().span,
                ));
            }
            filter = Some(self.block_state_value("fill 的替换过滤器")?);
        }
        self.expect(TokenKind::RightParen, "fill 调用缺少 `)`")?;
        self.expect(TokenKind::Semicolon, "fill 调用后需要 `;`")?;
        Ok(StatementKind::Fill {
            from,
            to,
            block,
            mode,
            filter,
            nbt,
        })
    }

    /// `fill_biome(from, to, "生物群系"[, replace, "生物群系或标签"])`。
    pub(super) fn fill_biome_statement(&mut self) -> Result<StatementKind, Diagnostic> {
        self.expect(TokenKind::LeftParen, "fill_biome 后需要 `(`")?;
        let from = self.block_position("fill_biome 的起点")?;
        self.expect(TokenKind::Comma, "fill_biome 起点后需要 `,`")?;
        let to = self.block_position("fill_biome 的终点")?;
        self.expect(TokenKind::Comma, "fill_biome 终点后需要 `,`")?;
        let (biome, _) = self.string("fill_biome 需要生物群系资源位置字符串")?;
        let filter = if self.take(&TokenKind::Comma).is_some() {
            self.expect_word("replace")?;
            self.expect(TokenKind::Comma, "replace 后需要 `,`")?;
            Some(self.string("fill_biome 过滤器需要生物群系或标签字符串")?.0)
        } else {
            None
        };
        self.expect(TokenKind::RightParen, "fill_biome 调用缺少 `)`")?;
        self.expect(TokenKind::Semicolon, "fill_biome 调用后需要 `;`")?;
        Ok(StatementKind::FillBiome {
            from,
            to,
            biome,
            filter,
        })
    }

    /// `clone(begin, end, destination[, 选项...])`。
    ///
    /// 选项按任意顺序书写，但每个最多一次：`replace`/`masked`/`filtered(方块谓词)`、
    /// `normal`/`force`/`move`、`strict`、`from_dimension("维度")`、`to_dimension("维度")`。
    pub(super) fn clone_statement(&mut self) -> Result<StatementKind, Diagnostic> {
        self.expect(TokenKind::LeftParen, "clone 后需要 `(`")?;
        let begin = self.block_position("clone 的起点")?;
        self.expect(TokenKind::Comma, "clone 起点后需要 `,`")?;
        let end = self.block_position("clone 的终点")?;
        self.expect(TokenKind::Comma, "clone 终点后需要 `,`")?;
        let destination = self.block_position("clone 的目标位置")?;

        let mut from_dimension = None;
        let mut to_dimension = None;
        let mut filter: Option<CloneFilter> = None;
        let mut mode: Option<CloneMode> = None;
        let mut strict = false;
        while self.take(&TokenKind::Comma).is_some() {
            let (clause, span) = self.ident("clone 选项")?;
            if let Some(kind) = clone_filter(&clause) {
                if filter.is_some() {
                    return Err(Diagnostic::new("clone 只能指定一次过滤方式", span));
                }
                filter = Some(match kind {
                    "replace" => CloneFilter::Replace,
                    "masked" => CloneFilter::Masked,
                    "filtered" => {
                        self.expect(TokenKind::Comma, "filtered 后需要 `,` 和方块谓词")?;
                        CloneFilter::Filtered(self.block_state_value("clone 的方块谓词")?)
                    }
                    _ => unreachable!("clone_filter 只返回 replace、masked、filtered"),
                });
                continue;
            }
            if let Some(kind) = clone_mode(&clause) {
                if mode.is_some() {
                    return Err(Diagnostic::new("clone 只能指定一次复制模式", span));
                }
                mode = Some(kind);
                continue;
            }
            if let Some(kind) = clone_dimension(&clause) {
                self.expect(TokenKind::LeftParen, "维度选项后需要 `(`")?;
                let (dimension, _) = self.string("clone 维度选项需要维度资源位置")?;
                self.expect(TokenKind::RightParen, "维度资源位置后需要 `)`")?;
                let slot = if kind == "from" {
                    &mut from_dimension
                } else {
                    &mut to_dimension
                };
                if slot.is_some() {
                    return Err(Diagnostic::new(
                        format!("clone 只能指定一次 `{clause}`"),
                        span,
                    ));
                }
                *slot = Some(dimension);
                continue;
            }
            if strict_word(&clause) {
                if strict {
                    return Err(Diagnostic::new("clone 只能指定一次 strict", span));
                }
                strict = true;
                continue;
            }
            return Err(Diagnostic::new(format!("未知 clone 选项 `{clause}`"), span));
        }
        self.expect(TokenKind::RightParen, "clone 调用缺少 `)`")?;
        self.expect(TokenKind::Semicolon, "clone 调用后需要 `;`")?;
        Ok(StatementKind::Clone {
            begin,
            end,
            destination,
            from_dimension,
            to_dimension,
            filter: filter.unwrap_or(CloneFilter::Replace),
            mode: mode.unwrap_or(CloneMode::Normal),
            strict,
        })
    }

    /// `place.feature/jigsaw/structure/template(...)`。
    pub(super) fn place_statement(&mut self) -> Result<StatementKind, Diagnostic> {
        self.expect(TokenKind::Dot, "place 后需要 `.`")?;
        let (method, method_span) = self.ident("place 方法")?;
        let Some(method) = place_method(&method) else {
            return Err(Diagnostic::new(
                format!("未知 place 方法 `{method}`，可用 feature、jigsaw、structure、template"),
                method_span,
            ));
        };
        self.expect(TokenKind::LeftParen, "place 方法后需要 `(`")?;
        let kind = match method {
            "feature" | "structure" => {
                let label = if method == "feature" {
                    "place.feature 需要地物资源位置字符串"
                } else {
                    "place.structure 需要结构资源位置字符串"
                };
                let (id, _) = self.string(label)?;
                let pos = self.optional_position()?;
                if method == "feature" {
                    StatementKind::PlaceFeature { feature: id, pos }
                } else {
                    StatementKind::PlaceStructure { structure: id, pos }
                }
            }
            "jigsaw" => {
                let (pool, _) = self.string("place.jigsaw 需要模板池资源位置字符串")?;
                self.expect(TokenKind::Comma, "模板池后需要 `,`")?;
                let (target, _) = self.string("place.jigsaw 需要目标字符串")?;
                self.expect(TokenKind::Comma, "目标后需要 `,`")?;
                let max_depth = self.unsigned("place.jigsaw 最大深度")?;
                let pos = self.optional_position()?;
                StatementKind::PlaceJigsaw {
                    pool,
                    target,
                    max_depth,
                    pos,
                }
            }
            "template" => {
                let (template, _) = self.string("place.template 需要模板资源位置字符串")?;
                self.expect(TokenKind::Comma, "模板后需要 `,`")?;
                let pos = self.block_position("place.template 的位置参数")?;
                let options = self.template_options()?;
                StatementKind::PlaceTemplate {
                    template,
                    pos,
                    rotation: options.rotation,
                    mirror: options.mirror,
                    integrity: options.integrity,
                    seed: options.seed,
                    strict: options.strict,
                }
            }
            _ => unreachable!("place_method 只返回 feature、jigsaw、structure、template"),
        };
        self.expect(TokenKind::RightParen, "place 调用缺少 `)`")?;
        self.expect(TokenKind::Semicolon, "place 调用后需要 `;`")?;
        Ok(kind)
    }

    /// `place.template` 的可选后缀，按原版顺序：旋转、镜像、完整度、种子、strict。
    fn template_options(&mut self) -> Result<TemplateOptions, Diagnostic> {
        let mut options = TemplateOptions {
            rotation: None,
            mirror: None,
            integrity: None,
            seed: None,
            strict: false,
        };
        if self.take(&TokenKind::Comma).is_none() {
            return Ok(options);
        }
        options.rotation = Some(self.template_rotation_value()?);
        if self.take(&TokenKind::Comma).is_none() {
            return Ok(options);
        }
        options.mirror = Some(self.template_mirror_value()?);
        if self.take(&TokenKind::Comma).is_none() {
            return Ok(options);
        }
        options.integrity = Some(self.signed_number_text("place.template 完整度")?);
        if self.take(&TokenKind::Comma).is_none() {
            return Ok(options);
        }
        options.seed = Some(self.signed("place.template 随机种子")?);
        if self.take(&TokenKind::Comma).is_none() {
            return Ok(options);
        }
        let (value, span) = self.ident("strict 标志")?;
        if !strict_word(&value) {
            return Err(Diagnostic::new("这里需要 strict", span));
        }
        options.strict = true;
        Ok(options)
    }

    /// 旋转值是原版 `Rotation` 枚举名；`180` 以数字记号出现。
    fn template_rotation_value(&mut self) -> Result<TemplateRotation, Diagnostic> {
        let token = self.advance().clone();
        let text = match token.kind {
            TokenKind::Ident(value) => value,
            TokenKind::Number(value) => value.to_string(),
            _ => {
                return Err(Diagnostic::new(
                    "模板旋转需要 none、clockwise_90、180 或 counterclockwise_90",
                    token.span,
                ));
            }
        };
        template_rotation(&text).ok_or_else(|| {
            Diagnostic::new(
                format!("未知模板旋转 `{text}`，可用 none、clockwise_90、180、counterclockwise_90"),
                token.span,
            )
        })
    }

    fn template_mirror_value(&mut self) -> Result<TemplateMirror, Diagnostic> {
        let (value, span) = self.ident("模板镜像 none、left_right 或 front_back")?;
        template_mirror(&value).ok_or_else(|| {
            Diagnostic::new(
                format!("未知模板镜像 `{value}`，可用 none、left_right、front_back"),
                span,
            )
        })
    }

    /// `forceload.add/remove/remove_all/query(...)`。
    pub(super) fn forceload_statement(&mut self) -> Result<StatementKind, Diagnostic> {
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
    pub(super) fn time_statement(&mut self) -> Result<StatementKind, Diagnostic> {
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
    pub(super) fn weather_statement(&mut self) -> Result<StatementKind, Diagnostic> {
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
    pub(super) fn gamerule_statement(&mut self) -> Result<StatementKind, Diagnostic> {
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
    pub(super) fn worldborder_statement(&mut self) -> Result<StatementKind, Diagnostic> {
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
    pub(super) fn locate_statement(&mut self) -> Result<StatementKind, Diagnostic> {
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

    /// `pos(x, y, z)` / `block_pos(x, y, z)`：方块坐标，绝对分量是整数，
    /// `~`/`^` 分量可带小数偏移。
    pub(super) fn block_position(&mut self, label: &str) -> Result<BlockPosition, Diagnostic> {
        let start = self
            .take_word("pos")
            .or_else(|| self.take_word("block_pos"));
        let Some(start) = start else {
            return Err(Diagnostic::new(
                format!("{label}需要 `pos(x, y, z)`（中文 `坐标(...)`，别名 `block_pos`）"),
                self.current().span,
            ));
        };
        self.expect(TokenKind::LeftParen, "pos 后需要 `(`")?;
        let x = self.coordinate(label)?;
        self.expect(TokenKind::Comma, "坐标分量后需要 `,`")?;
        let y = self.coordinate(label)?;
        self.expect(TokenKind::Comma, "坐标分量后需要 `,`")?;
        let z = self.coordinate(label)?;
        let end = self.expect(TokenKind::RightParen, "坐标缺少 `)`")?.span;
        let position = BlockPosition {
            x,
            y,
            z,
            span: start.span.merge(end),
        };
        let local = [&position.x, &position.y, &position.z]
            .iter()
            .filter(|coordinate| coordinate.is_local())
            .count();
        if local != 0 && local != 3 {
            return Err(Diagnostic::new(
                "方块坐标不能混用 `^` 局部坐标与 `~`/绝对坐标",
                position.span,
            ));
        }
        Ok(position)
    }

    /// 位置值：`pos`/`block_pos` 的方块坐标，或 `vec3` 的精确坐标。
    pub(super) fn position_value(&mut self, label: &str) -> Result<PositionValue, Diagnostic> {
        if self.check_word("vec3") {
            return Ok(PositionValue::Exact(self.vec3_value(label)?));
        }
        Ok(PositionValue::Block(self.block_position(label)?))
    }

    /// `vec3(x, y, z)`：精确坐标，绝对分量允许小数。
    pub(super) fn vec3_value(&mut self, label: &str) -> Result<Vec3Value, Diagnostic> {
        let Some(start) = self.take_word("vec3") else {
            return Err(Diagnostic::new(
                format!("{label}需要 `vec3(x, y, z)`"),
                self.current().span,
            ));
        };
        self.expect(TokenKind::LeftParen, "vec3 后需要 `(`")?;
        let x = self.fractional_coordinate(label)?;
        self.expect(TokenKind::Comma, "vec3 分量后需要 `,`")?;
        let y = self.fractional_coordinate(label)?;
        self.expect(TokenKind::Comma, "vec3 分量后需要 `,`")?;
        let z = self.fractional_coordinate(label)?;
        let end = self.expect(TokenKind::RightParen, "vec3 缺少 `)`")?.span;
        let position = Vec3Value {
            x,
            y,
            z,
            span: start.span.merge(end),
        };
        let local = [&position.x, &position.y, &position.z]
            .iter()
            .filter(|coordinate| coordinate.is_local())
            .count();
        if local != 0 && local != 3 {
            return Err(Diagnostic::new(
                "vec3 不能混用 `^` 局部坐标与 `~`/绝对坐标",
                position.span,
            ));
        }
        Ok(position)
    }

    /// `vec2(x, z)`：水平精确坐标，对应原版 `Vec2Argument`。
    pub(super) fn vec2_value(&mut self, label: &str) -> Result<Vec2Value, Diagnostic> {
        let Some(start) = self.take_word("vec2") else {
            return Err(Diagnostic::new(
                format!("{label}需要 `vec2(x, z)`"),
                self.current().span,
            ));
        };
        self.expect(TokenKind::LeftParen, "vec2 后需要 `(`")?;
        let x = self.fractional_coordinate(label)?;
        self.expect(TokenKind::Comma, "vec2 分量后需要 `,`")?;
        let z = self.fractional_coordinate(label)?;
        let end = self.expect(TokenKind::RightParen, "vec2 缺少 `)`")?.span;
        for coordinate in [&x, &z] {
            if coordinate.is_local() {
                return Err(Diagnostic::new(
                    "vec2 不支持 `^` 局部坐标，请使用绝对坐标或 `~`",
                    start.span.merge(end),
                ));
            }
        }
        Ok(Vec2Value {
            x,
            z,
            span: start.span.merge(end),
        })
    }

    /// `rotation(yaw, pitch)`：朝向，单位是度；与原版 `RotationArgument` 一样
    /// 只支持绝对角度与 `~` 相对角度，不支持 `^` 局部坐标。
    pub(super) fn rotation_value(&mut self, label: &str) -> Result<RotationValue, Diagnostic> {
        let Some(start) = self.take_word("rotation") else {
            return Err(Diagnostic::new(
                format!("{label}需要 `rotation(yaw, pitch)`（中文 `朝向(...)`）"),
                self.current().span,
            ));
        };
        self.expect(TokenKind::LeftParen, "rotation 后需要 `(`")?;
        let yaw = self.fractional_coordinate(label)?;
        self.expect(TokenKind::Comma, "rotation 分量后需要 `,`")?;
        let pitch = self.fractional_coordinate(label)?;
        let end = self
            .expect(TokenKind::RightParen, "rotation 缺少 `)`")?
            .span;
        if yaw.is_local() || pitch.is_local() {
            return Err(Diagnostic::new(
                "rotation 不支持 `^` 局部坐标，请使用绝对角度或 `~`",
                start.span.merge(end),
            ));
        }
        Ok(RotationValue {
            yaw,
            pitch,
            span: start.span.merge(end),
        })
    }

    /// 精确坐标分量：`coordinate` 的小数版本。
    fn fractional_coordinate(&mut self, label: &str) -> Result<Coordinate, Diagnostic> {
        if let Some(token) = self.take(&TokenKind::Tilde) {
            let offset = self.coordinate_offset(label, &token)?;
            return Ok(Coordinate::Relative(format!("~{offset}")));
        }
        if let Some(token) = self.take(&TokenKind::Caret) {
            let offset = self.coordinate_offset(label, &token)?;
            return Ok(Coordinate::Local(format!("^{offset}")));
        }
        let negative = self.negative_sign();
        let token = self.advance().clone();
        let text = match token.kind {
            TokenKind::Number(value) => value.to_string(),
            TokenKind::Decimal(value) => format!("{value}"),
            _ => {
                return Err(Diagnostic::new(
                    format!("{label}的坐标需要数字，或用 `~`/`^` 写相对坐标"),
                    token.span,
                ));
            }
        };
        Ok(Coordinate::Absolute(if negative {
            format!("-{text}")
        } else {
            text
        }))
    }

    /// `column(x, z)`：列坐标，与原版 `ColumnPosArgument` 一样不支持 `^`。
    pub(super) fn column_position(&mut self, label: &str) -> Result<ColumnPosition, Diagnostic> {
        let Some(start) = self.take_word("column") else {
            return Err(Diagnostic::new(
                format!("{label}需要 `column(x, z)`（中文 `列坐标(...)`）"),
                self.current().span,
            ));
        };
        self.expect(TokenKind::LeftParen, "column 后需要 `(`")?;
        let x = self.coordinate(label)?;
        self.expect(TokenKind::Comma, "列坐标分量后需要 `,`")?;
        let z = self.coordinate(label)?;
        let end = self.expect(TokenKind::RightParen, "列坐标缺少 `)`")?.span;
        for coordinate in [&x, &z] {
            if coordinate.is_local() {
                return Err(Diagnostic::new(
                    "forceload 的列坐标不支持 `^` 局部坐标，请使用绝对坐标或 `~`",
                    start.span.merge(end),
                ));
            }
        }
        Ok(ColumnPosition {
            x,
            z,
            span: start.span.merge(end),
        })
    }

    fn optional_column_position(
        &mut self,
        label: &str,
    ) -> Result<Option<ColumnPosition>, Diagnostic> {
        if self.take(&TokenKind::Comma).is_none() {
            return Ok(None);
        }
        Ok(Some(self.column_position(label)?))
    }

    fn optional_position(&mut self) -> Result<Option<BlockPosition>, Diagnostic> {
        if self.take(&TokenKind::Comma).is_none() {
            return Ok(None);
        }
        Ok(Some(self.block_position("位置参数")?))
    }

    /// 一个坐标分量：绝对整数、`~[±数]` 或 `^[±数]`。
    fn coordinate(&mut self, label: &str) -> Result<Coordinate, Diagnostic> {
        if let Some(token) = self.take(&TokenKind::Tilde) {
            let offset = self.coordinate_offset(label, &token)?;
            return Ok(Coordinate::Relative(format!("~{offset}")));
        }
        if let Some(token) = self.take(&TokenKind::Caret) {
            let offset = self.coordinate_offset(label, &token)?;
            return Ok(Coordinate::Local(format!("^{offset}")));
        }
        let negative = self.negative_sign();
        let token = self.advance().clone();
        let TokenKind::Number(value) = token.kind else {
            return Err(Diagnostic::new(
                format!("{label}的绝对坐标需要整数，或用 `~`/`^` 写相对坐标"),
                token.span,
            ));
        };
        Ok(Coordinate::Absolute(if negative {
            format!("-{value}")
        } else {
            value.to_string()
        }))
    }

    /// `~` 或 `^` 之后的可选小数偏移；紧跟着 `,` 或 `)` 表示偏移为 0。
    fn coordinate_offset(&mut self, label: &str, prefix: &Token) -> Result<String, Diagnostic> {
        if self.check(&TokenKind::Comma) || self.check(&TokenKind::RightParen) {
            return Ok(String::new());
        }
        let negative = self.negative_sign();
        let token = self.advance().clone();
        let text = match token.kind {
            TokenKind::Number(value) => value.to_string(),
            TokenKind::Decimal(value) => format!("{value}"),
            _ => {
                return Err(Diagnostic::new(
                    format!("{label}的 `~`/`^` 偏移需要数字"),
                    prefix.span.merge(token.span),
                ));
            }
        };
        Ok(if negative { format!("-{text}") } else { text })
    }

    /// `worldborder.center` 的分量：绝对小数或 `~[±数]`，与原版 `Vec2Argument` 一致。
    fn vec2_component(&mut self, label: &str) -> Result<String, Diagnostic> {
        if let Some(token) = self.take(&TokenKind::Tilde) {
            let offset = self.coordinate_offset(label, &token)?;
            return Ok(format!("~{offset}"));
        }
        let negative = self.negative_sign();
        let token = self.advance().clone();
        let text = match token.kind {
            TokenKind::Number(value) => value.to_string(),
            TokenKind::Decimal(value) => format!("{value}"),
            _ => {
                return Err(Diagnostic::new(
                    format!("{label}需要数字或 `~`"),
                    token.span,
                ));
            }
        };
        Ok(if negative { format!("-{text}") } else { text })
    }

    /// `block_state("资源位置") { 属性 = "值"; ... }`；属性块可省略。
    pub(super) fn block_state_value(&mut self, label: &str) -> Result<BlockStateValue, Diagnostic> {
        let Some(start) = self.take_word("block_state") else {
            return Err(Diagnostic::new(
                format!(
                    "{label}需要 `block_state(\"命名空间:方块\") {{ ... }}`（中文 `方块状态(...)`）"
                ),
                self.current().span,
            ));
        };
        self.expect(TokenKind::LeftParen, "block_state 后需要 `(`")?;
        let (id, _) = self.string("block_state 需要方块资源位置字符串")?;
        let end = self
            .expect(TokenKind::RightParen, "方块资源位置后需要 `)`")?
            .span;
        let mut span = start.span.merge(end);
        let mut properties = Vec::new();
        if self.take(&TokenKind::LeftBrace).is_some() {
            while !self.check(&TokenKind::RightBrace) {
                if self.check(&TokenKind::Eof) {
                    return Err(Diagnostic::new("方块状态缺少 `}`", self.current().span));
                }
                let (name, name_span) = self.ident("方块属性名称")?;
                self.expect(TokenKind::Equal, "方块属性后需要 `=`")?;
                let (value, value_span) = self.string("方块属性值需要字符串，例如 \"east\"")?;
                self.expect(TokenKind::Semicolon, "方块属性后需要 `;`")?;
                if properties
                    .iter()
                    .any(|property: &BlockProperty| property.name == name)
                {
                    return Err(Diagnostic::new(
                        format!("方块属性 `{name}` 重复声明"),
                        name_span,
                    ));
                }
                properties.push(BlockProperty {
                    name,
                    value,
                    span: name_span.merge(value_span),
                });
            }
            let close = self.advance().span;
            self.take(&TokenKind::Semicolon);
            span = span.merge(close);
        }
        Ok(BlockStateValue {
            id,
            properties,
            span,
        })
    }

    /// 时间操作的可选世界时钟参数：`..., "minecraft:overworld"`。
    fn optional_clock(&mut self, label: &str) -> Result<Option<String>, Diagnostic> {
        if self.take(&TokenKind::Comma).is_none() {
            return Ok(None);
        }
        Ok(Some(self.string(label)?.0))
    }
}
