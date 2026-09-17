//! 顶层声明：命名空间内的计分、实体查询、存储、资源和函数。
//!
//! 物品定义在 [`super::items`]。无符号整数与资源路径的读取辅助函数也放在这里，
//! 供语句和条件子模块复用。

use crate::ast::*;
use crate::diagnostic::Diagnostic;
use crate::lexer::TokenKind;

use super::Parser;
use super::keywords::{
    advancement_frame, advancement_property, advancement_requirements, attribute_word,
    boolean_word, criterion_property, data_slot_kind, display_property, entity_sort,
    function_tag_property, gamemode_value, item_property, number_format_kind, objective_property,
    query_property, render_type as render_type_value, resource_kind, reward_property, slot_name,
};

impl Parser {
    pub(super) fn score(&mut self) -> Result<ScoreDecl, Diagnostic> {
        let start = self.expect_word("score")?.span;
        let (name, name_span) = self.ident("计分变量名称")?;
        self.expect(
            TokenKind::Equal,
            "计分变量需要初始值，例如 `score count = 0;`",
        )?;
        let sign = if self.take(&TokenKind::Minus).is_some() {
            -1_i64
        } else {
            1
        };
        let token = self.advance().clone();
        let TokenKind::Number(number) = token.kind else {
            return Err(Diagnostic::new("初始值必须是整数常量", token.span));
        };
        let signed = number
            .checked_mul(sign)
            .ok_or_else(|| Diagnostic::new("整数超出 32 位范围", token.span))?;
        let initial =
            i32::try_from(signed).map_err(|_| Diagnostic::new("整数超出 32 位范围", token.span))?;
        let end = self
            .expect(TokenKind::Semicolon, "计分变量声明后需要 `;`")?
            .span;
        Ok(ScoreDecl {
            exported: false,
            name,
            name_span,
            initial,
            span: start.merge(end),
        })
    }

    /// `objective 名称;`：声明一个用户计分板目标。
    pub(super) fn objective(&mut self) -> Result<ObjectiveDecl, Diagnostic> {
        let start = self.expect_word("objective")?.span;
        let (name, name_span) = self.ident("计分板目标名称")?;
        let mut criteria = None;
        let mut display_name = None;
        let mut render_type = None;
        let mut number_format = None;
        let mut display_slot = None;
        let mut display_slot_span = None;
        let end = if self.take(&TokenKind::LeftBrace).is_some() {
            while !self.check(&TokenKind::RightBrace) {
                if self.check(&TokenKind::Eof) {
                    return Err(Diagnostic::new(
                        "计分板目标声明缺少 `}`",
                        self.current().span,
                    ));
                }
                let (property, property_span) = self.ident("目标属性")?;
                let Some(kind) = objective_property(&property) else {
                    return Err(Diagnostic::new(
                        format!(
                            "未知目标属性 `{property}`，可用 criteria、display_name、render_type、number_format、display_slot"
                        ),
                        property_span,
                    ));
                };
                match kind {
                    "criteria" => {
                        if criteria.is_some() {
                            return Err(Diagnostic::new("准则只能声明一次", property_span));
                        }
                        self.expect(TokenKind::Equal, "准则后需要 `=`")?;
                        criteria = Some(self.string("准则需要字符串")?.0);
                    }
                    "display_name" => {
                        if display_name.is_some() {
                            return Err(Diagnostic::new("显示名只能声明一次", property_span));
                        }
                        self.expect(TokenKind::Equal, "显示名后需要 `=`")?;
                        display_name = Some(self.text_component_or_string("显示名")?);
                    }
                    "render_type" => {
                        if render_type.is_some() {
                            return Err(Diagnostic::new("渲染类型只能声明一次", property_span));
                        }
                        self.expect(TokenKind::Equal, "渲染类型后需要 `=`")?;
                        let (value, span) = self.string("渲染类型需要字符串")?;
                        render_type = Some(match render_type_value(&value) {
                            Some(value) => value.to_owned(),
                            None => {
                                return Err(Diagnostic::new(
                                    format!(
                                        "渲染类型只能是 integer/整数 或 hearts/爱心，实际为 `{value}`"
                                    ),
                                    span,
                                ));
                            }
                        });
                    }
                    "number_format" => {
                        if number_format.is_some() {
                            return Err(Diagnostic::new("数字格式只能声明一次", property_span));
                        }
                        self.expect(TokenKind::Equal, "数字格式后需要 `=`")?;
                        number_format = Some(self.number_format()?);
                    }
                    "display_slot" => {
                        if display_slot.is_some() {
                            return Err(Diagnostic::new("显示槽只能声明一次", property_span));
                        }
                        self.expect(TokenKind::Equal, "显示槽后需要 `=`")?;
                        let (value, span) = self.string("显示槽需要字符串")?;
                        display_slot = Some(value);
                        display_slot_span = Some(span);
                    }
                    _ => unreachable!("objective_property 只返回已知属性"),
                }
                self.expect(TokenKind::Semicolon, "目标属性后需要 `;`")?;
            }
            self.advance().span
        } else {
            self.expect(TokenKind::Semicolon, "计分板目标声明后需要 `;`")?
                .span
        };
        Ok(ObjectiveDecl {
            exported: false,
            name,
            name_span,
            criteria,
            display_name,
            render_type,
            number_format,
            display_slot,
            display_slot_span,
            span: start.merge(end),
        })
    }

