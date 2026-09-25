use crate::ast::{BinaryOp, Expr, ExprKind};

/// 在编译期折叠纯常量表达式；表达式含变量或函数调用时返回 `None`。
///
/// 语义检查和代码生成共用同一份折叠规则，保证“检查能算出的常量”与
/// “生成时能折叠的常量”含义一致。
pub(super) fn constant_value(expression: &Expr) -> Option<i32> {
    match &expression.kind {
        ExprKind::Integer(value) => Some(*value),
        ExprKind::Negate(value) => constant_value(value)?.checked_neg(),
        ExprKind::Binary {
            left,
            operation,
            right,
        } => {
            let left = constant_value(left)?;
            let right = constant_value(right)?;
            constant_binary(left, *operation, right)
        }
        ExprKind::CoreCommand(_)
        | ExprKind::EntityCommand(_)
        | ExprKind::Score(_)
        | ExprKind::Call { .. }
        | ExprKind::ScoreQuery { .. }
        | ExprKind::XpQuery { .. }
        | ExprKind::StopwatchQuery { .. }
        | ExprKind::TimeQuery { .. }
        | ExprKind::GameTimeQuery
        | ExprKind::GameRuleQuery { .. }
        | ExprKind::WorldBorderSize
        | ExprKind::BossBarGet { .. }
        | ExprKind::Count { .. }
        | ExprKind::Random { .. }
        | ExprKind::DataGet { .. }
        | ExprKind::Compute { .. } => None,
    }
}

/// Match scoreboard's floorDiv/floorMod, including a negative divisor, while
/// keeping the language's diagnostics for statically known overflow.
pub(super) fn constant_binary(left: i32, operation: BinaryOp, right: i32) -> Option<i32> {
    match operation {
        BinaryOp::Add => left.checked_add(right),
        BinaryOp::Subtract => left.checked_sub(right),
        BinaryOp::Multiply => left.checked_mul(right),
        BinaryOp::Divide | BinaryOp::Modulo => {
            let left = i64::from(left);
            let right = i64::from(right);
            let mut quotient = left.checked_div(right)?;
            let remainder = left % right;
            if remainder != 0 && (remainder < 0) != (right < 0) {
                quotient -= 1;
            }
            let result = if operation == BinaryOp::Divide {
                quotient
            } else {
                left - quotient * right
            };
            i32::try_from(result).ok()
        }
    }
}
