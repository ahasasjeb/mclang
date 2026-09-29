//! 语句块的保守写集：判断重复求值同一个计分单元是否稳定。
//!
//! 只有在能证明「条件读取的计分单元不会被链上的后续求值或分支体改写」时，才允许把
//! 原生条件子句重复放进后续守卫；证明不了就调用方退回「先冻结到编译器临时
//! 项」的路径。因此这里宁可多报写入：多报只会少一个优化，漏报会改变语义。

use std::collections::BTreeSet;

use crate::ast::{
    ExecuteClauseKind, ExecuteClauses, ExecuteStoreTarget, Holder, ReturnKind, ScoreTarget,
    ScoreboardCommand, Statement, StatementKind,
};

use super::Compiler;
use super::condition_facts::{has_condition_effects, has_expression_effects};
use super::names::user_objective_name;

/// 一个语句块可能写入的计分单元，以及是否存在无法精确归纳的外部副作用。
#[derive(Clone, Debug, Default)]
pub(super) struct WriteSet {
    /// `<持有者>|<运行时目标>`；持有者无法确定时记成 `*|<目标>`。
    cells: BTreeSet<String>,
    /// 块内可能执行用户代码、raw 命令或其它有外部副作用的运行期求值：无法
    /// 证明全局计分变量与实体上的用户目标仍保持不变。
    external_effects: bool,
}

impl WriteSet {
    pub(super) fn merge(&mut self, other: &Self) {
        self.cells.extend(other.cells.iter().cloned());
        self.external_effects |= other.external_effects;
    }

    /// 写入是否可能覆盖 `<holder>|<objective>`。
    pub(super) fn touches(&self, holder: &str, objective: &str) -> bool {
        self.cells.contains(&cell_key(holder, objective))
            || self.cells.contains(&cell_key("*", objective))
            || self.cells.contains(&cell_key(holder, "*"))
            || self.cells.contains(&cell_key("*", "*"))
    }

    /// 区块内是否可能出现无法证明不改写外部计分状态的求值。
    pub(super) fn may_change_external_scores(&self) -> bool {
        self.external_effects
    }

    /// 条件求值可能运行用户代码或其它有状态操作时，不能假定此前读取的全局
    /// 计分单元在后续守卫里仍保持不变。
    pub(super) fn include_condition(&mut self, condition: &crate::ast::Condition) {
        self.external_effects |= has_condition_effects(condition);
    }

    fn include_expression(&mut self, expression: &crate::ast::Expr) {
        self.external_effects |= has_expression_effects(expression);
    }
}

fn cell_key(holder: &str, objective: &str) -> String {
    format!("{holder}|{objective}")
}

