//! `if`/`else` 链上的条件分派：互补的正/反原生子句 + 可选的前置求值命令。
//!
//! 每个条件编译成一个 [`Branch`]：`positive` 成立时执行当层分支，`negative`
//! 与它互补，累积进后续守卫。只要能做到，就直接比较源码里的计分单元（0 条
//! 求值命令）；做不到时先按源码顺序求值，把不稳定或不可直接比较的值冻结到
//! 编译器临时项，再比较临时项。编译器临时项全构建唯一，只有生成它的那条命令
//! 会写它，因此重复求值总是稳定的。

use crate::ast::{Comparison, Condition, Expr};
use crate::constant::constant_value;

use super::Compiler;
use super::Value;
use super::condition_facts::invert_comparison;
use super::conditions::{
    comparison_operator, invert_test, reverse_comparison, score_constant_clause,
};
use super::writes::WriteSet;

/// 条件操作数：常量，或一个可直接写进 `execute ... if score` 的计分单元。
enum Operand {
    Literal(i32),
    Cell { holder: String, objective: String },
}

/// 一个条件的两种极性。
///
/// 求值命令由调用方传入的 `setup` 收集：调用方负责给它们加上当前链式守卫。
pub(super) struct Branch {
    /// 条件成立时的原生子句。
    pub(super) positive: String,
    /// 与 `positive` 互补的子句。
    pub(super) negative: String,
}

impl Compiler<'_> {
    /// 编译一个条件的两极性。
    ///
    /// `stability` 为 `Some` 时表示该子句还会在分支体可能执行之后被再次求值
    /// （后续 else 层或最终 else 的守卫），此时只有稳定单元才允许直接比较，
    /// 其余值先冻结；为 `None` 表示只测试一次，直接用原生子句即可。
    pub(super) fn condition_branch(
        &mut self,
        condition: &Condition,
        owner: &str,
        stability: Option<&WriteSet>,
        setup: &mut Vec<String>,
    ) -> Branch {
        if let Some(branch) = self.comparison_branch(condition, owner, stability, setup) {
            return branch;
        }
        // 复合条件、谓词、函数条件等：先固化 0/1 标志，再按标志分派。
        let flag = self.compile_condition(condition, owner, setup);
        Branch {
            positive: format!("if score {flag} {} matches 1", self.objective),
            negative: format!("if score {flag} {} matches 0", self.objective),
        }
    }

    fn comparison_branch(
        &mut self,
        condition: &Condition,
        owner: &str,
        stability: Option<&WriteSet>,
        setup: &mut Vec<String>,
    ) -> Option<Branch> {
        let Condition::Compare {
            left,
            comparison,
            right,
        } = condition
        else {
            return None;
        };

        // 1) 直接比较源码单元：不产生求值命令。
        let left_operand = self.condition_operand(left, owner);
        let right_operand = self.condition_operand(right, owner);
        if let (Some(left_operand), Some(right_operand)) = (&left_operand, &right_operand)
            && operands_stable(left_operand, right_operand, stability)
            && let Some((positive, negative)) =
                comparison_clauses(*comparison, left_operand, right_operand)
        {
            return Some(Branch { positive, negative });
        }

        // 2) 冻结：按源码顺序求值两侧（右侧可能有副作用时左值已被冻结），
        //    再把不稳定或不可直接比较的值落到临时项。
        let values = self.compile_comparison_operands(left, right, owner, setup);
        let left_operand = self.frozen_operand(values.0, stability, setup);
        let right_operand = self.frozen_operand(values.1, stability, setup);
        if let Some((positive, negative)) =
            comparison_clauses(*comparison, &left_operand, &right_operand)
        {
            return Some(Branch { positive, negative });
        }
        // 已经求值过一次，不能再走 `compile_condition`（会重复副作用）；
        // 用同一份结果固化标志，`>` i32::MAX 这类边界由它判成恒假。
        let flag = self.temporary();
        let values = (left_operand.value(), right_operand.value());
        self.comparison_flag(&flag, *comparison, values, setup);
        Some(Branch {
            positive: format!("if score {flag} {} matches 1", self.objective),
            negative: format!("if score {flag} {} matches 0", self.objective),
        })
    }

    /// 未求值的源码操作数：常量，或可直接比较的计分单元。
    fn condition_operand(&self, expression: &Expr, owner: &str) -> Option<Operand> {
        if let Some(literal) = constant_value(expression) {
            return Some(Operand::Literal(literal));
        }
        self.condition_score_cell(expression, owner)
            .map(|(holder, objective)| Operand::Cell { holder, objective })
    }

    /// 已求值的操作数：不稳定时先复制到编译器临时项。
    ///
    /// 冻结源一定是编译器持有者（全局计分在 `__mcl/load` 初始化，局部与参数
    /// 在读写前赋值），因此单条 `operation` 就能取到当前值，不需要预置 0。
    fn frozen_operand(
        &mut self,
        value: Value,
        stability: Option<&WriteSet>,
        setup: &mut Vec<String>,
    ) -> Operand {
        let operand = match value {
            Value::Integer(literal) => Operand::Literal(literal),
            Value::Score(holder) => Operand::Cell {
                holder,
                objective: self.objective.clone(),
            },
        };
        let Some(writes) = stability else {
            return operand;
        };
        if operand_stable(&operand, writes) {
            return operand;
        }
        if let Operand::Cell { holder, objective } = operand {
            let frozen = self.temporary();
            setup.push(format!(
                "scoreboard players operation {frozen} {objective} = {holder} {objective}"
            ));
            Operand::Cell {
                holder: frozen,
                objective,
            }
        } else {
            operand
        }
    }
}