    /// `blank`、`fixed(<组件>)` 或 `styled`。
    fn number_format(&mut self) -> Result<NumberFormat, Diagnostic> {
        let (kind, span) = self.ident("数字格式 blank、fixed 或 styled")?;
        let Some(kind) = number_format_kind(&kind) else {
            return Err(Diagnostic::new(
                format!("未知数字格式 `{kind}`，可用 blank/空白、fixed/固定 或 styled/样式"),
                span,
            ));
        };
        Ok(match kind {
            "blank" => NumberFormat::Blank,
            "styled" => NumberFormat::Styled,
            _ => {
                self.expect(TokenKind::LeftParen, "fixed 后需要 `(`")?;
                let component = self.text_component_or_string("fixed 显示内容")?;
                self.expect(TokenKind::RightParen, "fixed 后需要 `)`")?;
                NumberFormat::Fixed(Box::new(component))
            }
        })
    }

    /// `data_slot 名称 = item_data("键");` 或 `= entity_data("键");`
    pub(super) fn data_slot(&mut self) -> Result<DataSlotDecl, Diagnostic> {
        let start = self.expect_word("data_slot")?.span;
        let (name, name_span) = self.ident("数据槽名称")?;
        self.expect(TokenKind::Equal, "数据槽名称后需要 `=`")?;
        let (source, source_span) = self.ident("数据槽来源 item_data 或 entity_data")?;
        let Some(source) = data_slot_kind(&source) else {
            return Err(Diagnostic::new(
                "数据槽来源只能是 item_data/物品数据 或 entity_data/实体数据",
                source_span,
            ));
        };
        self.expect(TokenKind::LeftParen, "数据槽来源后需要 `(`")?;
        let (key, key_span) = self.string("数据槽需要键字符串")?;
        self.expect(TokenKind::RightParen, "数据槽键后需要 `)`")?;
        let end = self
            .expect(TokenKind::Semicolon, "数据槽声明后需要 `;`")?
            .span;
        Ok(DataSlotDecl {
            exported: false,
            name,
            name_span,
            kind: if source == "item_data" {
                DataSlotKind::ItemData
            } else {
                DataSlotKind::EntityData
            },
            key,
            key_span,
            span: start.merge(end),
        })
    }

