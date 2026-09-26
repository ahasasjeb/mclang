//! Arithmetic lowering, command results, and temporary score slots.

use crate::ast::{
    BinaryOp, ComputeKind, ComputeSource, Expr, ExprKind, Holder, ItemConditionSource,
    NbtComponentSource,
};

use super::Compiler;
use super::Value;
use super::emit::{entity_query_clause, entity_query_selector};
use super::names::user_objective_name;
use crate::compiler::constant::constant_value;

impl Compiler<'_> {
    /// Write a result into its final slot when the expression can be lowered
    /// there without changing when its operands are observed.
    pub(super) fn compile_expr_into(
        &mut self,
        expression: &Expr,
        target: &str,
        owner: &str,
        commands: &mut Vec<String>,
    ) {
        if let Some(value) = constant_value(expression) {
            self.store_value(target, Value::Integer(value), commands);
            return;
        }
        match &expression.kind {
            ExprKind::Score(name) => {
                let source = self.variable_holder(owner, name);
                if source != target {
                    self.store_value(target, Value::Score(source), commands);
                }
            }
            ExprKind::Call {
                function,
                arguments,
            } => {
                self.bind_arguments(function, arguments, owner, commands);
                commands.push(format!(
                    "execute store result score {target} {} run function {}:{function}",
                    self.objective, self.program.namespace
                ));
            }
            ExprKind::Binary {
                left,
                operation,
                right,
            } if let Some(literal) = constant_value(right) => {
                self.compile_expr_into(left, target, owner, commands);
                self.apply_literal_operation(target, *operation, literal, commands);
            }
            _ => {
                let value = self.compile_expr(expression, owner, commands);
                self.store_value(target, value, commands);
            }
        }
    }

    fn apply_literal_operation(
        &mut self,
        target: &str,
        operation: BinaryOp,
        literal: i32,
        commands: &mut Vec<String>,
    ) {
        if matches!(operation, BinaryOp::Add | BinaryOp::Subtract) {
            let signed = if operation == BinaryOp::Add {
                i64::from(literal)
            } else {
                -i64::from(literal)
            };
            if signed == 0 {
                return;
            }
            if (0..=i64::from(i32::MAX)).contains(&signed) {
                commands.push(format!(
                    "scoreboard players add {target} {} {signed}",
                    self.objective
                ));
                return;
            }
            if (-i64::from(i32::MAX)..0).contains(&signed) {
                commands.push(format!(
                    "scoreboard players remove {target} {} {}",
                    self.objective, -signed
                ));
                return;
            }
        }
        if literal == 1 && matches!(operation, BinaryOp::Multiply | BinaryOp::Divide) {
            return;
        }
        let source = self.materialize(Value::Integer(literal), commands);
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
    }

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
            ExprKind::CoreCommand(command) => {
                let native = self.core_command(command, owner);
                let result = self.temporary();
                commands.push(format!(
                    "scoreboard players set {result} {} 0",
                    self.objective
                ));
                commands.push(format!(
                    "execute store result score {result} {} run {}",
                    self.objective, native
                ));
                Value::Score(result)
            }
            ExprKind::EntityCommand(command) => {
                let native = self.entity_command(command, owner);
                let result = self.temporary();
                commands.push(format!(
                    "scoreboard players set {result} {} 0",
                    self.objective
                ));
                commands.push(format!(
                    "execute store result score {result} {} run {}",
                    self.objective, native
                ));
                Value::Score(result)
            }
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
                let query = self.query(target);
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
            ExprKind::ScoreQuery { target } => {
                let objective = user_objective_name(&self.program.namespace, &target.objective);
                let result = self.temporary();
                // 持有者不存在（没有投掷者或查询无匹配）时原版不会触发 store 回调，
                // 先置 0 保证读到的是「未赋值」而不是上一次调用留下的值。
                commands.push(format!(
                    "scoreboard players set {result} {} 0",
                    self.objective
                ));
                let prefix = match &target.holder {
                    Holder::SelfEntity => "execute ".to_owned(),
                    Holder::Origin => "execute on origin ".to_owned(),
                    Holder::Query(name, _) => {
                        let query = self.query(name);
                        format!("execute {} ", entity_query_clause(query))
                    }
                };
                commands.push(format!(
                    "{prefix}store result score {result} {} run scoreboard players get @s {objective}",
                    self.objective
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
            ExprKind::BossBarGet { id, property } => self.capture_result(
                format!("bossbar get {id} {}", property.command_name()),
                commands,
            ),
            ExprKind::Count { query, .. } => {
                let target = self.temporary();
                let selector = entity_query_selector(self.query(query));
                commands.push(format!(
                    "scoreboard players set {target} {} 0",
                    self.objective
                ));
                commands.push(format!(
                    "execute store result score {target} {} if entity {selector}",
                    self.objective
                ));
                Value::Score(target)
            }
            ExprKind::Random { min, max } => {
                let target = self.temporary();
                commands.push(format!(
                    "scoreboard players set {target} {} 0",
                    self.objective
                ));
                commands.push(format!(
                    "execute store result score {target} {} run random value {min}..{max}",
                    self.objective
                ));
                Value::Score(target)
            }
            ExprKind::DataGet {
                source,
                path,
                scale,
                ..
            } => {
                let holders = match source {
                    NbtComponentSource::Entity(holder) => vec![holder],
                    NbtComponentSource::Block(_) | NbtComponentSource::Storage(_, _) => Vec::new(),
                };
                let native = self.capture_command_targets(&holders, owner, |compiler| {
                    let scale = scale
                        .as_ref()
                        .map(|scale| format!(" {scale}"))
                        .unwrap_or_default();
                    format!(
                        "data get {} {path}{scale}",
                        compiler.nbt_source_text(source)
                    )
                });
                let target = self.temporary();
                commands.push(format!(
                    "scoreboard players set {target} {} 0",
                    self.objective
                ));
                commands.push(format!(
                    "execute store result score {target} {} run {native}",
                    self.objective,
                ));
                Value::Score(target)
            }
            ExprKind::Compute {
                source,
                kind,
                provider,
                scale,
                ..
            } => {
                let kind = match kind {
                    ComputeKind::Float => "float",
                    ComputeKind::Integer => "integer",
                };
                let scale = scale
                    .as_ref()
                    .map(|scale| format!(" {scale}"))
                    .unwrap_or_default();
                self.capture_result(
                    format!(
                        "compute {} {kind} {provider}{scale}",
                        self.compute_source_text(source)
                    ),
                    commands,
                )
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
                if let Some(literal) = constant_value(right)
                    && (matches!(operation, BinaryOp::Add | BinaryOp::Subtract) && literal == 0
                        || matches!(operation, BinaryOp::Multiply | BinaryOp::Divide)
                            && literal == 1)
                {
                    return self.compile_expr(left, owner, commands);
                }
                if *operation == BinaryOp::Add && constant_value(left) == Some(0) {
                    return self.compile_expr(right, owner, commands);
                }
                let left_value = self.compile_expr(left, owner, commands);
                let left_value = self.freeze_before_effect(left_value, right, commands);
                let right_value = self.compile_expr(right, owner, commands);
                // Compiler temporaries have unique names within a build. The
                // right operand cannot refer to one from source code.
                let reuse = matches!(&left_value, Value::Score(score) if score.starts_with("#t"));
                let target = if reuse {
                    match &left_value {
                        Value::Score(score) => score.clone(),
                        _ => unreachable!(),
                    }
                } else {
                    self.temporary()
                };
                if !reuse {
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
                }
                if let Value::Integer(literal) = &right_value {
                    self.apply_literal_operation(&target, *operation, *literal, commands);
                    return Value::Score(target);
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

    /// A literal operand uses a reserved slot initialized once by __mcl/load.
    pub(super) fn materialize(&mut self, value: Value, _commands: &mut Vec<String>) -> String {
        match value {
            Value::Score(score) => score,
            Value::Integer(value) => {
                self.constants.insert(value);
                format!("#c_{value}")
            }
        }
    }

    pub(super) fn freeze_before_effect(
        &mut self,
        value: Value,
        next: &Expr,
        commands: &mut Vec<String>,
    ) -> Value {
        match value {
            Value::Score(score)
                if !score.starts_with("#t") && expression_may_modify_state(next) =>
            {
                let frozen = self.temporary();
                commands.push(format!(
                    "scoreboard players operation {frozen} {} = {score} {}",
                    self.objective, self.objective
                ));
                Value::Score(frozen)
            }
            other => other,
        }
    }

    /// NBT 数据来源的命令文本（`data` 条件与表达式共用）。
    pub(super) fn nbt_source_text(&self, source: &NbtComponentSource) -> String {
        match source {
            NbtComponentSource::Entity(holder) => {
                format!("entity {}", self.component_holder(holder))
            }
            NbtComponentSource::Block(position) => {
                format!("block {}", super::world::position_text(position))
            }
            NbtComponentSource::Storage(storage, _) => format!("storage {storage}"),
        }
    }

    /// 物品条件的来源文本（`if items`/`if slots`）。
    pub(super) fn item_condition_source_text(&self, source: &ItemConditionSource) -> String {
        match source {
            ItemConditionSource::Entity(holder) => {
                format!("entity {}", self.component_holder(holder))
            }
            ItemConditionSource::Block(position) => {
                format!("block {}", super::world::position_text(position))
            }
        }
    }

    /// `compute` 上下文来源的命令文本（表达式与 `data.modify` 共用）。
    pub(super) fn compute_source_text(&self, source: &ComputeSource) -> String {
        match source {
            ComputeSource::Default => "default".to_owned(),
            ComputeSource::Block(position) => {
                format!("block {}", super::world::position_text(position))
            }
            ComputeSource::Entity(holder) => {
                format!("entity {}", self.component_holder(holder))
            }
        }
    }
}

/// A later operand can change an earlier score through a function or native
/// command. Freeze that earlier value before compiling the later operand.
pub(super) fn expression_may_modify_state(expression: &Expr) -> bool {
    match &expression.kind {
        ExprKind::Integer(_) | ExprKind::Score(_) => false,
        ExprKind::Negate(inner) => expression_may_modify_state(inner),
        ExprKind::Binary { left, right, .. } => {
            expression_may_modify_state(left) || expression_may_modify_state(right)
        }
        _ => true,
    }
}
