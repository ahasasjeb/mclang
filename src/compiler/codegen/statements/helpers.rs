use crate::ast::*;

use crate::compiler::codegen::Compiler;

impl Compiler<'_> {
    /// 把语句块放入新辅助函数，返回该函数的路径。
    pub(super) fn compile_helper(&mut self, body: &[Statement], owner: &str) -> String {
        let path = self.next_helper_path(owner);
        let commands = self.compile_block(body, owner);
        self.functions.insert(path.clone(), commands);
        path
    }

    pub(in crate::compiler::codegen) fn next_helper_path(&mut self, owner: &str) -> String {
        let counter = self.helper_counters.entry(owner.to_owned()).or_default();
        let path = format!("__mcl/{owner}/{}", *counter);
        *counter += 1;
        path
    }
}
/// `for` 循环的上限：常量直接用 `matches ..N` 比较，否则用计分项。
pub(super) enum LoopLimit {
    Constant(i32),
    Holder(String),
}

/// 编译期能确定的整数表达式的值。
pub(super) fn constant_integer(expression: &Expr) -> Option<i32> {
    match &expression.kind {
        ExprKind::Integer(value) => Some(*value),
        ExprKind::Negate(value) => constant_integer(value)?.checked_neg(),
        _ => None,
    }
}

/// 编译期能确定真假的布尔条件；三值逻辑，`None` 表示要在运行期求值。
pub(super) fn constant_condition(condition: &Condition) -> Option<bool> {
    match condition {
        Condition::Compare {
            left,
            comparison,
            right,
        } => {
            let left = constant_integer(left)?;
            let right = constant_integer(right)?;
            Some(match comparison {
                Comparison::Equal => left == right,
                Comparison::NotEqual => left != right,
                Comparison::Less => left < right,
                Comparison::LessEqual => left <= right,
                Comparison::Greater => left > right,
                Comparison::GreaterEqual => left >= right,
            })
        }
        Condition::Not(inner) => Some(!constant_condition(inner)?),
        Condition::And(left, right) => {
            match (constant_condition(left), constant_condition(right)) {
                (Some(false), _) | (_, Some(false)) => Some(false),
                (Some(true), Some(true)) => Some(true),
                _ => None,
            }
        }
        Condition::Or(left, right) => match (constant_condition(left), constant_condition(right)) {
            (Some(true), _) | (_, Some(true)) => Some(true),
            (Some(false), Some(false)) => Some(false),
            _ => None,
        },
        _ => None,
    }
}

/// 语句（含嵌套块）里是否可能出现 `break`/`continue`。
///
/// 保守判断：`if`/`execute`/内层循环里出现跳转时也返回真。多插入的状态检查
/// 在状态为 0 时是空操作，因此不会改变行为，只多一条命令。
pub(super) fn contains_flow_jump(statement: &Statement) -> bool {
    match &statement.kind {
        StatementKind::Break | StatementKind::Continue => true,
        StatementKind::If {
            then_body,
            else_body,
            ..
        } => then_body.iter().any(contains_flow_jump) || else_body.iter().any(contains_flow_jump),
        StatementKind::Execute { body, .. }
        | StatementKind::While { body, .. }
        | StatementKind::For { body, .. } => body.iter().any(contains_flow_jump),
        _ => false,
    }
}