    pub(super) fn query(&mut self) -> Result<EntityQueryDecl, Diagnostic> {
        let start = self.expect_word("query")?.span;
        let (name, name_span) = self.ident("查询名称")?;
        self.expect(TokenKind::Equal, "查询名称后需要 `=`")?;
        self.expect_word("entity")?;
        self.expect(TokenKind::LeftParen, "entity 后需要 `(`")?;
        let (entity_type, _) = self.string("entity 需要实体类型字符串")?;
        self.expect(TokenKind::RightParen, "实体类型后需要 `)`")?;
        self.expect(TokenKind::LeftBrace, "实体查询需要 `{`")?;

        let mut tags = Vec::new();
        let mut excluded_tags = Vec::new();
        let mut type_filters = Vec::new();
        let mut limit = None;
        let mut sort = None;
        let mut within = None;
        let mut name_filter = None;
        let mut scores = Vec::new();
        let mut nbt_filter = None;
        let mut box_filter = None;
        let mut distance = None;
        let mut level = None;
        let mut gamemode = None;
        let mut team_filter = None;
        let mut rotation = None;
        let mut predicate = None;
        let mut advancements = None;
        let mut item = None;
        while !self.check(&TokenKind::RightBrace) {
            if self.check(&TokenKind::Eof) {
                return Err(Diagnostic::new("实体查询缺少 `}`", self.current().span));
            }
            let (property, property_span) = self.ident("查询属性")?;
            let Some(property_kind) = query_property(&property) else {
                return Err(Diagnostic::new(
                    format!("未知实体查询属性 `{property}`"),
                    property_span,
                ));
            };
            match property_kind {
                "tag" | "without_tag" => {
                    self.expect(TokenKind::LeftParen, "查询属性后需要 `(`")?;
                    let (value, _) = self.string("标签需要字符串")?;
                    self.expect(TokenKind::RightParen, "标签后需要 `)`")?;
                    self.expect(TokenKind::Semicolon, "查询属性后需要 `;`")?;
                    if property_kind == "tag" {
                        tags.push(value);
                    } else {
                        excluded_tags.push(value);
                    }
                }
                "type" | "without_type" => {
                    self.expect(TokenKind::LeftParen, "type 后需要 `(`")?;
                    let (value, span) = self.string("type 需要实体类型或 #标签字符串")?;
                    self.expect(TokenKind::RightParen, "type 后需要 `)`")?;
                    self.expect(TokenKind::Semicolon, "查询属性后需要 `;`")?;
                    type_filters.push(if property_kind == "type" {
                        EntityTypeFilter::Include(value, span)
                    } else {
                        EntityTypeFilter::Exclude(value, span)
                    });
                }
                "limit" => {
                    if limit.is_some() {
                        return Err(Diagnostic::new("查询只能声明一次 limit", property_span));
                    }
                    limit = Some(self.unsigned_call("limit")?);
                }
                "within" => {
                    if within.is_some() {
                        return Err(Diagnostic::new("查询只能声明一次 within", property_span));
                    }
                    within = Some(self.unsigned_call("within")?);
                }
                "sort" => {
                    if sort.is_some() {
                        return Err(Diagnostic::new("查询只能声明一次 sort", property_span));
                    }
                    self.expect(TokenKind::LeftParen, "sort 后需要 `(`")?;
                    let (value, span) = self.ident("排序方式")?;
                    sort = Some(match entity_sort(&value) {
                        Some("nearest") => EntitySort::Nearest,
                        Some("furthest") => EntitySort::Furthest,
                        Some("random") => EntitySort::Random,
                        Some("arbitrary") => EntitySort::Arbitrary,
                        _ => {
                            return Err(Diagnostic::new(
                                "排序方式只能是 nearest/最近、furthest/最远、random/随机或 arbitrary/任意",
                                span,
                            ));
                        }
                    });
                    self.expect(TokenKind::RightParen, "排序方式后需要 `)`")?;
                    self.expect(TokenKind::Semicolon, "查询属性后需要 `;`")?;
                }
                "name" | "without_name" | "nbt" | "without_nbt" | "team" | "without_team" => {
                    let target = match property_kind {
                        "name" | "without_name" => &mut name_filter,
                        "nbt" | "without_nbt" => &mut nbt_filter,
                        _ => &mut team_filter,
                    };
                    if target.is_some() {
                        return Err(Diagnostic::new(
                            format!("查询只能声明一次 {property}"),
                            property_span,
                        ));
                    }
                    self.expect(TokenKind::LeftParen, "查询属性后需要 `(`")?;
                    let (value, span) = self.string("查询属性需要字符串")?;
                    self.expect(TokenKind::RightParen, "查询属性后需要 `)`")?;
                    self.expect(TokenKind::Semicolon, "查询属性后需要 `;`")?;
                    *target = Some(QueryTextFilter {
                        value,
                        negated: property_kind.starts_with("without"),
                        span,
                    });
                }
                "scores" => {
                    self.expect(TokenKind::LeftParen, "scores 后需要 `(`")?;
                    let (objective, _) = self.string("scores 需要计分板目标字符串")?;
                    self.expect(TokenKind::Comma, "scores 目标后需要 `,`")?;
                    let (range, span) = self.string("scores 需要区间字符串，例如 \"1..5\"")?;
                    self.expect(TokenKind::RightParen, "scores 后需要 `)`")?;
                    self.expect(TokenKind::Semicolon, "查询属性后需要 `;`")?;
                    scores.push(QueryScoreFilter {
                        objective,
                        range,
                        span,
                    });
                }
                "box" => {
                    if box_filter.is_some() {
                        return Err(Diagnostic::new("查询只能声明一次 box", property_span));
                    }
                    box_filter = Some(self.query_box(property_span)?);
                }
                "distance" | "level" | "advancements" => {
                    let target = match property_kind {
                        "distance" => &mut distance,
                        "level" => &mut level,
                        _ => &mut advancements,
                    };
                    if target.is_some() {
                        return Err(Diagnostic::new(
                            format!("查询只能声明一次 {property_kind}"),
                            property_span,
                        ));
                    }
                    *target = Some(self.string_call(property_kind)?);
                }
                "gamemode" => {
                    if gamemode.is_some() {
                        return Err(Diagnostic::new("查询只能声明一次 gamemode", property_span));
                    }
                    let value = self.string_call("gamemode")?;
                    gamemode = Some(gamemode_value(&value).unwrap_or(&value).to_owned());
                }
                "rotate" => {
                    if rotation.is_some() {
                        return Err(Diagnostic::new("查询只能声明一次 rotate", property_span));
                    }
                    self.expect(TokenKind::LeftParen, "rotate 后需要 `(`")?;
                    let (yaw, _) = self.string("rotate 需要偏航区间字符串")?;
                    self.expect(TokenKind::Comma, "rotate 偏航后需要 `,`")?;
                    let (pitch, _) = self.string("rotate 需要俯仰区间字符串")?;
                    let end = self
                        .expect(TokenKind::RightParen, "rotate 后需要 `)`")?
                        .span;
                    self.expect(TokenKind::Semicolon, "查询属性后需要 `;`")?;
                    rotation = Some(QueryRotation {
                        yaw,
                        pitch,
                        span: property_span.merge(end),
                    });
                }
                "predicate" => {
                    if predicate.is_some() {
                        return Err(Diagnostic::new("查询只能声明一次 predicate", property_span));
                    }
                    predicate = Some(self.string_call("predicate")?);
                }
                "item" => {
                    if item.is_some() {
                        return Err(Diagnostic::new(
                            "查询只能声明一个 item 过滤器",
                            property_span,
                        ));
                    }
                    item = Some(self.item_filter(property_span)?);
                }
                _ => unreachable!(),
            }
        }
        let end = self.advance().span;
        self.take(&TokenKind::Semicolon);
        Ok(EntityQueryDecl {
            exported: false,
            name,
            name_span,
            entity_type,
            type_filters,
            tags,
            excluded_tags,
            limit,
            sort,
            within,
            name_filter,
            scores,
            nbt_filter,
            box_filter,
            distance,
            level,
            gamemode,
            team_filter,
            rotation,
            predicate,
            advancements,
            item,
            span: start.merge(end),
        })
    }

    /// `属性("字符串");`：读取一个字符串参数并消费分号。
    fn string_call(&mut self, property: &str) -> Result<String, Diagnostic> {
        self.expect(TokenKind::LeftParen, "查询属性后需要 `(`")?;
        let (value, _) = self.string(&format!("{property} 需要字符串"))?;
        self.expect(TokenKind::RightParen, "查询属性后需要 `)`")?;
        self.expect(TokenKind::Semicolon, "查询属性后需要 `;`")?;
        Ok(value)
    }

