use std::collections::{HashMap, HashSet};

use crate::ast::{EntityQueryDecl, ItemStackDecl};

/// 函数签名的语义摘要，供调用、调度和执行上下文检查使用。
#[derive(Clone, Copy)]
pub(super) struct Signature {
    pub(super) parameters: usize,
    pub(super) returns_score: bool,
    pub(super) required_context: ExecutionContext,
}

/// 执行上下文层级，`None < Entity < Player`。
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) enum ExecutionContext {
    None,
    Entity,
    Player,
}

/// 当前语句位置是否允许 `return`，以及所在函数是否返回 score。
#[derive(Clone, Copy)]
pub(super) struct ReturnRules {
    pub(super) returns_score: bool,
    pub(super) allowed_here: bool,
}

impl ReturnRules {
    /// 进入 `if`、`while`、`each` 等嵌套块后，`return` 不再直接对应外层函数。
    pub(super) fn nested(self) -> Self {
        Self {
            allowed_here: false,
            ..self
        }
    }
}

/// 语句校验可见的符号表。
pub(super) struct StatementSymbols<'a> {
    pub(super) scores: &'a HashSet<&'a str>,
    pub(super) parameters: &'a HashSet<&'a str>,
    pub(super) functions: &'a HashMap<&'a str, Signature>,
    pub(super) queries: &'a HashMap<&'a str, &'a EntityQueryDecl>,
    pub(super) item_stacks: &'a HashMap<&'a str, &'a ItemStackDecl>,
    pub(super) storages: &'a HashSet<&'a str>,
    pub(super) predicates: &'a HashSet<&'a str>,
}
