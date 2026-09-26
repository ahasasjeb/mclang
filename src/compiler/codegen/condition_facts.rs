//! Small integer range facts for adjacent, side-effect-free score guards.

use std::collections::HashMap;

use crate::ast::{Comparison, Condition, ExprKind};
use crate::compiler::constant::constant_value;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Truth {
    True,
    False,
    Unknown,
}

#[derive(Clone, Copy)]
struct IntRange {
    min: i64,
    max: i64,
    excluded: Option<i32>,
}

impl Default for IntRange {
    fn default() -> Self {
        Self {
            min: i64::from(i32::MIN),
            max: i64::from(i32::MAX),
            excluded: None,
        }
    }
}

/// Facts learned from the true path of score-versus-constant comparisons.
/// The deliberately small domain keeps this useful without guessing about
/// calls, random values, or general arithmetic relationships.
#[derive(Default)]
pub(super) struct ScoreFacts {
    ranges: HashMap<String, IntRange>,
}

impl ScoreFacts {
    pub(super) fn truth(&self, condition: &Condition) -> Truth {
        // A condition that may run user code, read a predicate or consume
        // random state must still be evaluated even when its result is already
        // implied by the incoming facts: `f() || known_true` proves the branch
        // is taken, but only after `f()` has run. Report `Unknown` so callers
        // keep the original evaluation path.
        if has_condition_effects(condition) {
            return Truth::Unknown;
        }
        match condition {
            Condition::Compare {
                left,
                comparison,
                right,
            } => score_comparison(left, *comparison, right)
                .map_or(Truth::Unknown, |(name, comparison, value)| {
                    self.compare(name, comparison, value)
                }),
            Condition::Not(inner) => invert(self.truth(inner)),
            Condition::And(left, right) => match (self.truth(left), self.truth(right)) {
                (Truth::False, _) | (_, Truth::False) => Truth::False,
                (Truth::True, Truth::True) => Truth::True,
                _ => Truth::Unknown,
            },
            Condition::Or(left, right) => match (self.truth(left), self.truth(right)) {
                (Truth::True, _) | (_, Truth::True) => Truth::True,
                (Truth::False, Truth::False) => Truth::False,
                _ => Truth::Unknown,
            },
            _ => Truth::Unknown,
        }
    }

    /// Add only constraints that must hold when this condition evaluates true.
    pub(super) fn assume_true(&mut self, condition: &Condition) {
        match condition {
            Condition::Compare {
                left,
                comparison,
                right,
            } => {
                if let Some((name, comparison, value)) = score_comparison(left, *comparison, right)
                {
                    self.constrain(name, comparison, value);
                }
            }
            Condition::Not(inner) => {
                if let Some((name, comparison, value)) = comparison_atom(inner) {
                    self.constrain(name, invert_comparison(comparison), value);
                }
            }
            Condition::And(left, right) => {
                self.assume_true(left);
                self.assume_true(right);
            }
            Condition::Or(left, right) => {
                if self.truth(left) == Truth::False {
                    self.assume_true(right);
                } else if self.truth(right) == Truth::False {
                    self.assume_true(left);
                }
            }
            _ => {}
        }
    }

    pub(super) fn is_impossible(&self) -> bool {
        self.ranges
            .values()
            .any(|range| range.min > range.max || singleton_is_excluded(*range))
    }