    /// `box(x, y, z, dx, dy, dz);`：选择器坐标盒。
    fn query_box(&mut self, start: Span) -> Result<QueryBox, Diagnostic> {
        self.expect(TokenKind::LeftParen, "box 后需要 `(`")?;
        let x = self.signed_number_text("box x")?;
        self.expect(TokenKind::Comma, "box 分量之间需要 `,`")?;
        let y = self.signed_number_text("box y")?;
        self.expect(TokenKind::Comma, "box 分量之间需要 `,`")?;
        let z = self.signed_number_text("box z")?;
        self.expect(TokenKind::Comma, "box 分量之间需要 `,`")?;
        let dx = self.signed_number_text("box dx")?;
        self.expect(TokenKind::Comma, "box 分量之间需要 `,`")?;
        let dy = self.signed_number_text("box dy")?;
        self.expect(TokenKind::Comma, "box 分量之间需要 `,`")?;
        let dz = self.signed_number_text("box dz")?;
        let end = self.expect(TokenKind::RightParen, "box 后需要 `)`")?.span;
        self.expect(TokenKind::Semicolon, "查询属性后需要 `;`")?;
        Ok(QueryBox {
            x,
            y,
            z,
            dx,
            dy,
            dz,
            span: start.merge(end),
        })
    }

    fn item_filter(&mut self, start: Span) -> Result<ItemFilter, Diagnostic> {
        self.expect(TokenKind::LeftParen, "item 后需要 `(`")?;
        let slot = if matches!(self.current().kind, TokenKind::String(_)) {
            self.string("物品槽名称需要字符串或标识符")?.0
        } else {
            let (first, _) = self.ident("物品槽名称")?;
            let mut slot = first;
            while self.take(&TokenKind::Dot).is_some() {
                let (part, _) = self.ident("槽位分量")?;
                slot.push('.');
                slot.push_str(&part);
            }
            slot
        };
        let slot = slot_name(&slot).unwrap_or(&slot).to_owned();
        self.expect(TokenKind::RightParen, "物品槽后需要 `)`")?;
        self.expect(TokenKind::LeftBrace, "item 过滤器需要 `{`")?;
        let mut item_id = None;
        let mut count = None;
        let mut custom_name = None;
        while !self.check(&TokenKind::RightBrace) {
            if self.check(&TokenKind::Eof) {
                return Err(Diagnostic::new("item 过滤器缺少 `}`", self.current().span));
            }
            let (property, span) = self.ident("item 属性")?;
            self.expect(TokenKind::Equal, "item 属性后需要 `=`")?;
            let Some(property_kind) = item_property(&property) else {
                return Err(Diagnostic::new(
                    format!("未知 item 属性 `{property}`"),
                    span,
                ));
            };
            match property_kind {
                "id" => {
                    if item_id.is_some() {
                        return Err(Diagnostic::new("item.id 只能声明一次", span));
                    }
                    item_id = Some(self.string("item.id 需要物品类型字符串")?.0);
                }
                "count" => {
                    if count.is_some() {
                        return Err(Diagnostic::new("item.count 只能声明一次", span));
                    }
                    count = Some(self.unsigned("item.count")?);
                }
                "custom_name" => {
                    if custom_name.is_some() {
                        return Err(Diagnostic::new("item.custom_name 只能声明一次", span));
                    }
                    custom_name = Some(self.string("item.custom_name 需要文本字符串")?.0);
                }
                _ => unreachable!(),
            }
            self.expect(TokenKind::Semicolon, "item 属性后需要 `;`")?;
        }
        let end = self.advance().span;
        let Some(item_id) = item_id else {
            return Err(Diagnostic::new("item 过滤器必须声明 id", start.merge(end)));
        };
        Ok(ItemFilter {
            slot,
            item_id,
            count,
            custom_name,
            span: start.merge(end),
        })
    }

    pub(super) fn storage(&mut self) -> Result<StorageDecl, Diagnostic> {
        let start = self.expect_word("storage")?.span;
        let (name, name_span) = self.ident("存储名称")?;
        self.expect(TokenKind::Equal, "存储名称后需要 `=`")?;
        self.expect_word("item_list")?;
        self.expect(TokenKind::LeftParen, "item_list 后需要 `(`")?;
        let (storage_id, _) = self.string("item_list 需要存储资源位置")?;
        self.expect(TokenKind::Comma, "存储资源位置后需要 `,`")?;
        let (path, _) = self.string("item_list 需要 NBT 路径")?;
        self.expect(TokenKind::RightParen, "存储声明缺少 `)`")?;
        let end = self
            .expect(TokenKind::Semicolon, "存储声明后需要 `;`")?
            .span;
        Ok(StorageDecl {
            exported: false,
            name,
            name_span,
            storage_id,
            path,
            span: start.merge(end),
        })
    }

