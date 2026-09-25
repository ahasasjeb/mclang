use crate::ast::*;

use crate::compiler::codegen::Compiler;
use crate::compiler::constant::constant_value;

impl Compiler<'_> {
    /// Keep a helper boundary for multi-command blocks and commands whose
    /// return would otherwise escape the enclosing function.
    pub(super) fn compile_small_block(&mut self, body: &[Statement], owner: &str) -> String {
        let mut compiled = self.compile_block(body, owner);
        let inline = body.len() == 1
            && !matches!(
                body[0].kind,
                StatementKind::Return(_) | StatementKind::Run(_)
            )
            && compiled.len() == 1
            && !compiled[0].starts_with("return ")
            && !compiled[0].contains(" run return ");
        if inline {
            compiled.pop().expect("one command")
        } else {
            let path = self.next_helper_path(owner);
            self.functions.insert(path.clone(), compiled);
            format!("function {}:{path}", self.program.namespace)
        }
    }

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
    constant_value(expression)
}

/// 编译期能确定真假的布尔条件；三值逻辑，`None` 表示要在运行期求值。
pub(in crate::compiler::codegen) fn constant_condition(condition: &Condition) -> Option<bool> {
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
        // Only a constant *left* operand may skip the right operand. A constant
        // right operand cannot erase the evaluation of a dynamic left operand.
        Condition::And(left, right) => match constant_condition(left) {
            Some(false) => Some(false),
            Some(true) => constant_condition(right),
            None => None,
        },
        Condition::Or(left, right) => match constant_condition(left) {
            Some(true) => Some(true),
            Some(false) => constant_condition(right),
            None => None,
        },
        _ => None,
    }
}

/// Whether a jump can target the current loop. Inner loops consume their own
/// jumps and must not force a state check in the outer loop.
pub(super) fn contains_current_loop_jump(statement: &Statement) -> bool {
    match &statement.kind {
        StatementKind::Break | StatementKind::Continue => true,
        StatementKind::If {
            then_body,
            else_body,
            ..
        } => {
            then_body.iter().any(contains_current_loop_jump)
                || else_body.iter().any(contains_current_loop_jump)
        }
        StatementKind::Execute { body, .. }
        | StatementKind::Each { body, .. }
        | StatementKind::InDimension { body, .. }
        | StatementKind::Spawn { body, .. } => body.iter().any(contains_current_loop_jump),
        _ => false,
    }
}
