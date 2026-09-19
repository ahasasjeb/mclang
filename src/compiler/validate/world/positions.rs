use crate::ast::{
    BlockPosition, ColumnPosition, Coordinate, PositionValue, Span, Vec2Value, Vec3Value,
};
use crate::diagnostic::Diagnostic;

use super::{HORIZONTAL_MAX, HORIZONTAL_MIN, VERTICAL_MAX, VERTICAL_MIN};

/// 校验方块坐标的绝对分量；相对坐标与局部坐标留给运行时。
pub(crate) fn validate_block_position(position: &BlockPosition, diagnostics: &mut Vec<Diagnostic>) {
    for (axis, coordinate) in [("X", &position.x), ("Y", &position.y), ("Z", &position.z)] {
        let Some(value) = coordinate.absolute_integer() else {
            continue;
        };
        let valid = match axis {
            "Y" => (VERTICAL_MIN..=VERTICAL_MAX).contains(&value),
            _ => (HORIZONTAL_MIN..=HORIZONTAL_MAX).contains(&value),
        };
        if !valid {
            let (low, high) = if axis == "Y" {
                (VERTICAL_MIN, VERTICAL_MAX)
            } else {
                (HORIZONTAL_MIN, HORIZONTAL_MAX)
            };
            diagnostics.push(Diagnostic::new(
                format!("方块 {axis} 坐标 {value} 超出世界范围（{low} 到 {high}）"),
                position.span,
            ));
        }
    }
}

/// 校验精确坐标（`vec3`）的绝对分量；小数允许，范围与方块坐标一致。
pub(super) fn validate_vec3(position: &Vec3Value, diagnostics: &mut Vec<Diagnostic>) {
    validate_fractional_axis(
        "X",
        &position.x,
        HORIZONTAL_MIN as f64,
        HORIZONTAL_MAX as f64,
        position.span,
        diagnostics,
    );
    validate_fractional_axis(
        "Y",
        &position.y,
        VERTICAL_MIN as f64,
        VERTICAL_MAX as f64,
        position.span,
        diagnostics,
    );
    validate_fractional_axis(
        "Z",
        &position.z,
        HORIZONTAL_MIN as f64,
        HORIZONTAL_MAX as f64,
        position.span,
        diagnostics,
    );
}

/// 校验水平精确坐标（`vec2`）的绝对分量。
pub(in crate::compiler::validate) fn validate_vec2(
    position: &Vec2Value,
    diagnostics: &mut Vec<Diagnostic>,
) {
    validate_fractional_axis(
        "X",
        &position.x,
        HORIZONTAL_MIN as f64,
        HORIZONTAL_MAX as f64,
        position.span,
        diagnostics,
    );
    validate_fractional_axis(
        "Z",
        &position.z,
        HORIZONTAL_MIN as f64,
        HORIZONTAL_MAX as f64,
        position.span,
        diagnostics,
    );
}

/// 任意位置值：方块坐标或精确坐标。
pub(crate) fn validate_position_value(position: &PositionValue, diagnostics: &mut Vec<Diagnostic>) {
    match position {
        PositionValue::Block(position) => validate_block_position(position, diagnostics),
        PositionValue::Exact(position) => validate_vec3(position, diagnostics),
    }
}

/// 精确坐标的绝对分量范围；原版在运行期规范化朝向，编译器不限制取值。
fn validate_fractional_axis(
    axis: &str,
    coordinate: &Coordinate,
    min: f64,
    max: f64,
    span: Span,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Coordinate::Absolute(text) = coordinate else {
        return;
    };
    let Ok(value) = text.parse::<f64>() else {
        return;
    };
    if !(min..=max).contains(&value) {
        diagnostics.push(Diagnostic::new(
            format!("{axis} 坐标 {text} 超出世界范围（{min} 到 {max}）"),
            span,
        ));
    }
}

pub(super) fn validate_column_position(
    position: &ColumnPosition,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for (axis, coordinate) in [("X", &position.x), ("Z", &position.z)] {
        let Some(value) = coordinate.absolute_integer() else {
            continue;
        };
        if !(HORIZONTAL_MIN..=HORIZONTAL_MAX).contains(&value) {
            diagnostics.push(Diagnostic::new(
                format!(
                    "列 {axis} 坐标 {value} 超出世界范围（{HORIZONTAL_MIN} 到 {HORIZONTAL_MAX}）"
                ),
                position.span,
            ));
        }
    }
}