    /// `fn_tag 名称 { value(函数或#标签); replace = 真; }`
    pub(super) fn function_tag(&mut self) -> Result<FunctionTagDecl, Diagnostic> {
        let start = self.expect_word("fn_tag")?.span;
        let (name, name_span) = self.ident("函数标签名称")?;
        self.expect(TokenKind::LeftBrace, "函数标签需要 `{`")?;
        let mut values = Vec::new();
        let mut replace = false;
        let mut replace_span = None;
        while !self.check(&TokenKind::RightBrace) {
            if self.check(&TokenKind::Eof) {
                return Err(Diagnostic::new("函数标签缺少 `}`", self.current().span));
            }
            let (property, property_span) = self.ident("函数标签属性")?;
            let Some(property_kind) = function_tag_property(&property) else {
                return Err(Diagnostic::new(
                    format!("未知函数标签属性 `{property}`"),
                    property_span,
                ));
            };
            match property_kind {
                "value" => {
                    self.expect(TokenKind::LeftParen, "value 后需要 `(`")?;
                    values.push(self.function_tag_entry()?);
                    self.expect(TokenKind::RightParen, "标签条目后需要 `)`")?;
                    self.expect(TokenKind::Semicolon, "标签条目后需要 `;`")?;
                }
                "replace" => {
                    if replace_span.is_some() {
                        return Err(Diagnostic::new(
                            "函数标签只能声明一次 replace",
                            property_span,
                        ));
                    }
                    self.expect(TokenKind::Equal, "replace 后需要 `=`")?;
                    let (value, value_span) = self.ident("true 或 false")?;
                    replace = match boolean_word(&value) {
                        Some("true") => true,
                        Some("false") => false,
                        _ => return Err(Diagnostic::new("这里需要 true 或 false", value_span)),
                    };
                    self.expect(TokenKind::Semicolon, "replace 后需要 `;`")?;
                    replace_span = Some(property_span);
                }
                _ => unreachable!(),
            }
        }
        let end = self.advance().span;
        self.take(&TokenKind::Semicolon);
        Ok(FunctionTagDecl {
            exported: false,
            name,
            name_span,
            values,
            replace,
            span: start.merge(end),
        })
    }

    /// 标签条目：本命名空间函数、`#` 本命名空间标签或字符串形式的外部资源位置。
    fn function_tag_entry(&mut self) -> Result<FunctionTagEntry, Diagnostic> {
        if let Some(hash) = self.take(&TokenKind::Hash) {
            let token = self.advance().clone();
            match token.kind {
                TokenKind::Ident(value) => {
                    Ok(FunctionTagEntry::Tag(value, hash.span.merge(token.span)))
                }
                TokenKind::String(value) => Ok(FunctionTagEntry::External(
                    format!("#{value}"),
                    hash.span.merge(token.span),
                )),
                _ => Err(Diagnostic::new(
                    "`#` 后需要函数标签名称或字符串资源位置",
                    token.span,
                )),
            }
        } else {
            let token = self.advance().clone();
            match token.kind {
                TokenKind::Ident(value) => Ok(FunctionTagEntry::Function(value, token.span)),
                TokenKind::String(value) => Ok(FunctionTagEntry::External(value, token.span)),
                _ => Err(Diagnostic::new(
                    "标签条目需要函数名称、`#标签` 或字符串资源位置",
                    token.span,
                )),
            }
        }
    }

    pub(super) fn resource(&mut self) -> Result<ResourceDecl, Diagnostic> {
        let start = self.expect_word("resource")?.span;
        let (kind, _) = self.resource_path("资源类型")?;
        let kind = resource_kind(&kind).unwrap_or(&kind).to_owned();
        let (name, name_span) = self.resource_path("资源名称")?;
        self.expect(TokenKind::Equal, "资源名称后需要 `=`")?;
        let (json, _) = self.string("资源内容需要 JSON 字符串")?;
        let end = self
            .expect(TokenKind::Semicolon, "资源声明后需要 `;`")?
            .span;
        Ok(ResourceDecl {
            exported: false,
            kind,
            name,
            name_span,
            json,
            span: start.merge(end),
        })
    }

    pub(super) fn resource_path(&mut self, expected: &str) -> Result<(String, Span), Diagnostic> {
        let token = self.advance().clone();
        match token.kind {
            TokenKind::Ident(value) | TokenKind::String(value) => Ok((value, token.span)),
            _ => Err(Diagnostic::new(
                format!("这里需要{expected}标识符或字符串"),
                token.span,
            )),
        }
    }

