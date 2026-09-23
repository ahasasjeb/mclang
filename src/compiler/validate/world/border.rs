use crate::ast::{Coordinate, Span, WorldBorderOperation};
use crate::diagnostic::Diagnostic;

use super::positions::validate_vec2;
use super::{BORDER_MAX_CENTER, BORDER_MAX_SIZE};

pub(super) fn validate_world_border(
    operation: &WorldBorderOperation,
    span: Span,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match operation {
        WorldBorderOperation::Add { distance, .. } => {
            if !distance
                .parse::<f64>()
                .is_ok_and(|value| value.abs() <= BORDER_MAX_SIZE)
            {
                diagnostics.push(Diagnostic::new(
                    format!("worldborder.add 的距离绝对值不能超过 {BORDER_MAX_SIZE}"),
                    span,
                ));
            }
        }
        WorldBorderOperation::Set { distance, .. } => {
            if !distance
                .parse::<f64>()
                .is_ok_and(|value| (1.0..=BORDER_MAX_SIZE).contains(&value))
            {
                diagnostics.push(Diagnostic::new(
                    format!("worldborder.set 的边长必须在 1 到 {BORDER_MAX_SIZE} 之间"),
                    span,
                ));
            }
        }
        WorldBorderOperation::Center(value) => {
            validate_vec2(value, diagnostics);
            for component in [&value.x, &value.z] {
                let Coordinate::Absolute(text) = component else {
                    continue;
                };
                if !text
                    .parse::<f64>()
                    .is_ok_and(|value| value.abs() <= BORDER_MAX_CENTER)
                {
                    diagnostics.push(Diagnostic::new(
                        format!("worldborder.center 的坐标绝对值不能超过 {BORDER_MAX_CENTER}"),
                        span,
                    ));
                }
            }
        }
        WorldBorderOperation::DamageAmount(value) | WorldBorderOperation::DamageBuffer(value) => {
            if !value
                .parse::<f64>()
                .is_ok_and(|value| (0.0..=f32::MAX as f64).contains(&value))
            {
                diagnostics.push(Diagnostic::new(
                    "worldborder 伤害参数必须是 0 到 f32::MAX 的有限数字",
                    span,
                ));
            }
        }
        WorldBorderOperation::WarningDistance(value) => {
            if *value > i32::MAX as u32 {
                diagnostics.push(Diagnostic::new(
                    "worldborder 警告距离不能超过 2147483647",
                    span,
                ));
            }
        }
        WorldBorderOperation::WarningTime(_) => {}
    }
}
