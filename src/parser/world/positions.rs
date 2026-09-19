use crate::ast::*;
use crate::diagnostic::Diagnostic;
use crate::lexer::{Token, TokenKind};

use super::Parser;

impl Parser {
    /// `pos(x, y, z)` / `block_pos(x, y, z)`：方块坐标，绝对分量是整数，
    /// `~`/`^` 分量可带小数偏移。
    pub(in crate::parser) fn block_position(
        &mut self,
        label: &str,
    ) -> Result<BlockPosition, Diagnostic> {
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
        self.record_macro_coordinate(&x, MacroCoordinateKind::Horizontal);
        self.expect(TokenKind::Comma, "坐标分量后需要 `,`")?;
        let y = self.coordinate(label)?;
        self.record_macro_coordinate(&y, MacroCoordinateKind::Vertical);
        self.expect(TokenKind::Comma, "坐标分量后需要 `,`")?;
        let z = self.coordinate(label)?;
        self.record_macro_coordinate(&z, MacroCoordinateKind::Horizontal);
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
    pub(in crate::parser) fn position_value(
        &mut self,
        label: &str,
    ) -> Result<PositionValue, Diagnostic> {
        if self.check_word("vec3") {
            return Ok(PositionValue::Exact(self.vec3_value(label)?));
        }
        Ok(PositionValue::Block(self.block_position(label)?))
    }

    /// `vec3(x, y, z)`：精确坐标，绝对分量允许小数。
    pub(in crate::parser) fn vec3_value(&mut self, label: &str) -> Result<Vec3Value, Diagnostic> {
        let Some(start) = self.take_word("vec3") else {
            return Err(Diagnostic::new(
                format!("{label}需要 `vec3(x, y, z)`"),
                self.current().span,
            ));
        };
        self.expect(TokenKind::LeftParen, "vec3 后需要 `(`")?;
        let x = self.fractional_coordinate(label)?;
        self.record_macro_coordinate(&x, MacroCoordinateKind::Horizontal);
        self.expect(TokenKind::Comma, "vec3 分量后需要 `,`")?;
        let y = self.fractional_coordinate(label)?;
        self.record_macro_coordinate(&y, MacroCoordinateKind::Vertical);
        self.expect(TokenKind::Comma, "vec3 分量后需要 `,`")?;
        let z = self.fractional_coordinate(label)?;
        self.record_macro_coordinate(&z, MacroCoordinateKind::Horizontal);
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
    pub(in crate::parser) fn vec2_value(&mut self, label: &str) -> Result<Vec2Value, Diagnostic> {
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
            self.record_macro_coordinate(coordinate, MacroCoordinateKind::Horizontal);
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
    pub(in crate::parser) fn rotation_value(
        &mut self,
        label: &str,
    ) -> Result<RotationValue, Diagnostic> {
        let Some(start) = self.take_word("rotation") else {
            return Err(Diagnostic::new(
                format!("{label}需要 `rotation(yaw, pitch)`（中文 `朝向(...)`）"),
                self.current().span,
            ));
        };
        self.expect(TokenKind::LeftParen, "rotation 后需要 `(`")?;
        let yaw = self.fractional_coordinate(label)?;
        self.record_macro_coordinate(&yaw, MacroCoordinateKind::Angle);
        self.expect(TokenKind::Comma, "rotation 分量后需要 `,`")?;
        let pitch = self.fractional_coordinate(label)?;
        self.record_macro_coordinate(&pitch, MacroCoordinateKind::Angle);
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
        if let Some(value) = self.macro_coordinate(false)? { return Ok(value); }
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
    pub(in crate::parser) fn column_position(
        &mut self,
        label: &str,
    ) -> Result<ColumnPosition, Diagnostic> {
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
            self.record_macro_coordinate(coordinate, MacroCoordinateKind::Horizontal);
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

    pub(super) fn optional_column_position(
        &mut self,
        label: &str,
    ) -> Result<Option<ColumnPosition>, Diagnostic> {
        if self.take(&TokenKind::Comma).is_none() {
            return Ok(None);
        }
        Ok(Some(self.column_position(label)?))
    }

    pub(super) fn optional_position(&mut self) -> Result<Option<BlockPosition>, Diagnostic> {
        if self.take(&TokenKind::Comma).is_none() {
            return Ok(None);
        }
        Ok(Some(self.block_position("位置参数")?))
    }

    /// 一个坐标分量：绝对整数、`~[±数]` 或 `^[±数]`。
    fn coordinate(&mut self, label: &str) -> Result<Coordinate, Diagnostic> {
        if let Some(value) = self.macro_coordinate(true)? { return Ok(value); }
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
    pub(super) fn vec2_component(&mut self, label: &str) -> Result<String, Diagnostic> {
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
}