    fn compare(&self, name: &str, comparison: Comparison, value: i32) -> Truth {
        let range = self.ranges.get(name).copied().unwrap_or_default();
        let value = i64::from(value);
        let is_outside = value < range.min || value > range.max;
        let is_excluded = range
            .excluded
            .is_some_and(|excluded| i64::from(excluded) == value);
        let is_singleton = range.min == range.max;
        let result = match comparison {
            Comparison::Equal if is_outside || is_excluded => Some(false),
            Comparison::Equal if is_singleton => Some(true),
            Comparison::NotEqual if is_outside || is_excluded => Some(true),
            Comparison::NotEqual if is_singleton => Some(false),
            Comparison::Less if range.max < value => Some(true),
            Comparison::Less if range.min >= value => Some(false),
            Comparison::LessEqual if range.max <= value => Some(true),
            Comparison::LessEqual if range.min > value => Some(false),
            Comparison::Greater if range.min > value => Some(true),
            Comparison::Greater if range.max <= value => Some(false),
            Comparison::GreaterEqual if range.min >= value => Some(true),
            Comparison::GreaterEqual if range.max < value => Some(false),
            _ => None,
        };
        match result {
            Some(true) => Truth::True,
            Some(false) => Truth::False,
            None => Truth::Unknown,
        }
    }

    fn constrain(&mut self, name: &str, comparison: Comparison, value: i32) {
        let range = self.ranges.entry(name.to_owned()).or_default();
        let value = i64::from(value);
        match comparison {
            Comparison::Equal => {
                range.min = range.min.max(value);
                range.max = range.max.min(value);
            }
            Comparison::NotEqual => range.excluded = Some(value as i32),
            Comparison::Less => range.max = range.max.min(value - 1),
            Comparison::LessEqual => range.max = range.max.min(value),
            Comparison::Greater => range.min = range.min.max(value + 1),
            Comparison::GreaterEqual => range.min = range.min.max(value),
        }
    }
}

pub(super) fn has_condition_effects(condition: &Condition) -> bool {
    match condition {
        // A predicate can consume random state; a function explicitly runs user code.
        Condition::Predicate { .. } | Condition::Function { .. } => true,
        Condition::Compare { left, right, .. } => {
            has_expression_effects(left) || has_expression_effects(right)
        }
        Condition::Not(inner) => has_condition_effects(inner),
        Condition::And(left, right) | Condition::Or(left, right) => {
            has_condition_effects(left) || has_condition_effects(right)
        }
        _ => false,
    }
}

fn has_expression_effects(expression: &crate::ast::Expr) -> bool {
    match &expression.kind {
        ExprKind::Call { .. }
        | ExprKind::CoreCommand(_)
        | ExprKind::EntityCommand(_)
        | ExprKind::Random { .. } => true,
        ExprKind::Negate(inner) => has_expression_effects(inner),
        ExprKind::Binary { left, right, .. } => {
            has_expression_effects(left) || has_expression_effects(right)
        }
        _ => false,
    }
}

fn score_comparison<'a>(
    left: &'a crate::ast::Expr,
    comparison: Comparison,
    right: &'a crate::ast::Expr,
) -> Option<(&'a str, Comparison, i32)> {
    match (&left.kind, &right.kind) {
        (ExprKind::Score(name), _) => Some((name, comparison, constant_value(right)?)),
        (_, ExprKind::Score(name)) => {
            Some((name, reverse_comparison(comparison), constant_value(left)?))
        }
        _ => None,
    }
}

fn comparison_atom(condition: &Condition) -> Option<(&str, Comparison, i32)> {
    let Condition::Compare {
        left,
        comparison,
        right,
    } = condition
    else {
        return None;
    };
    score_comparison(left, *comparison, right)
}

fn invert(truth: Truth) -> Truth {
    match truth {
        Truth::True => Truth::False,
        Truth::False => Truth::True,
        Truth::Unknown => Truth::Unknown,
    }
}

fn invert_comparison(comparison: Comparison) -> Comparison {
    match comparison {
        Comparison::Equal => Comparison::NotEqual,
        Comparison::NotEqual => Comparison::Equal,
        Comparison::Less => Comparison::GreaterEqual,
        Comparison::LessEqual => Comparison::Greater,
        Comparison::Greater => Comparison::LessEqual,
        Comparison::GreaterEqual => Comparison::Less,
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

fn singleton_is_excluded(range: IntRange) -> bool {
    range.min == range.max
        && range
            .excluded
            .is_some_and(|excluded| i64::from(excluded) == range.min)
}