    /// `advancement 名称 { parent = …; criterion …; reward {…}; display {…}; }`
    ///
    /// 进度是数据包的事件入口：`criterion` 监听原版触发器，命中后由
    /// `reward.function` 指向的函数接管。条件本身是触发器的原始 JSON，
    /// 触发器名、奖励引用与展示字段在编译期检查。
    pub(super) fn advancement(&mut self) -> Result<AdvancementDecl, Diagnostic> {
        let start = self.expect_word("advancement")?.span;
        let (name, name_span) = self.ident("进度名称")?;
        self.expect(TokenKind::LeftBrace, "进度声明需要 `{`")?;
        let mut parent = None;
        let mut criteria = Vec::new();
        let mut requirements = AdvancementRequirements::All;
        let mut requirements_span = None;
        let mut reward = None;
        let mut display = None;
        while !self.check(&TokenKind::RightBrace) {
            if self.check(&TokenKind::Eof) {
                return Err(Diagnostic::new("进度声明缺少 `}`", self.current().span));
            }
            let (property, property_span) = self.ident("进度属性")?;
            let Some(property) = advancement_property(&property) else {
                return Err(Diagnostic::new(
                    format!("未知进度属性 `{property}`"),
                    property_span,
                ));
            };
            match property {
                "parent" => {
                    if parent.is_some() {
                        return Err(Diagnostic::new("进度只能声明一次 parent", property_span));
                    }
                    self.expect(TokenKind::Equal, "parent 后需要 `=`")?;
                    parent = Some(self.advancement_reference("父进度")?);
                    self.expect(TokenKind::Semicolon, "parent 后需要 `;`")?;
                }
                "criterion" => criteria.push(self.advancement_criterion(property_span)?),
                "requirements" => {
                    if requirements_span.is_some() {
                        return Err(Diagnostic::new(
                            "进度只能声明一次 requirements",
                            property_span,
                        ));
                    }
                    self.expect(TokenKind::Equal, "requirements 后需要 `=`")?;
                    let (value, value_span) = self.ident("all 或 any")?;
                    let Some(value) = advancement_requirements(&value) else {
                        return Err(Diagnostic::new(
                            "requirements 只能是 all/全部 或 any/任意",
                            value_span,
                        ));
                    };
                    self.expect(TokenKind::Semicolon, "requirements 后需要 `;`")?;
                    requirements = value;
                    requirements_span = Some(property_span);
                }
                "reward" => {
                    if reward.is_some() {
                        return Err(Diagnostic::new("进度只能声明一次 reward", property_span));
                    }
                    reward = Some(self.advancement_reward()?);
                }
                "display" => {
                    if display.is_some() {
                        return Err(Diagnostic::new("进度只能声明一次 display", property_span));
                    }
                    display = Some(self.advancement_display()?);
                }
                _ => unreachable!(),
            }
        }
        let end = self.advance().span;
        self.take(&TokenKind::Semicolon);
        if criteria.is_empty() {
            return Err(Diagnostic::new(
                "进度必须声明至少一条 criterion",
                start.merge(end),
            ));
        }
        Ok(AdvancementDecl {
            exported: false,
            name,
            name_span,
            parent,
            criteria,
            requirements,
            reward,
            display,
            span: start.merge(end),
        })
    }

    /// 资源引用：标识符是本命名空间声明名，字符串是外部资源位置。
    pub(super) fn advancement_reference(
        &mut self,
        label: &str,
    ) -> Result<AdvancementReference, Diagnostic> {
        let token = self.advance().clone();
        match token.kind {
            TokenKind::Ident(value) => Ok(AdvancementReference {
                name: value,
                span: token.span,
                external: false,
            }),
            TokenKind::String(value) => Ok(AdvancementReference {
                name: value,
                span: token.span,
                external: true,
            }),
            _ => Err(Diagnostic::new(
                format!("这里需要{label}名称或资源位置字符串"),
                token.span,
            )),
        }
    }

    /// `criterion 名称 { trigger = …; conditions = """…"""; }`
    fn advancement_criterion(&mut self, start: Span) -> Result<AdvancementCriterion, Diagnostic> {
        let (name, name_span) = self.ident("准则名称")?;
        self.expect(TokenKind::LeftBrace, "准则需要 `{`")?;
        let mut trigger = None;
        let mut conditions = None;
        while !self.check(&TokenKind::RightBrace) {
            if self.check(&TokenKind::Eof) {
                return Err(Diagnostic::new("准则缺少 `}`", self.current().span));
            }
            let (property, property_span) = self.ident("准则属性")?;
            let Some(property) = criterion_property(&property) else {
                return Err(Diagnostic::new(
                    format!("未知准则属性 `{property}`"),
                    property_span,
                ));
            };
            match property {
                "trigger" => {
                    if trigger.is_some() {
                        return Err(Diagnostic::new("准则只能声明一次 trigger", property_span));
                    }
                    self.expect(TokenKind::Equal, "trigger 后需要 `=`")?;
                    let token = self.advance().clone();
                    let value = match token.kind {
                        TokenKind::Ident(value) | TokenKind::String(value) => value,
                        _ => {
                            return Err(Diagnostic::new("trigger 需要触发器名称", token.span));
                        }
                    };
                    self.expect(TokenKind::Semicolon, "trigger 后需要 `;`")?;
                    trigger = Some((value, token.span));
                }
                "conditions" => {
                    if conditions.is_some() {
                        return Err(Diagnostic::new(
                            "准则只能声明一次 conditions",
                            property_span,
                        ));
                    }
                    self.expect(TokenKind::Equal, "conditions 后需要 `=`")?;
                    let (json, span) = self.string("conditions 需要 JSON 字符串")?;
                    self.expect(TokenKind::Semicolon, "conditions 后需要 `;`")?;
                    conditions = Some((json, span));
                }
                _ => unreachable!(),
            }
        }
        let end = self.advance().span;
        let span = start.merge(end);
        let Some((trigger, trigger_span)) = trigger else {
            return Err(Diagnostic::new("准则必须声明 trigger", span));
        };
        Ok(AdvancementCriterion {
            name,
            name_span,
            trigger,
            trigger_span,
            conditions: conditions.as_ref().map(|(json, _)| json.clone()),
            conditions_span: conditions.map(|(_, span)| span),
            span,
        })
    }

