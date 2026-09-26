//! Safe direct lowering for adjacent if guards.

use crate::ast::{Condition, Statement, StatementKind};

use super::super::Compiler;
use super::super::condition_facts::{ScoreFacts, Truth, has_condition_effects};
use super::helpers::constant_condition;

/// Facts and clauses accumulated while walking one straight line of `if` guards.
///
/// Every guard in the line is evaluated before the leaf body runs, in the order
/// the source wrote them, so a condition proven true by an earlier guard is
/// still true when a later one is tested.
#[derive(Default)]
struct Chain {
    facts: ScoreFacts,
    /// Clauses an earlier guard already requires. Only effect-free guards are
    /// recorded: re-testing `function f()` or a predicate would repeat whatever
    /// they do, while re-testing a plain score test cannot change its answer.
    required: Vec<String>,
}

impl Chain {
    fn truth(&self, condition: &Condition) -> Truth {
        constant_condition(condition).map_or_else(
            || self.facts.truth(condition),
            |value| {
                if value { Truth::True } else { Truth::False }
            },
        )
    }

    fn already_required(&self, clause: &str) -> bool {
        self.required.iter().any(|required| required == clause)
    }

    fn require(&mut self, clause: &str) {
        self.required.push(clause.to_owned());
    }
}

enum Guard {
    AlwaysTrue,
    Impossible,
    Clause(String),
    Unavailable,
}

impl Compiler<'_> {
    /// Merge a straight line of nested `if` statements into one native execute
    /// chain when every guard can be represented without evaluating it twice.
    pub(super) fn compile_direct_if_chain(
        &mut self,
        condition: &Condition,
        then_body: &[Statement],
        owner: &str,
        commands: &mut Vec<String>,
    ) -> bool {
        if then_body.is_empty() {
            return false;
        }

        let mut conditions = vec![condition];
        let mut leaf = then_body;
        while let [statement] = leaf {
            let StatementKind::If {
                condition,
                then_body,
                else_body,
            } = &statement.kind
            else {
                break;
            };
            if !else_body.is_empty() {
                break;
            }
            conditions.push(condition);
            leaf = then_body;
        }

        let mut chain = Chain::default();
        let mut clauses = Vec::new();
        let mut prior_effects = false;
        for condition in conditions {
            match self.guard_clause(condition, owner, &mut chain) {
                Guard::AlwaysTrue => {}
                Guard::Impossible => {
                    if prior_effects || has_condition_effects(condition) {
                        return false;
                    }
                    return true;
                }
                Guard::Clause(clause) => clauses.push(clause),
                Guard::Unavailable => return false,
            }
            chain.facts.assume_true(condition);
            if chain.facts.is_impossible() {
                if prior_effects || has_condition_effects(condition) {
                    return false;
                }
                return true;
            }
            prior_effects |= has_condition_effects(condition);
        }

        if clauses.is_empty() {
            let mut body = self.compile_block(leaf, owner);
            commands.append(&mut body);
        } else {
            let block = self.compile_small_block(leaf, owner);
            commands.push(format!("execute {} run {block}", clauses.join(" ")));
        }
        true
    }

    fn guard_clause(&self, condition: &Condition, owner: &str, chain: &mut Chain) -> Guard {
        match chain.truth(condition) {
            Truth::True => return Guard::AlwaysTrue,
            Truth::False => {
                return if has_condition_effects(condition) {
                    Guard::Unavailable
                } else {
                    Guard::Impossible
                };
            }
            Truth::Unknown => {}
        }

        match condition {
            Condition::And(left, right) => {
                let left_guard = self.guard_clause(left, owner, chain);
                match left_guard {
                    Guard::AlwaysTrue => self.guard_clause(right, owner, chain),
                    Guard::Impossible => Guard::Impossible,
                    Guard::Unavailable => Guard::Unavailable,
                    Guard::Clause(left_clause) => {
                        chain.facts.assume_true(left);
                        let right_guard = self.guard_clause(right, owner, chain);
                        match right_guard {
                            Guard::AlwaysTrue => Guard::Clause(left_clause),
                            Guard::Impossible if has_condition_effects(left) => Guard::Unavailable,
                            Guard::Impossible | Guard::Unavailable => right_guard,
                            Guard::Clause(right_clause) => {
                                Guard::Clause(format!("{left_clause} {right_clause}"))
                            }
                        }
                    }
                }
            }
            Condition::Or(left, right) => {
                let left_truth = chain.truth(left);
                match left_truth {
                    // `||` short circuits, so a provably true left operand means
                    // the right operand is never evaluated.
                    Truth::True => Guard::AlwaysTrue,
                    Truth::False => self.guard_clause(right, owner, chain),
                    Truth::Unknown if chain.truth(right) == Truth::False => {
                        match self.guard_clause(left, owner, chain) {
                            Guard::Impossible => Guard::Impossible,
                            Guard::AlwaysTrue => Guard::AlwaysTrue,
                            Guard::Clause(clause) => Guard::Clause(clause),
                            Guard::Unavailable => Guard::Unavailable,
                        }
                    }
                    Truth::Unknown if chain.truth(right) == Truth::True => {
                        if has_condition_effects(left) {
                            Guard::Unavailable
                        } else {
                            Guard::AlwaysTrue
                        }
                    }
                    Truth::Unknown => Guard::Unavailable,
                }
            }
            _ => {
                let Some(clause) = self.direct_condition_clause(condition, false, owner) else {
                    return Guard::Unavailable;
                };
                if !has_condition_effects(condition) {
                    if chain.already_required(&clause) {
                        return Guard::AlwaysTrue;
                    }
                    chain.require(&clause);
                }
                chain.facts.assume_true(condition);
                Guard::Clause(clause)
            }
        }
    }
}
