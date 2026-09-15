#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Span {
    pub source: usize,
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub fn merge(self, other: Span) -> Span {
        Span {
            source: self.source,
            start: self.start,
            end: other.end,
        }
    }
}

#[derive(Debug)]
pub struct Program {
    pub namespace: String,
    pub namespace_span: Span,
    pub scores: Vec<ScoreDecl>,
    pub queries: Vec<EntityQueryDecl>,
    pub item_stacks: Vec<ItemStackDecl>,
    pub storages: Vec<StorageDecl>,
    pub resources: Vec<ResourceDecl>,
    pub function_tags: Vec<FunctionTagDecl>,
    pub functions: Vec<Function>,
}

/// `fn_tag` 声明的函数标签，输出到 `data/<命名空间>/tags/function/<名称>.json`。
#[derive(Debug)]
pub struct FunctionTagDecl {
    pub name: String,
    pub values: Vec<FunctionTagEntry>,
    /// 对应标签文件的 `replace` 字段；26.3 的默认值是 `false`（与低优先级包合并）。
    pub replace: bool,
    pub span: Span,
}

#[derive(Debug)]
pub enum FunctionTagEntry {
    /// 本命名空间内的函数。
    Function(String, Span),
    /// `#名称`：本命名空间内的函数标签。
    Tag(String, Span),
    /// 字符串形式的外部引用，`#` 前缀表示标签；内容按资源位置校验，不做存在性检查。
    External(String, Span),
}

#[derive(Debug)]
pub struct EntityQueryDecl {
    pub name: String,
    pub entity_type: String,
    pub tags: Vec<String>,
    pub excluded_tags: Vec<String>,
    pub limit: Option<u32>,
    pub sort: Option<EntitySort>,
    pub within: Option<u32>,
    pub item: Option<ItemFilter>,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EntitySort {
    Nearest,
    Furthest,
    Random,
    Arbitrary,
}

impl EntitySort {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Nearest => "nearest",
            Self::Furthest => "furthest",
            Self::Random => "random",
            Self::Arbitrary => "arbitrary",
        }
    }
}

#[derive(Debug)]
pub struct ItemFilter {
    pub slot: String,
    pub item_id: String,
    pub count: Option<u32>,
    pub custom_name: Option<String>,
    pub span: Span,
}

#[derive(Debug)]
pub struct ItemStackDecl {
    pub name: String,
    pub item_id: String,
    pub count: u32,
    pub custom_name: Option<String>,
    pub item_name: Option<String>,
    pub lore: Vec<String>,
    pub enchantments: Vec<ItemEnchantment>,
    pub stored_enchantments: Vec<ItemEnchantment>,
    pub damage: Option<u32>,
    pub max_damage: Option<u32>,
    pub max_stack_size: Option<u32>,
    pub rarity: Option<ItemRarity>,
    pub item_model: Option<String>,
    pub dyed_color: Option<u32>,
    pub enchantment_glint_override: Option<bool>,
    pub unbreakable: bool,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ItemRarity {
    Common,
    Uncommon,
    Rare,
    Epic,
}

impl ItemRarity {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Common => "common",
            Self::Uncommon => "uncommon",
            Self::Rare => "rare",
            Self::Epic => "epic",
        }
    }
}

#[derive(Debug)]
pub struct ItemEnchantment {
    pub enchantment_id: String,
    pub level: u32,
    pub span: Span,
}

#[derive(Debug)]
pub struct StorageDecl {
    pub name: String,
    pub storage_id: String,
    pub path: String,
    pub span: Span,
}

#[derive(Debug)]
pub struct ResourceDecl {
    pub kind: String,
    pub name: String,
    pub json: String,
    pub span: Span,
}

#[derive(Debug)]
pub struct ScoreDecl {
    pub name: String,
    pub initial: i32,
    pub span: Span,
}

#[derive(Debug)]
pub struct Function {
    pub name: String,
    pub parameters: Vec<Parameter>,
    pub returns_score: bool,
    pub attributes: Vec<Attribute>,
    pub body: Vec<Statement>,
    pub span: Span,
}