    /// `reward { function = …; experience = …; loot = …; recipe = …; }`
    fn advancement_reward(&mut self) -> Result<AdvancementReward, Diagnostic> {
        self.expect(TokenKind::LeftBrace, "reward 后需要 `{`")?;
        let mut function = None;
        let mut experience = None;
        let mut loot = Vec::new();
        let mut recipes = Vec::new();
        while !self.check(&TokenKind::RightBrace) {
            if self.check(&TokenKind::Eof) {
                return Err(Diagnostic::new("reward 缺少 `}`", self.current().span));
            }
            let (property, property_span) = self.ident("奖励属性")?;
            let Some(property) = reward_property(&property) else {
                return Err(Diagnostic::new(
                    format!("未知奖励属性 `{property}`"),
                    property_span,
                ));
            };
            match property {
                "function" => {
                    if function.is_some() {
                        return Err(Diagnostic::new(
                            "reward 只能声明一次 function",
                            property_span,
                        ));
                    }
                    self.expect(TokenKind::Equal, "function 后需要 `=`")?;
                    function = Some(self.advancement_reference("奖励函数")?);
                    self.expect(TokenKind::Semicolon, "function 后需要 `;`")?;
                }
                "experience" => {
                    if experience.is_some() {
                        return Err(Diagnostic::new(
                            "reward 只能声明一次 experience",
                            property_span,
                        ));
                    }
                    self.expect(TokenKind::Equal, "experience 后需要 `=`")?;
                    experience = Some(self.signed("experience 经验值")?);
                    self.expect(TokenKind::Semicolon, "experience 后需要 `;`")?;
                }
                "loot" => {
                    self.expect(TokenKind::Equal, "loot 后需要 `=`")?;
                    loot.push(self.advancement_reference("战利品表")?);
                    self.expect(TokenKind::Semicolon, "loot 后需要 `;`")?;
                }
                "recipe" => {
                    self.expect(TokenKind::Equal, "recipe 后需要 `=`")?;
                    recipes.push(self.advancement_reference("配方")?);
                    self.expect(TokenKind::Semicolon, "recipe 后需要 `;`")?;
                }
                _ => unreachable!(),
            }
        }
        self.advance();
        Ok(AdvancementReward {
            function,
            experience,
            loot,
            recipes,
        })
    }

    /// `display { icon = …; title = "…"; description = "…"; … }`
    fn advancement_display(&mut self) -> Result<AdvancementDisplay, Diagnostic> {
        let start = self
            .expect(TokenKind::LeftBrace, "display 后需要 `{`")?
            .span;
        let mut icon = None;
        let mut title = None;
        let mut description = None;
        let mut frame = AdvancementFrame::Task;
        let mut frame_span = None;
        let mut background = None;
        let mut background_span = None;
        let mut show_toast = None;
        let mut announce_to_chat = None;
        let mut hidden = None;
        while !self.check(&TokenKind::RightBrace) {
            if self.check(&TokenKind::Eof) {
                return Err(Diagnostic::new("display 缺少 `}`", self.current().span));
            }
            let (property, property_span) = self.ident("展示属性")?;
            let Some(property) = display_property(&property) else {
                return Err(Diagnostic::new(
                    format!("未知展示属性 `{property}`"),
                    property_span,
                ));
            };
            match property {
                "icon" => {
                    if icon.is_some() {
                        return Err(Diagnostic::new("display 只能声明一次 icon", property_span));
                    }
                    self.expect(TokenKind::Equal, "icon 后需要 `=`")?;
                    icon = Some(self.ident("图标物品定义")?);
                    self.expect(TokenKind::Semicolon, "icon 后需要 `;`")?;
                }
                "title" => {
                    if title.is_some() {
                        return Err(Diagnostic::new("display 只能声明一次 title", property_span));
                    }
                    self.expect(TokenKind::Equal, "title 后需要 `=`")?;
                    title = Some(self.string("title 需要文本字符串")?);
                    self.expect(TokenKind::Semicolon, "title 后需要 `;`")?;
                }
                "description" => {
                    if description.is_some() {
                        return Err(Diagnostic::new(
                            "display 只能声明一次 description",
                            property_span,
                        ));
                    }
                    self.expect(TokenKind::Equal, "description 后需要 `=`")?;
                    description = Some(self.string("description 需要文本字符串")?);
                    self.expect(TokenKind::Semicolon, "description 后需要 `;`")?;
                }
                "frame" => {
                    if frame_span.is_some() {
                        return Err(Diagnostic::new("display 只能声明一次 frame", property_span));
                    }
                    self.expect(TokenKind::Equal, "frame 后需要 `=`")?;
                    let (value, value_span) = self.ident("task、goal 或 challenge")?;
                    let Some(value) = advancement_frame(&value) else {
                        return Err(Diagnostic::new(
                            "frame 只能是 task/任务、goal/目标 或 challenge/挑战",
                            value_span,
                        ));
                    };
                    self.expect(TokenKind::Semicolon, "frame 后需要 `;`")?;
                    frame = value;
                    frame_span = Some(property_span);
                }
                "background" => {
                    if background_span.is_some() {
                        return Err(Diagnostic::new(
                            "display 只能声明一次 background",
                            property_span,
                        ));
                    }
                    self.expect(TokenKind::Equal, "background 后需要 `=`")?;
                    let (value, value_span) = self.string("background 需要纹理资源位置")?;
                    self.expect(TokenKind::Semicolon, "background 后需要 `;`")?;
                    background = Some(value);
                    background_span = Some(value_span);
                }
                "show_toast" | "announce_to_chat" | "hidden" => {
                    let slot = match property {
                        "show_toast" => &mut show_toast,
                        "announce_to_chat" => &mut announce_to_chat,
                        _ => &mut hidden,
                    };
                    if slot.is_some() {
                        return Err(Diagnostic::new(
                            format!("display 只能声明一次 {property}"),
                            property_span,
                        ));
                    }
                    self.expect(TokenKind::Equal, &format!("{property} 后需要 `=`"))?;
                    let (value, value_span) = self.ident("true 或 false")?;
                    *slot = Some(match boolean_word(&value) {
                        Some("true") => true,
                        Some("false") => false,
                        _ => return Err(Diagnostic::new("这里需要 true 或 false", value_span)),
                    });
                    self.expect(TokenKind::Semicolon, &format!("{property} 后需要 `;`"))?;
                }
                _ => unreachable!(),
            }
        }
        let end = self.advance().span;
        let span = start.merge(end);
        let Some((icon, icon_span)) = icon else {
            return Err(Diagnostic::new("display 必须声明 icon", span));
        };
        let Some((title, _)) = title else {
            return Err(Diagnostic::new("display 必须声明 title", span));
        };
        let Some((description, _)) = description else {
            return Err(Diagnostic::new("display 必须声明 description", span));
        };
        Ok(AdvancementDisplay {
            icon,
            icon_span,
            title,
            description,
            frame,
            background,
            background_span,
            show_toast,
            announce_to_chat,
            hidden,
            span,
        })
    }

