//! Boolean condition lowering and native execute predicates.

use super::emit::entity_query_selector;
use super::statements::helpers::constant_condition;
use super::{Compiler, Value};
use crate::ast::*;
use crate::compiler::constant::constant_value;

impl Compiler<'_> {
    /// A condition that can participate directly in a native execute chain.
    /// No expression is evaluated here; dynamic expressions keep the existing
    /// one-shot flag path, so calls and command results retain their order.
    pub(super) fn direct_condition_clause(
        &self,
        condition: &Condition,
        negated: bool,
        owner: &str,
    ) -> Option<String> {
        // Native predicate errors abort even an `unless` chain. Capture the
        // boolean before negation unless this atom handles absence as false.
        if negated
            && !matches!(
                condition,
                Condition::Not(_)
                    | Condition::Compare { .. }
                    | Condition::Entity { .. }
                    | Condition::Loaded { .. }
            )
        {
            return None;
        }
        let polarity = if negated { "unless" } else { "if" };
        let atom = match condition {
            Condition::Not(inner) => return self.direct_condition_clause(inner, !negated, owner),
            Condition::And(left, right) if !negated => {
                return Some(format!(
                    "{} {}",
                    self.direct_condition_clause(left, false, owner)?,
                    self.direct_condition_clause(right, false, owner)?
                ));
            }
            Condition::Predicate { name, .. } => {
                format!("predicate {}:{name}", self.program.namespace)
            }
            Condition::Block { pos, block, .. } => format!(
                "block {} {}",
                super::world::position_text(pos),
                super::world::block_state_text(block)
            ),
            Condition::Blocks {
                start,
                end,
                destination,
                masked,
                ..
            } => format!(
                "blocks {} {} {}{}",
                super::world::position_text(start),
                super::world::position_text(end),
                super::world::position_text(destination),
                if *masked { " masked" } else { "" }
            ),
            Condition::Biome { pos, biome, .. } => {
                format!("biome {} {biome}", super::world::position_text(pos))
            }
            Condition::Loaded { pos, .. } => {
                format!("loaded {}", super::world::position_text(pos))
            }
            Condition::Dimension { dimension, .. } => format!("dimension {dimension}"),
            Condition::Entity { query, .. } => {
                let query = self.query(query);
                if query.item.is_some() {
                    return None;
                }
                format!("entity {}", entity_query_selector(query))
            }
            Condition::Data { source, path, .. } => {
                if self.condition_source_needs_capture(source) {
                    return None;
                }
                format!("data {} {path}", self.nbt_source_text(source))
            }
            Condition::Items {
                source,
                slots,
                item,
                ..
            } => {
                if self.item_source_needs_capture(source) {
                    return None;
                }
                format!(
                    "items {} {slots} {}",
                    self.item_condition_source_text(source),
                    super::emit::item_predicate_text(item)
                )
            }
            Condition::Slots { source, slots, .. } => {
                if self.item_source_needs_capture(source) {
                    return None;
                }
                format!("slots {} {slots}", self.item_condition_source_text(source))
            }
            Condition::Function { target, .. } => {
                format!("function {}", self.function_target_text(target))
            }
            Condition::Stopwatch { id, .. } => format!("stopwatch {id} 0.."),
            Condition::Compare {
                left,
                comparison,
                right,
            } => {
                let (score, comparison, literal) = match (&left.kind, &right.kind) {
                    (ExprKind::Score(name), _) => (
                        self.variable_holder(owner, name),
                        *comparison,
                        constant_value(right),
                    ),
                    (_, ExprKind::Score(name)) => (
                        self.variable_holder(owner, name),
                        reverse_comparison(*comparison),
                        constant_value(left),
                    ),
                    _ => return None,
                };
                if let Some(literal) = literal {
                    let (test, range) = score_constant_clause(comparison, literal)?;
                    let test = if negated { invert_test(test) } else { test };
                    return Some(format!(
                        "{test} score {score} {} matches {range}",
                        self.objective
                    ));
                }
                if let (ExprKind::Score(left), ExprKind::Score(right)) = (&left.kind, &right.kind) {
                    let (test, symbol) = comparison_operator(comparison);
                    let test = if negated { invert_test(test) } else { test };
                    return Some(format!(
                        "{test} score {} {} {symbol} {} {}",
                        self.variable_holder(owner, left),
                        self.objective,
                        self.variable_holder(owner, right),
                        self.objective
                    ));
                }
                return None;
            }
            Condition::And(_, _) | Condition::Or(_, _) => return None,
        };
        Some(format!("{polarity} {atom}"))
    }

    fn condition_source_needs_capture(&self, source: &NbtComponentSource) -> bool {
        matches!(source, NbtComponentSource::Entity(Holder::Query(name, _)) if self.query(name).item.is_some())
    }

    fn item_source_needs_capture(&self, source: &ItemConditionSource) -> bool {
        matches!(source, ItemConditionSource::Entity(Holder::Query(name, _)) if self.query(name).item.is_some())
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
            Condition::Data { source, path, .. } => {
                let holders = match source {
                    NbtComponentSource::Entity(holder) => vec![holder],
                    NbtComponentSource::Block(_) | NbtComponentSource::Storage(_, _) => Vec::new(),
                };
                let native = self.capture_command_targets(&holders, owner, |compiler| {
                    format!(
                        "execute if data {} {path}",
                        compiler.nbt_source_text(source)
                    )
                });
                self.compile_command_success(native, commands)
            }
            Condition::Items {
                source,
                slots,
                item,
                ..
            } => self.compile_atomic_condition(
                format!(
                    "items {} {slots} {}",
                    self.item_condition_source_text(source),
                    super::emit::item_predicate_text(item)
                ),
                commands,
            ),
            Condition::Slots { source, slots, .. } => self.compile_atomic_condition(
                format!("slots {} {slots}", self.item_condition_source_text(source)),
                commands,
            ),
            Condition::Function { target, .. } => {
                let target = match target {
                    CallTarget::External(id) => id.clone(),
                    CallTarget::Function(name) => format!("{}:{name}", self.program.namespace),
                    CallTarget::Tag(tag) => format!("#{}:{tag}", self.program.namespace),
                };
                self.compile_atomic_condition(format!("function {target}"), commands)
            }
            Condition::Stopwatch { id, .. } => {
                self.compile_atomic_condition(format!("stopwatch {id} 0.."), commands)
            }
            Condition::Compare {
                left,
                comparison,
                right,
            } => {
                let values = self.compile_comparison_operands(left, right, owner, commands);
                let flag = self.temporary();
                self.comparison_flag(&flag, *comparison, values, commands);
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
                if let Some(value) = constant_condition(left) {
                    if value {
                        return self.compile_condition(right, owner, commands);
                    }
                    let flag = self.temporary();
                    commands.push(format!(
                        "scoreboard players set {flag} {} 0",
                        self.objective
                    ));
                    return flag;
                }
                let left = self.compile_condition(left, owner, commands);
                if let Some(value) = constant_condition(right) {
                    if !value {
                        commands.push(format!(
                            "scoreboard players set {left} {} 0",
                            self.objective
                        ));
                    }
                    return left;
                }
                if self.fuse_condition_and(&left, right, owner, commands) {
                    return left;
                }
                let mut right_commands = Vec::new();
                let right = self.compile_condition(right, owner, &mut right_commands);
                self.append_guarded_commands(&left, 1, right_commands, commands);
                commands.push(format!(
                    "execute if score {left} {} matches 1 unless score {right} {} matches 1 run scoreboard players set {left} {} 0",
                    self.objective, self.objective, self.objective,
                ));
                left
            }
            Condition::Or(left, right) => {
                if let Some(value) = constant_condition(left) {
                    if !value {
                        return self.compile_condition(right, owner, commands);
                    }
                    let flag = self.temporary();
                    commands.push(format!(
                        "scoreboard players set {flag} {} 1",
                        self.objective
                    ));
                    return flag;
                }
                let left = self.compile_condition(left, owner, commands);
                if let Some(value) = constant_condition(right) {
                    if value {
                        commands.push(format!(
                            "scoreboard players set {left} {} 1",
                            self.objective
                        ));
                    }
                    return left;
                }
                let mut right_commands = Vec::new();
                let right = self.compile_condition(right, owner, &mut right_commands);
                self.append_guarded_commands(&left, 0, right_commands, commands);
                commands.push(format!(
                    "execute if score {left} {} matches 0 if score {right} {} matches 1 run scoreboard players set {left} {} 1",
                    self.objective, self.objective, self.objective,
                ));
                left
            }
        }
    }

    /// 比较两侧按源码顺序求值；右侧可能改变左侧读取的状态时先冻结左值。
    fn compile_comparison_operands(
        &mut self,
        left: &Expr,
        right: &Expr,
        owner: &str,
        commands: &mut Vec<String>,
    ) -> (Value, Value) {
        let left_value = self.compile_expr(left, owner, commands);
        let left_value = self.freeze_before_effect(left_value, right, commands);
        let right_value = self.compile_expr(right, owner, commands);
        (left_value, right_value)
    }

    /// 把已经求值的比较固化成 0/1 标志计分项。
    fn comparison_flag(
        &self,
        flag: &str,
        comparison: Comparison,
        values: (Value, Value),
        commands: &mut Vec<String>,
    ) {
        match values {
            (Value::Integer(left), Value::Integer(right)) => commands.push(format!(
                "scoreboard players set {flag} {} {}",
                self.objective,
                i32::from(compare_integers(left, comparison, right))
            )),
            (Value::Score(score), Value::Integer(value)) => {
                self.compile_score_constant_comparison(flag, &score, comparison, value, commands);
            }
            (Value::Integer(value), Value::Score(score)) => self.compile_score_constant_comparison(
                flag,
                &score,
                reverse_comparison(comparison),
                value,
                commands,
            ),
            (Value::Score(left), Value::Score(right)) => {
                commands.push(format!(
                    "scoreboard players set {flag} {} 0",
                    self.objective
                ));
                let (prefix, symbol) = comparison_operator(comparison);
                commands.push(format!(
                    "execute {prefix} score {left} {} {symbol} {right} {} run scoreboard players set {flag} {} 1",
                    self.objective, self.objective, self.objective
                ));
            }
        }
    }

    /// `left && right` 中右侧是比较时，右侧只需要一个真假值：把它的否定直接
    /// 折进合并命令，省掉第二个标志计分项。
    ///
    /// 只有在能给出原生否定子句时才折进；否则右侧仍按原来的两步固化生成，
    /// 行为与旧路径逐条一致。
    fn fuse_condition_and(
        &mut self,
        left_flag: &str,
        right: &Condition,
        owner: &str,
        commands: &mut Vec<String>,
    ) -> bool {
        let Condition::Compare {
            left,
            comparison,
            right: right_operand,
        } = right
        else {
            return false;
        };

        // `&&` 短路：右侧只在左侧为真时求值，因此这些命令都带左侧守卫。
        let mut nested = Vec::new();
        let values = self.compile_comparison_operands(left, right_operand, owner, &mut nested);
        match self.negated_comparison_clause(*comparison, &values) {
            Some(clause) => nested.push(format!(
                "execute {clause} run scoreboard players set {left_flag} {} 0",
                self.objective
            )),
            None => {
                let flag = self.temporary();
                self.comparison_flag(&flag, *comparison, values, &mut nested);
                nested.push(format!(
                    "execute unless score {flag} {} matches 1 run scoreboard players set {left_flag} {} 0",
                    self.objective, self.objective
                ));
            }
        }
        self.append_guarded_commands(left_flag, 1, nested, commands);
        true
    }

    /// 已求值比较的否定原生子句；无法用单条 `execute` 条件表达时返回 `None`。
    fn negated_comparison_clause(
        &self,
        comparison: Comparison,
        values: &(Value, Value),
    ) -> Option<String> {
        match values {
            (Value::Score(score), Value::Integer(value)) => {
                let (test, range) = score_constant_clause(comparison, *value)?;
                Some(format!(
                    "{} score {score} {} matches {range}",
                    invert_test(test),
                    self.objective
                ))
            }
            (Value::Integer(value), Value::Score(score)) => {
                let (test, range) = score_constant_clause(reverse_comparison(comparison), *value)?;
                Some(format!(
                    "{} score {score} {} matches {range}",
                    invert_test(test),
                    self.objective
                ))
            }
            (Value::Score(left), Value::Score(right)) => {
                let (test, symbol) = comparison_operator(comparison);
                Some(format!(
                    "{} score {left} {} {symbol} {right} {}",
                    invert_test(test),
                    self.objective,
                    self.objective
                ))
            }
            _ => None,
        }
    }

    /// Compare a score against a literal without materializing the literal as
    /// another fake player. This is common in guards and saves one command and
    /// one temporary for every comparison.
    fn compile_score_constant_comparison(
        &self,
        flag: &str,
        score: &str,
        comparison: Comparison,
        value: i32,
        commands: &mut Vec<String>,
    ) {
        commands.push(format!(
            "scoreboard players set {flag} {} 0",
            self.objective
        ));
        if let Some((prefix, range)) = score_constant_clause(comparison, value) {
            commands.push(format!(
                "execute {prefix} score {score} {} matches {range} run scoreboard players set {flag} {} 1",
                self.objective, self.objective
            ));
        }
    }

    /// `&&` and `||` are short-circuiting. Expressions may call functions, so
    /// eagerly compiling the right side would also execute its side effects.
    fn append_guarded_commands(
        &self,
        guard: &str,
        expected: i32,
        guarded: Vec<String>,
        commands: &mut Vec<String>,
    ) {
        commands.extend(guarded.into_iter().map(|command| {
            if let Some(clauses) = command.strip_prefix("execute ") {
                format!(
                    "execute if score {guard} {} matches {expected} {clauses}",
                    self.objective
                )
            } else {
                format!(
                    "execute if score {guard} {} matches {expected} run {command}",
                    self.objective
                )
            }
        }));
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

    fn compile_command_success(&mut self, command: String, commands: &mut Vec<String>) -> String {
        let flag = self.temporary();
        commands.push(format!(
            "scoreboard players set {flag} {} 0",
            self.objective
        ));
        commands.push(format!(
            "execute store success score {flag} {} run {command}",
            self.objective
        ));
        flag
    }
}

