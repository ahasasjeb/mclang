use std::collections::{HashMap, HashSet};

use crate::ast::{
    AdvancementDecl, DataSlotDecl, EntityQueryDecl, FunctionTagDecl, ItemStackDecl, ObjectiveDecl,
};

/// 函数签名的语义摘要，供调用、调度和执行上下文检查使用。
#[derive(Clone, Copy)]
pub(super) struct Signature {
    pub(super) is_macro: bool,
    pub(super) parameters: usize,
    pub(super) returns_score: bool,
    pub(super) required_context: ExecutionContext,
}

/// 执行上下文，代码生成时决定 `@s` 指向什么。
///
/// - `None`：没有执行实体，例如 `@load`/`@tick` 入口函数；
/// - `Entity`：有实体，但可能是玩家，`@entity` 函数属于这一类；
/// - `Mob`：确定不是玩家的实体，来自非玩家查询的 `each`、非玩家 `spawn`
///   和 `@non_player` 函数；
/// - `Player`：确定是玩家，来自玩家查询的 `each` 和 `@player` 函数。
///
/// `Mob` 和 `Player` 都是 `Entity` 的特例，但互不包含：Minecraft 拒绝
/// `data ... entity` 作用于玩家，所以修改实体 NBT 的操作只允许 `Mob`。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ExecutionContext {
    None,
    Entity,
    Mob,
    Player,
}

impl ExecutionContext {
    pub(super) fn is_entity(self) -> bool {
        !matches!(self, Self::None)
    }

    /// 当前上下文是否满足某个最低要求。`Mob` 满足 `Entity`，但也仅此而已。
    pub(super) fn satisfies(self, required: Self) -> bool {
        match required {
            Self::None => true,
            Self::Entity => self.is_entity(),
            Self::Mob => matches!(self, Self::Mob),
            Self::Player => matches!(self, Self::Player),
        }
    }
}

/// 当前语句位置是否允许 `return`、所在函数是否返回 score，以及嵌套的
/// `for`/`while` 循环层数（`break`/`continue` 只允许出现在循环体内）。
#[derive(Clone, Copy)]
pub(super) struct ReturnRules {
    pub(super) returns_score: bool,
    pub(super) allowed_here: bool,
    pub(super) loop_depth: u32,
}

impl ReturnRules {
    /// 进入 `if`、`while`、`each` 等嵌套块后，`return` 不再直接对应外层函数。
    pub(super) fn nested(self) -> Self {
        Self {
            allowed_here: false,
            ..self
        }
    }

    /// 进入 `for`/`while` 循环体：嵌套层数加一，`return` 同样不再直接对应外层函数。
    pub(super) fn in_loop(self) -> Self {
        Self {
            allowed_here: false,
            loop_depth: self.loop_depth + 1,
            ..self
        }
    }
}

/// 语句校验可见的符号表。
pub(super) struct StatementSymbols<'a> {
    pub(super) scores: &'a HashSet<&'a str>,
    pub(super) objectives: &'a HashSet<&'a str>,
    /// 目标声明表，供 scoreboard.enable 等语句读取准则。
    pub(super) objective_declarations: &'a HashMap<&'a str, &'a ObjectiveDecl>,
    pub(super) parameters: &'a HashSet<&'a str>,
    pub(super) functions: &'a HashMap<&'a str, Signature>,
    pub(super) function_declarations: &'a HashMap<&'a str, &'a crate::ast::Function>,
    pub(super) queries: &'a HashMap<&'a str, &'a EntityQueryDecl>,
    pub(super) item_stacks: &'a HashMap<&'a str, &'a ItemStackDecl>,
    pub(super) storages: &'a HashSet<&'a str>,
    pub(super) data_slots: &'a HashMap<&'a str, &'a DataSlotDecl>,
    pub(super) predicates: &'a HashSet<&'a str>,
    pub(super) loot_tables: &'a HashSet<&'a str>,
    pub(super) recipes: &'a HashSet<&'a str>,
    pub(super) dialogs: &'a HashSet<&'a str>,
    pub(super) advancements: &'a HashMap<&'a str, &'a AdvancementDecl>,
    pub(super) advancement_resources: &'a HashSet<&'a str>,
    pub(super) function_tags: &'a HashMap<&'a str, &'a FunctionTagDecl>,
}
