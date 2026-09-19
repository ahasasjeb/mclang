use crate::ast::*;
use crate::diagnostic::Diagnostic;
use crate::lexer::TokenKind;

use crate::parser::Parser;
use crate::parser::keywords::{
    entity_sort, gamemode_value, item_property, query_property, slot_name,
};

impl Parser {
    pub(in crate::parser) fn query(&mut self) -> Result<EntityQueryDecl, Diagnostic> {
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
                    item_id = Some(self.item_predicate()?);
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

    pub(in crate::parser) fn storage(&mut self) -> Result<StorageDecl, Diagnostic> {
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
}
