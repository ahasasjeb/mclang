use crate::ast::{BinaryOp, Expr, ExprKind};

/// 表达式不能折叠成编译期常量的原因；用于生成带原因的诊断。
pub(crate) enum ConstantBlocker {
    /// 引用了运行期计分变量。
    Name(String),
    /// 包含函数调用、查询或命令结果。
    Runtime,
    /// 只有常量，但运算除以零或超出 32 位整数范围。
    Overflow,
}

impl ConstantBlocker {
    pub(crate) fn reason(&self) -> String {
        match self {
            ConstantBlocker::Name(name) => format!("`{name}` 不是编译期常量"),
            ConstantBlocker::Runtime => "表达式包含运行期求值：函数调用、查询或命令结果".to_owned(),
            ConstantBlocker::Overflow => "常量运算除以零或超出 32 位整数范围".to_owned(),
        }
    }
}

/// 折叠编译期常量；失败时给出原因。调用方负责组织诊断文案与位置。
pub(crate) fn compile_time_constant(expression: &Expr) -> Result<i32, ConstantBlocker> {
    constant_value(expression).ok_or_else(|| constant_blocker(expression))
}

/// 找出表达式不能折叠的原因，按源码顺序取第一个。
fn constant_blocker(expression: &Expr) -> ConstantBlocker {
    fn scan(expression: &Expr, runtime: &mut bool) -> Option<String> {
        match &expression.kind {
            ExprKind::Integer(_) => None,
            ExprKind::Score(name) => Some(name.clone()),
            ExprKind::Negate(value) => scan(value, runtime),
            ExprKind::Binary { left, right, .. } => {
                scan(left, runtime).or_else(|| scan(right, runtime))
            }
            _ => {
                *runtime = true;
                None
            }
        }
    }
    let mut runtime = false;
    match scan(expression, &mut runtime) {
        Some(name) => ConstantBlocker::Name(name),
        None if runtime => ConstantBlocker::Runtime,
        None => ConstantBlocker::Overflow,
    }
}

/// 在编译期折叠纯常量表达式；表达式含变量或函数调用时返回 `None`。
///
/// 解析、语义检查和代码生成共用同一份折叠规则，保证“语法层能算出的常量”、
/// “检查能算出的常量”与“生成时能折叠的常量”含义一致。
pub(crate) fn constant_value(expression: &Expr) -> Option<i32> {
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
pub(crate) fn constant_binary(left: i32, operation: BinaryOp, right: i32) -> Option<i32> {
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