fn invert_test(test: &str) -> &str {
    if test == "if" { "unless" } else { "if" }
}

fn score_constant_clause(comparison: Comparison, value: i32) -> Option<(&'static str, String)> {
    match comparison {
        Comparison::Equal => Some(("if", value.to_string())),
        Comparison::NotEqual => Some(("unless", value.to_string())),
        Comparison::Less if value == i32::MIN => None,
        Comparison::Less => Some(("if", format!("..{}", value - 1))),
        Comparison::LessEqual => Some(("if", format!("..{value}"))),
        Comparison::Greater if value == i32::MAX => None,
        Comparison::Greater => Some(("if", format!("{}..", value + 1))),
        Comparison::GreaterEqual => Some(("if", format!("{value}.."))),
    }
}

fn compare_integers(left: i32, comparison: Comparison, right: i32) -> bool {
    match comparison {
        Comparison::Equal => left == right,
        Comparison::NotEqual => left != right,
        Comparison::Less => left < right,
        Comparison::LessEqual => left <= right,
        Comparison::Greater => left > right,
        Comparison::GreaterEqual => left >= right,
    }
}

fn reverse_comparison(comparison: Comparison) -> Comparison {
    match comparison {
        Comparison::Equal => Comparison::Equal,
        Comparison::NotEqual => Comparison::NotEqual,
        Comparison::Less => Comparison::Greater,
        Comparison::LessEqual => Comparison::GreaterEqual,
        Comparison::Greater => Comparison::Less,
        Comparison::GreaterEqual => Comparison::LessEqual,
    }
}

fn comparison_operator(comparison: Comparison) -> (&'static str, &'static str) {
    match comparison {
        Comparison::Equal => ("if", "="),
        Comparison::NotEqual => ("unless", "="),
        Comparison::Less => ("if", "<"),
        Comparison::LessEqual => ("if", "<="),
        Comparison::Greater => ("if", ">"),
        Comparison::GreaterEqual => ("if", ">="),
    }
}