impl Operand {
    fn value(&self) -> Value {
        match self {
            Operand::Literal(literal) => Value::Integer(*literal),
            Operand::Cell { holder, .. } => Value::Score(holder.clone()),
        }
    }
}

fn operands_stable(left: &Operand, right: &Operand, stability: Option<&WriteSet>) -> bool {
    let Some(writes) = stability else {
        return true;
    };
    operand_stable(left, writes) && operand_stable(right, writes)
}

fn operand_stable(operand: &Operand, writes: &WriteSet) -> bool {
    match operand {
        Operand::Literal(_) => true,
        Operand::Cell { holder, objective } => {
            if writes.touches(holder, objective) {
                return false;
            }
            // 参数、局部变量、临时项与常量槽只有本函数能写；全局计分变量和
            // 实体上的用户目标可能被任何被调用函数改写。
            cell_is_private(holder) || !writes.may_change_external_scores()
        }
    }
}

fn cell_is_private(holder: &str) -> bool {
    ["#l_", "#p_", "#t", "#c_", "#loop_"]
        .iter()
        .any(|prefix| holder.starts_with(prefix))
}

/// 互补的正/反子句。无法用单条 `matches` 表达（例如 `x > 2147483647`）时
/// 返回 `None`，调用方在已求值的结果上退回 0/1 标志。
fn comparison_clauses(
    comparison: Comparison,
    left: &Operand,
    right: &Operand,
) -> Option<(String, String)> {
    match (left, right) {
        (Operand::Cell { holder, objective }, Operand::Literal(value)) => {
            let (positive, range) = score_constant_clause(comparison, *value)?;
            let (negative, negative_range) =
                score_constant_clause(invert_comparison(comparison), *value)?;
            Some((
                format!("{positive} score {holder} {objective} matches {range}"),
                format!("{negative} score {holder} {objective} matches {negative_range}"),
            ))
        }
        (Operand::Literal(value), Operand::Cell { holder, objective }) => {
            let comparison = reverse_comparison(comparison);
            let (positive, range) = score_constant_clause(comparison, *value)?;
            let (negative, negative_range) =
                score_constant_clause(invert_comparison(comparison), *value)?;
            Some((
                format!("{positive} score {holder} {objective} matches {range}"),
                format!("{negative} score {holder} {objective} matches {negative_range}"),
            ))
        }
        (
            Operand::Cell {
                holder: left_holder,
                objective: left_objective,
            },
            Operand::Cell {
                holder: right_holder,
                objective: right_objective,
            },
        ) => {
            let (positive, symbol) = comparison_operator(comparison);
            Some((
                format!(
                    "{positive} score {left_holder} {left_objective} {symbol} {right_holder} {right_objective}"
                ),
                format!(
                    "{} score {left_holder} {left_objective} {symbol} {right_holder} {right_objective}",
                    invert_test(positive)
                ),
            ))
        }
        (Operand::Literal(_), Operand::Literal(_)) => None,
    }
}

/// 给一批命令加上守卫子句：`execute ` 开头的命令并入子句，其余包一条 `run`。
pub(super) fn guard_commands(guard: &[String], commands: Vec<String>) -> Vec<String> {
    if guard.is_empty() {
        return commands;
    }
    let clauses = guard.join(" ");
    commands
        .into_iter()
        .map(|command| match command.strip_prefix("execute ") {
            Some(rest) => format!("execute {clauses} {rest}"),
            None => format!("execute {clauses} run {command}"),
        })
        .collect()
}
