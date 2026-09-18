use crate::ast::*;
use crate::diagnostic::Diagnostic;
use crate::lexer::TokenKind;

use super::Parser;
use super::TemplateOptions;
use crate::parser::keywords::{
    clone_dimension, clone_filter, clone_mode, fill_mode, place_method, set_block_mode,
    strict_word, template_mirror, template_rotation,
};

impl Parser {
    pub(in crate::parser) fn set_block_statement(&mut self) -> Result<StatementKind, Diagnostic> {
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
    pub(in crate::parser) fn fill_statement(&mut self) -> Result<StatementKind, Diagnostic> {
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
    pub(in crate::parser) fn fill_biome_statement(&mut self) -> Result<StatementKind, Diagnostic> {
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
    pub(in crate::parser) fn clone_statement(&mut self) -> Result<StatementKind, Diagnostic> {
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
    pub(in crate::parser) fn place_statement(&mut self) -> Result<StatementKind, Diagnostic> {
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
}
