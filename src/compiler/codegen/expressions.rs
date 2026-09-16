//! 表达式与条件下降：产生计分板操作，并管理表达式临时值。

use crate::ast::{
    BinaryOp, CallTarget, Comparison, ComputeKind, ComputeSource, Condition, Expr, ExprKind,
    Holder, ItemConditionSource, NbtComponentSource,
};

use super::Compiler;
use super::Value;
use super::emit::{entity_query_clause, entity_query_selector};
use super::names::user_objective_name;
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
                        let query = self
                            .program
                            .queries
                            .iter()
                            .find(|candidate| candidate.name == *name)
                            .expect("semantic validation guarantees the entity query exists");
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
            ExprKind::DataGet { source, path, .. } => {
                let targets = match source {
                    NbtComponentSource::Entity(holder) => {
                        format!("entity {}", self.component_holder(holder))
                    }
                    NbtComponentSource::Block(position) => {
                        format!("block {}", super::world::position_text(position))
                    }
                    NbtComponentSource::Storage(storage, _) => format!("storage {storage}"),
                };
                let target = self.temporary();
                commands.push(format!(
                    "scoreboard players set {target} {} 0",
                    self.objective
                ));
                commands.push(format!(
                    "execute store result score {target} {} run data get {targets} {path}",
                    self.objective
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
                let source = match source {
                    ComputeSource::Default => "default".to_owned(),
                    ComputeSource::Block(position) => {
                        format!("block {}", super::world::position_text(position))
                    }
                    ComputeSource::Entity(holder) => {
                        format!("entity {}", self.component_holder(holder))
                    }
                };
                let kind = match kind {
                    ComputeKind::Float => "float",
                    ComputeKind::Integer => "integer",
                };
                let scale = scale
                    .as_ref()
                    .map(|scale| format!(" {scale}"))
                    .unwrap_or_default();
                self.capture_result(
                    format!("compute {source} {kind} {provider}{scale}"),
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
            Condition::Predicate { name, .. } => self.compile_atomic_condition(
                format!("predicate {}:{name}", self.program.namespace),
                commands,
            ),
            Condition::Block { pos, block, .. } => self.compile_atomic_condition(
                format!(
                    "block {} {}",
                    super::world::position_text(pos),
                    super::world::block_state_text(block)
                ),
                commands,
            ),
            Condition::Blocks {
                start,
                end,
                destination,
                masked,
                ..
            } => {
                let mode = if *masked { " masked" } else { "" };
                self.compile_atomic_condition(
                    format!(
                        "blocks {} {} {}{mode}",
                        super::world::position_text(start),
                        super::world::position_text(end),
                        super::world::position_text(destination),
                    ),
                    commands,
                )
            }
            Condition::Biome { pos, biome, .. } => self.compile_atomic_condition(
                format!("biome {} {biome}", super::world::position_text(pos)),
                commands,
            ),
            Condition::Loaded { pos, .. } => self.compile_atomic_condition(
                format!("loaded {}", super::world::position_text(pos)),
                commands,
            ),
            Condition::Dimension { dimension, .. } => {
                self.compile_atomic_condition(format!("dimension {dimension}"), commands)
            }
            Condition::Entity { query, .. } => self.compile_atomic_condition(
                format!("entity {}", entity_query_selector(self.query(query))),
                commands,
            ),
            Condition::Data { source, path, .. } => self.compile_atomic_condition(
                format!("data {} {path}", self.nbt_source_text(source)),
                commands,
            ),
            Condition::Items {
                source,
                slots,
                item,
                ..
            } => self.compile_atomic_condition(
                format!(
                    "items {} {slots} {item}",
                    self.item_condition_source_text(source)
                ),
                commands,
            ),
            Condition::Slots { source, slots, .. } => self.compile_atomic_condition(
                format!("slots {} {slots}", self.item_condition_source_text(source)),
                commands,
            ),
            Condition::Function { target, .. } => {
                let target = match target {
                    CallTarget::Function(name) => format!("{}:{name}", self.program.namespace),
                    CallTarget::Tag(tag) => format!("#{}:{tag}", self.program.namespace),
                };
                self.compile_atomic_condition(format!("function {target}"), commands)
            }
            Condition::Stopwatch { id, .. } => {
                self.compile_atomic_condition(format!("stopwatch {id}"), commands)
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

    /// 原子条件：置 0 后用一条 `execute if <谓词>` 冻结为 0/1 标志。
    fn compile_atomic_condition(
        &mut self,
        predicate: String,
        commands: &mut Vec<String>,
    ) -> String {
        let flag = self.temporary();
        commands.push(format!(
            "scoreboard players set {flag} {} 0",
            self.objective
        ));
        commands.push(format!(
            "execute if {predicate} run scoreboard players set {flag} {} 1",
            self.objective
        ));
        flag
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
    fn item_condition_source_text(&self, source: &ItemConditionSource) -> String {
        match source {
            ItemConditionSource::Entity(holder) => {
                format!("entity {}", self.component_holder(holder))
            }
            ItemConditionSource::Block(position) => {
                format!("block {}", super::world::position_text(position))
            }
        }
    }
}
