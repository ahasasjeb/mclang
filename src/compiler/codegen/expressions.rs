//! 表达式与条件下降：产生计分板操作，并管理表达式临时值。

use crate::ast::{BinaryOp, Comparison, Condition, Expr, ExprKind};

use super::Compiler;
use super::Value;
use super::emit::entity_query_clause;
use crate::compiler::constant::constant_value;

impl Compiler<'_> {
    /// 下降表达式：常量直接折叠，变量解析为假玩家槽位，其余运算生成命令。
    pub(super) fn compile_expr(
        &mut self,
        expression: &Expr,
        owner: &str,
        commands: &mut Vec<String>,
    ) -> Value {
        if let Some(value) = constant_value(expression) {
            return Value::Integer(value);
        }
        match &expression.kind {
            ExprKind::Integer(value) => Value::Integer(*value),
            ExprKind::Score(name) => Value::Score(self.variable_holder(owner, name)),
            ExprKind::Call {
                function,
                arguments,
            } => {
                self.bind_arguments(function, arguments, owner, commands);
                let target = self.temporary();
                commands.push(format!(
                    "execute store result score {target} {} run function {}:{function}",
                    self.objective, self.program.namespace
                ));
                Value::Score(target)
            }
            ExprKind::XpQuery { target, kind } => {
                let query = self
                    .program
                    .queries
                    .iter()
                    .find(|candidate| candidate.name == *target)
                    .expect("semantic validation guarantees the entity query exists");
                let result = self.temporary();
                commands.push(format!(
                    "scoreboard players set {result} {} 0",
                    self.objective
                ));
                commands.push(format!(
                    "execute {} store result score {result} {} run xp query @s {}",
                    entity_query_clause(query),
                    self.objective,
                    kind.as_str()
                ));
                Value::Score(result)
            }
            ExprKind::StopwatchQuery { id, scale } => {
                let result = self.temporary();
                let scale = scale
                    .as_ref()
                    .map(|scale| format!(" {scale}"))
                    .unwrap_or_default();
                commands.push(format!(
                    "execute store result score {result} {} run stopwatch query {id}{scale}",
                    self.objective
                ));
                Value::Score(result)
            }
            ExprKind::TimeQuery { clock } => {
                let command = match clock {
                    Some(clock) => format!("time of {clock} query time"),
                    None => "time query time".to_owned(),
                };
                self.capture_result(command, commands)
            }
            ExprKind::GameTimeQuery => {
                self.capture_result("time query gametime".to_owned(), commands)
            }
            ExprKind::GameRuleQuery { name } => {
                self.capture_result(format!("gamerule {name}"), commands)
            }
            ExprKind::WorldBorderSize => {
                self.capture_result("worldborder get".to_owned(), commands)
            }
            ExprKind::Negate(value) => {
                let source_value = self.compile_expr(value, owner, commands);
                let source = self.materialize(source_value, commands);
                let target = self.temporary();
                commands.push(format!(
                    "scoreboard players set {target} {} 0",
                    self.objective
                ));
                commands.push(format!(
                    "scoreboard players operation {target} {} -= {source} {}",
                    self.objective, self.objective
                ));
                Value::Score(target)
            }
            ExprKind::Binary {
                left,
                operation,
                right,
            } => {
                let left_value = self.compile_expr(left, owner, commands);
                let right_value = self.compile_expr(right, owner, commands);
                let target = self.temporary();
                match left_value {
                    Value::Integer(value) => commands.push(format!(
                        "scoreboard players set {target} {} {value}",
                        self.objective
                    )),
                    Value::Score(source) => commands.push(format!(
                        "scoreboard players operation {target} {} = {source} {}",
                        self.objective, self.objective
                    )),
                }
                if matches!(operation, BinaryOp::Add | BinaryOp::Subtract)
                    && let Value::Integer(value) = &right_value
                {
                    let signed = if *operation == BinaryOp::Add {
                        i64::from(*value)
                    } else {
                        -i64::from(*value)
                    };
                    if (0..=i64::from(i32::MAX)).contains(&signed) {
                        commands.push(format!(
                            "scoreboard players add {target} {} {signed}",
                            self.objective
                        ));
                        return Value::Score(target);
                    } else if (-i64::from(i32::MAX)..0).contains(&signed) {
                        commands.push(format!(
                            "scoreboard players remove {target} {} {}",
                            self.objective, -signed
                        ));
                        return Value::Score(target);
                    }
                }
                let source = self.materialize(right_value, commands);
                let symbol = match operation {
                    BinaryOp::Add => "+=",
                    BinaryOp::Subtract => "-=",
                    BinaryOp::Multiply => "*=",
                    BinaryOp::Divide => "/=",
                    BinaryOp::Modulo => "%=",
                };
                commands.push(format!(
                    "scoreboard players operation {target} {} {symbol} {source} {}",
                    self.objective, self.objective
                ));
                Value::Score(target)
            }
        }
    }

    /// 把一条查询命令的结果捕获到新的临时计分项。
    ///
    /// 命令失败时原版 `execute store result` 写入 0，因此不需要额外预置。
    fn capture_result(&mut self, command: String, commands: &mut Vec<String>) -> Value {
        let result = self.temporary();
        commands.push(format!(
            "execute store result score {result} {} run {command}",
            self.objective
        ));
        Value::Score(result)
    }

    /// 把值落到具体计分项：常量需要额外生成一条 set 命令。
    pub(super) fn materialize(&mut self, value: Value, commands: &mut Vec<String>) -> String {
        match value {
            Value::Score(score) => score,
            Value::Integer(value) => {
                let score = self.temporary();
                commands.push(format!(
                    "scoreboard players set {score} {} {value}",
                    self.objective
                ));
                score
            }
        }
    }

    /// 把布尔条件求值为 0/1 的标志计分项，并返回它的名字。
    ///
    /// 调用方负责用 `execute if score <flag> matches 1` 分派分支。
    pub(super) fn compile_condition(
        &mut self,
        condition: &Condition,
        owner: &str,
        commands: &mut Vec<String>,
    ) -> String {
        match condition {
            Condition::Predicate { name, .. } => {
                let flag = self.temporary();
                commands.push(format!(
                    "scoreboard players set {flag} {} 0",
                    self.objective
                ));
                commands.push(format!(
                    "execute if predicate {}:{name} run scoreboard players set {flag} {} 1",
                    self.program.namespace, self.objective
                ));
                flag
            }
            Condition::Compare {
                left,
                comparison,
                right,
            } => {
                let left_value = self.compile_expr(left, owner, commands);
                let right_value = self.compile_expr(right, owner, commands);
                let left = self.materialize(left_value, commands);
                let right = self.materialize(right_value, commands);
                let flag = self.temporary();
                commands.push(format!(
                    "scoreboard players set {flag} {} 0",
                    self.objective
                ));
                let (prefix, symbol) = match comparison {
                    Comparison::Equal => ("if", "="),
                    Comparison::NotEqual => ("unless", "="),
                    Comparison::Less => ("if", "<"),
                    Comparison::LessEqual => ("if", "<="),
                    Comparison::Greater => ("if", ">"),
                    Comparison::GreaterEqual => ("if", ">="),
                };
                commands.push(format!(
                    "execute {prefix} score {left} {} {symbol} {right} {} run scoreboard players set {flag} {} 1",
                    self.objective, self.objective, self.objective
                ));
                flag
            }
            Condition::Not(condition) => {
                let inner = self.compile_condition(condition, owner, commands);
                let flag = self.temporary();
                commands.push(format!(
                    "scoreboard players set {flag} {} 1",
                    self.objective
                ));
                commands.push(format!(
                    "execute if score {inner} {} matches 1 run scoreboard players set {flag} {} 0",
                    self.objective, self.objective
                ));
                flag
            }
            Condition::And(left, right) => {
                let left = self.compile_condition(left, owner, commands);
                let right = self.compile_condition(right, owner, commands);
                let flag = self.temporary();
                commands.push(format!(
                    "scoreboard players set {flag} {} 0",
                    self.objective
                ));
                commands.push(format!(
                    "execute if score {left} {} matches 1 if score {right} {} matches 1 run scoreboard players set {flag} {} 1",
                    self.objective, self.objective, self.objective
                ));
                flag
            }
            Condition::Or(left, right) => {
                let left = self.compile_condition(left, owner, commands);
                let right = self.compile_condition(right, owner, commands);
                let flag = self.temporary();
                commands.push(format!(
                    "scoreboard players set {flag} {} 0",
                    self.objective
                ));
                commands.push(format!(
                    "execute if score {left} {} matches 1 run scoreboard players set {flag} {} 1",
                    self.objective, self.objective
                ));
                commands.push(format!(
                    "execute if score {right} {} matches 1 run scoreboard players set {flag} {} 1",
                    self.objective, self.objective
                ));
                flag
            }
        }
    }
}