#[derive(Debug)]
pub struct Parameter {
    pub name: String,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Attribute {
    Load,
    Tick,
    Entity,
    Player,
    NonPlayer,
}

#[derive(Debug)]
pub struct Statement {
    pub kind: StatementKind,
    pub span: Span,
}

#[derive(Debug)]
pub enum StatementKind {
    Run(String),
    Each {
        query: String,
        body: Vec<Statement>,
    },
    InDimension {
        dimension: String,
        body: Vec<Statement>,
    },
    Spawn {
        entity_type: String,
        body: Vec<Statement>,
    },
    Give {
        target: GiveTarget,
        item: GiveItem,
        count: Option<u32>,
        count_span: Option<Span>,
    },
    EffectGive {
        target: String,
        effect: String,
        duration: EffectDuration,
        amplifier: Option<u32>,
        hide_particles: bool,
    },
    EffectClear {
        target: String,
        effect: Option<String>,
    },
    XpChange {
        target: String,
        kind: XpKind,
        operation: XpOperation,
        amount: i32,
    },
    ClearInventory {
        target: String,
        item: Option<String>,
        max_count: Option<u32>,
    },
    SelfAction(SelfAction),
    Message {
        target: MessageTarget,
        text: String,
        color: Option<String>,
    },
    PlaySound {
        sound: String,
        source: String,
    },
    Call {
        target: CallTarget,
        arguments: Vec<Expr>,
    },
    Let {
        name: String,
        value: Expr,
    },
    Schedule {
        target: CallTarget,
        delay: String,
        mode: ScheduleMode,
    },
    ScheduleClear {
        function: String,
    },
    Assign {
        target: String,
        operation: AssignOp,
        value: Expr,
    },
    If {
        condition: Condition,
        then_body: Vec<Statement>,
        else_body: Vec<Statement>,
    },
    Execute {
        clauses: String,
        body: Vec<Statement>,
    },
    While {
        condition: Condition,
        body: Vec<Statement>,
    },
    Return(ReturnKind),
}

/// `effect give` 的持续时间：整数秒或 `infinite`。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EffectDuration {
    Seconds(u32),
    Infinite,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum XpOperation {
    Add,
    Set,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum XpKind {
    Points,
    Levels,
}

impl XpKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Points => "points",
            Self::Levels => "levels",
        }
    }
}

/// 被调用或被调度的目标：本命名空间函数或 `#` 函数标签。
#[derive(Debug)]
pub enum CallTarget {
    Function(String),
    Tag(String),
}

/// `return` 的四种形式。
#[derive(Debug)]
pub enum ReturnKind {
    Void,
    Value(Expr),
    Fail,
    Run(String),
}

#[derive(Debug)]
pub enum GiveTarget {
    Query(String),
    Origin,
}

#[derive(Debug)]
pub enum GiveItem {
    Definition(String),
    SelfItem,
}

#[derive(Debug)]
pub enum SelfAction {
    AddTag(String),
    RemoveTag(String),
    SetInvulnerable(bool),
    SaveItems(String),
    RestoreItems(String),
    RemovePreservingItems(String),
    GiveItem {
        item: String,
        count: Option<u32>,
        count_span: Option<Span>,
    },
    ClearItems,
    Remove,
}

#[derive(Debug)]
pub enum MessageTarget {
    All,
    SelfEntity,
    Nearest { within: u32 },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScheduleMode {
    Replace,
    Append,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AssignOp {
    Set,
    Add,
    Subtract,
    Multiply,
    Divide,
    Modulo,
}

#[derive(Debug)]
pub enum Condition {
    Predicate {
        name: String,
        span: Span,
    },
    Compare {
        left: Expr,
        comparison: Comparison,
        right: Expr,
    },
    Not(Box<Condition>),
    And(Box<Condition>, Box<Condition>),
    Or(Box<Condition>, Box<Condition>),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Comparison {
    Equal,
    NotEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
}

#[derive(Debug)]
pub struct Expr {
    pub kind: ExprKind,
    pub span: Span,
}

#[derive(Debug)]
pub enum ExprKind {
    Integer(i32),
    Score(String),
    Call {
        function: String,
        arguments: Vec<Expr>,
    },
    XpQuery {
        target: String,
        kind: XpKind,
    },
    Negate(Box<Expr>),
    Binary {
        left: Box<Expr>,
        operation: BinaryOp,
        right: Box<Expr>,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BinaryOp {
    Add,
    Subtract,
    Multiply,
    Divide,
    Modulo,
}