    pub(super) fn function(&mut self) -> Result<Function, Diagnostic> {
        let mut attributes = Vec::new();
        let start = self.current().span;
        while self.take(&TokenKind::At).is_some() {
            let (attribute, span) = self.ident("属性名称")?;
            let Some(attribute) = attribute_word(&attribute) else {
                return Err(Diagnostic::new(
                    format!("未知函数属性 `@{attribute}`"),
                    span,
                ));
            };
            attributes.push(attribute);
        }
        self.expect_word("fn")?;
        let (name, name_span) = self.ident("函数名称")?;
        self.expect(TokenKind::LeftParen, "函数名称后需要 `(`")?;
        let mut parameters = Vec::new();
        if !self.check(&TokenKind::RightParen) {
            loop {
                let (name, span) = self.ident("参数名称")?;
                parameters.push(Parameter { name, span });
                if self.take(&TokenKind::Comma).is_none() {
                    break;
                }
            }
        }
        self.expect(TokenKind::RightParen, "参数列表缺少 `)`")?;
        let returns_score = if self.take(&TokenKind::Arrow).is_some() {
            self.expect_word("score")?;
            true
        } else {
            false
        };
        let (body, end) = self.block()?;
        Ok(Function {
            exported: false,
            name,
            name_span,
            parameters,
            returns_score,
            attributes,
            body,
            span: start.merge(end),
        })
    }

    fn unsigned_call(&mut self, name: &str) -> Result<u32, Diagnostic> {
        self.expect(TokenKind::LeftParen, &format!("{name} 后需要 `(`"))?;
        let value = self.unsigned(name)?;
        self.expect(TokenKind::RightParen, &format!("{name} 后需要 `)`"))?;
        self.expect(TokenKind::Semicolon, "查询属性后需要 `;`")?;
        Ok(value)
    }

    pub(super) fn unsigned(&mut self, name: &str) -> Result<u32, Diagnostic> {
        self.unsigned_with_span(name).map(|(value, _)| value)
    }

    /// 读取可带负号的整数；供 `xp add` 这类允许减少的命令使用。
    pub(super) fn signed(&mut self, name: &str) -> Result<i32, Diagnostic> {
        let minus = self.take(&TokenKind::Minus);
        let token = self.advance().clone();
        let TokenKind::Number(value) = token.kind else {
            return Err(Diagnostic::new(format!("{name} 需要整数"), token.span));
        };
        let signed = if minus.is_some() {
            value
                .checked_neg()
                .ok_or_else(|| Diagnostic::new(format!("{name} 超出 32 位范围"), token.span))?
        } else {
            value
        };
        i32::try_from(signed)
            .map_err(|_| Diagnostic::new(format!("{name} 超出 32 位范围"), token.span))
    }

    /// 读取可带符号的整数或小数并返回规范文本，供原版双精度参数使用。
    ///
    /// 直接保留源文本可以避免把 64 位整数未经检查地转成 `f64`，输出也更贴近手写命令。
    pub(super) fn signed_number_text(&mut self, name: &str) -> Result<String, Diagnostic> {
        let negative = self.negative_sign();
        let token = self.advance().clone();
        let text = match token.kind {
            TokenKind::Number(value) => value.to_string(),
            TokenKind::Decimal(value) => format!("{value}"),
            _ => return Err(Diagnostic::new(format!("{name} 需要数字"), token.span)),
        };
        Ok(if negative { format!("-{text}") } else { text })
    }

    /// 读取可选的正负号；原版参数接受 `+5`，这里把正号规范化为无符号。
    pub(super) fn negative_sign(&mut self) -> bool {
        if self.take(&TokenKind::Minus).is_some() {
            return true;
        }
        self.take(&TokenKind::Plus);
        false
    }

    pub(super) fn unsigned_with_span(&mut self, name: &str) -> Result<(u32, Span), Diagnostic> {
        let token = self.advance().clone();
        let TokenKind::Number(value) = token.kind else {
            return Err(Diagnostic::new(format!("{name} 需要非负整数"), token.span));
        };
        let value = u32::try_from(value)
            .map_err(|_| Diagnostic::new(format!("{name} 超出范围"), token.span))?;
        Ok((value, token.span))
    }
}
