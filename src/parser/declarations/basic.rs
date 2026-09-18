use crate::ast::*;
use crate::diagnostic::Diagnostic;
use crate::lexer::TokenKind;

use crate::parser::Parser;
use crate::parser::keywords::{
    data_slot_kind, number_format_kind, objective_property, render_type as render_type_value,
};

impl Parser {
    pub(in crate::parser) fn score(&mut self) -> Result<ScoreDecl, Diagnostic> {
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
    pub(in crate::parser) fn objective(&mut self) -> Result<ObjectiveDecl, Diagnostic> {
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
    pub(in crate::parser) fn data_slot(&mut self) -> Result<DataSlotDecl, Diagnostic> {
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
}
