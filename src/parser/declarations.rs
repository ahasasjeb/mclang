//! 顶层声明：命名空间内的计分、实体查询、存储、资源和函数。
//!
//! 物品定义在 [`super::items`]。无符号整数与资源路径的读取辅助函数也放在这里，
//! 供语句和条件子模块复用。

use crate::ast::*;
use crate::diagnostic::Diagnostic;
use crate::lexer::TokenKind;

use super::Parser;
use super::keywords::{
    attribute_word, boolean_word, entity_sort, function_tag_property, item_property,
    query_property, resource_kind, slot_name,
};

impl Parser {
    pub(super) fn score(&mut self) -> Result<ScoreDecl, Diagnostic> {
        let start = self.expect_word("score")?.span;
        let (name, _) = self.ident("计分变量名称")?;
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
            name,
            initial,
            span: start.merge(end),
        })
    }

    pub(super) fn query(&mut self) -> Result<EntityQueryDecl, Diagnostic> {
        let start = self.expect_word("query")?.span;
        let (name, _) = self.ident("查询名称")?;
        self.expect(TokenKind::Equal, "查询名称后需要 `=`")?;
        self.expect_word("entity")?;
        self.expect(TokenKind::LeftParen, "entity 后需要 `(`")?;
        let (entity_type, _) = self.string("entity 需要实体类型字符串")?;
        self.expect(TokenKind::RightParen, "实体类型后需要 `)`")?;
        self.expect(TokenKind::LeftBrace, "实体查询需要 `{`")?;

        let mut tags = Vec::new();
        let mut excluded_tags = Vec::new();
        let mut limit = None;
        let mut sort = None;
        let mut within = None;
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
            name,
            entity_type,
            tags,
            excluded_tags,
            limit,
            sort,
            within,
            item,
            span: start.merge(end),
        })
    }

    fn item_filter(&mut self, start: Span) -> Result<ItemFilter, Diagnostic> {
        self.expect(TokenKind::LeftParen, "item 后需要 `(`")?;
        let (slot, _) = self.ident("物品槽名称")?;
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
        let (name, _) = self.ident("存储名称")?;
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
            name,
            storage_id,
            path,
            span: start.merge(end),
        })
    }

    /// `fn_tag 名称 { value(函数或#标签); replace = 真; }`
    pub(super) fn function_tag(&mut self) -> Result<FunctionTagDecl, Diagnostic> {
        let start = self.expect_word("fn_tag")?.span;
        let (name, _) = self.ident("函数标签名称")?;
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
            name,
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
        let (name, _) = self.resource_path("资源名称")?;
        self.expect(TokenKind::Equal, "资源名称后需要 `=`")?;
        let (json, _) = self.string("资源内容需要 JSON 字符串")?;
        let end = self
            .expect(TokenKind::Semicolon, "资源声明后需要 `;`")?
            .span;
        Ok(ResourceDecl {
            kind,
            name,
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
        let (name, _) = self.ident("函数名称")?;
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
            name,
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

    /// 读取可带负号的整数或小数并返回规范文本，供原版双精度参数使用。
    ///
    /// 直接保留源文本可以避免把 64 位整数未经检查地转成 `f64`，输出也更贴近手写命令。
    pub(super) fn signed_number_text(&mut self, name: &str) -> Result<String, Diagnostic> {
        let minus = self.take(&TokenKind::Minus);
        let token = self.advance().clone();
        let text = match token.kind {
            TokenKind::Number(value) => value.to_string(),
            TokenKind::Decimal(value) => format!("{value}"),
            _ => return Err(Diagnostic::new(format!("{name} 需要数字"), token.span)),
        };
        Ok(if minus.is_some() {
            format!("-{text}")
        } else {
            text
        })
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