impl Compiler<'_> {
    /// 语句块的保守写集；`owner` 决定局部与参数持有者的名字。
    pub(super) fn block_writes(&self, body: &[Statement], owner: &str) -> WriteSet {
        let mut writes = WriteSet::default();
        self.block_writes_into(body, owner, &mut writes);
        writes
    }

    pub(super) fn block_writes_into(&self, body: &[Statement], owner: &str, writes: &mut WriteSet) {
        for statement in body {
            self.statement_writes(statement, owner, writes);
        }
    }

    fn statement_writes(&self, statement: &Statement, owner: &str, writes: &mut WriteSet) {
        match &statement.kind {
            StatementKind::Assign { target, value, .. }
            | StatementKind::Let {
                name: target,
                value,
                ..
            } => {
                writes.cells.insert(cell_key(
                    &self.variable_holder(owner, target),
                    &self.objective,
                ));
                writes.include_expression(value);
            }
            StatementKind::ScoreSet { target, value } => {
                self.score_target_writes(target, writes);
                writes.include_expression(value);
            }
            StatementKind::ScoreReset { target } | StatementKind::ScoreboardEnable { target } => {
                self.score_target_writes(target, writes)
            }
            StatementKind::ScoreboardOperation { result, .. } => {
                self.score_target_writes(result, writes);
            }
            StatementKind::ScoreboardCommand(command) => match command.as_ref() {
                ScoreboardCommand::ObjectivesRemove((objective, _)) => {
                    let objective = user_objective_name(&self.program.namespace, objective);
                    writes.cells.insert(cell_key("*", &objective));
                }
                ScoreboardCommand::PlayersChange { target, .. } => {
                    self.score_target_writes(target, writes);
                }
                ScoreboardCommand::PlayersResetAll(holder) => {
                    writes
                        .cells
                        .insert(cell_key(&self.holder_text(holder), "*"));
                }
                _ => {}
            },
            StatementKind::For {
                variable,
                start,
                end,
                body,
                ..
            } => {
                writes.cells.insert(cell_key(
                    &self.variable_holder(owner, variable),
                    &self.objective,
                ));
                writes.include_expression(start);
                writes.include_expression(end);
                self.block_writes_into(body, owner, writes);
            }
            StatementKind::If {
                condition,
                then_body,
                else_body,
            } => {
                writes.include_condition(condition);
                self.block_writes_into(then_body, owner, writes);
                self.block_writes_into(else_body, owner, writes);
            }
            StatementKind::Each { body, .. }
            | StatementKind::InDimension { body, .. }
            | StatementKind::Spawn { body, .. } => self.block_writes_into(body, owner, writes),
            StatementKind::While { condition, body } => {
                writes.include_condition(condition);
                self.block_writes_into(body, owner, writes);
            }
            StatementKind::Execute { clauses, body } => {
                match clauses {
                    ExecuteClauses::Raw(_) => writes.external_effects = true,
                    ExecuteClauses::Structured(clauses) => {
                        for clause in clauses {
                            match &clause.kind {
                                ExecuteClauseKind::If(condition)
                                | ExecuteClauseKind::Unless(condition) => {
                                    writes.include_condition(condition);
                                }
                                ExecuteClauseKind::StoreResult(ExecuteStoreTarget::Score(
                                    target,
                                ))
                                | ExecuteClauseKind::StoreSuccess(ExecuteStoreTarget::Score(
                                    target,
                                )) => self.score_target_writes(target, writes),
                                _ => {}
                            }
                        }
                    }
                }
                self.block_writes_into(body, owner, writes);
            }
            StatementKind::Call { .. }
            | StatementKind::MacroCall { .. }
            | StatementKind::Run(_) => {
                writes.external_effects = true;
            }
            StatementKind::Return(kind) => match kind {
                ReturnKind::Command(command) => {
                    writes.external_effects = true;
                    self.statement_writes(command, owner, writes);
                }
                ReturnKind::Run(_) | ReturnKind::Value(_) => writes.external_effects = true,
                ReturnKind::Void | ReturnKind::Fail => {}
            },
            // 其余语句只改世界、物品、实体状态或展示层，不会写计分单元。
            _ => {}
        }
    }

    fn score_target_writes(&self, target: &ScoreTarget, writes: &mut WriteSet) {
        let objective = user_objective_name(&self.program.namespace, &target.objective);
        writes
            .cells
            .insert(cell_key(&self.holder_text(&target.holder), &objective));
    }

    /// 写集里的持有者文本：`@s` 是当前实体，其余形式无法确定具体实体，记成
    /// 通配 `*`（比对只在 `<持有者>|<目标>` 与 `*|<目标>` 上做）。
    fn holder_text(&self, holder: &Holder) -> String {
        match holder {
            Holder::SelfEntity => "@s".to_owned(),
            Holder::Origin | Holder::Query(_, _) => "*".to_owned(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{WriteSet, cell_key};

    #[test]
    fn wildcard_writes_cover_every_matching_cell() {
        let mut holder_wildcard = WriteSet::default();
        holder_wildcard
            .cells
            .insert(cell_key("*", "example_points"));
        assert!(holder_wildcard.touches("@s", "example_points"));
        assert!(!holder_wildcard.touches("@s", "example_other"));

        let mut objective_wildcard = WriteSet::default();
        objective_wildcard.cells.insert(cell_key("@s", "*"));
        assert!(objective_wildcard.touches("@s", "example_points"));
        assert!(!objective_wildcard.touches("#other", "example_points"));

        let mut both_wildcard = WriteSet::default();
        both_wildcard.cells.insert(cell_key("*", "*"));
        assert!(both_wildcard.touches("@s", "example_points"));
        assert!(both_wildcard.touches("#other", "example_other"));
    }
}
