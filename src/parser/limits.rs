//! 在递归下降前限制调用深度，在构造运算链时限制树深度。
//! 后者覆盖没有括号的 `a + b + ...`，保护后续遍历、codegen 和析构。

use super::Parser;
use crate::ast::{Condition, Expr, ExprKind, Span};
use crate::diagnostic::Diagnostic;

pub(super) const MAX_NESTING: usize = 64;

enum Node<'a> {
    Expression(&'a Expr),
    Condition(&'a Condition),
}

impl Parser {
    pub(super) fn nested<T>(
        &mut self,
        parse: impl FnOnce(&mut Self) -> Result<T, Diagnostic>,
    ) -> Result<T, Diagnostic> {
        if self.nesting_depth >= MAX_NESTING {
            return Err(Self::depth_error(self.current().span));
        }
        self.nesting_depth += 1;
        let result = parse(self);
        self.nesting_depth -= 1;
        result
    }

    fn depth_error(span: Span) -> Diagnostic {
        Diagnostic::new(
            format!("语法嵌套过深（上限 {MAX_NESTING}）；请拆分代码块或表达式"),
            span,
        )
    }

    pub(super) fn check_expression_depth(&self, expression: &Expr) -> Result<(), Diagnostic> {
        self.check_tree_depth(Node::Expression(expression), expression.span)
    }

    pub(super) fn check_condition_depth(&self, condition: &Condition) -> Result<(), Diagnostic> {
        self.check_tree_depth(Node::Condition(condition), self.previous().span)
    }

    fn check_tree_depth(&self, root: Node<'_>, span: Span) -> Result<(), Diagnostic> {
        let mut pending = vec![(root, self.nesting_depth + 1)];
        while let Some((node, depth)) = pending.pop() {
            if depth > MAX_NESTING {
                return Err(Self::depth_error(span));
            }
            match node {
                Node::Expression(expr) => match &expr.kind {
                    ExprKind::Negate(value) => pending.push((Node::Expression(value), depth + 1)),
                    ExprKind::Binary { left, right, .. } => {
                        pending.push((Node::Expression(left), depth + 1));
                        pending.push((Node::Expression(right), depth + 1));
                    }
                    ExprKind::Call { arguments, .. } => pending.extend(
                        arguments
                            .iter()
                            .map(|arg| (Node::Expression(arg), depth + 1)),
                    ),
                    _ => {}
                },
                Node::Condition(condition) => match condition {
                    Condition::Not(value) => pending.push((Node::Condition(value), depth + 1)),
                    Condition::And(left, right) | Condition::Or(left, right) => {
                        pending.push((Node::Condition(left), depth + 1));
                        pending.push((Node::Condition(right), depth + 1));
                    }
                    Condition::Compare { left, right, .. } => {
                        pending.push((Node::Expression(left), depth + 1));
                        pending.push((Node::Expression(right), depth + 1));
                    }
                    _ => {}
                },
            }
        }
        Ok(())
    }
}
