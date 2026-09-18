use super::*;

impl ItemMethod {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Replace => "replace",
            Self::Fill => "fill",
            Self::Override => "override",
            Self::Modify => "modify",
        }
    }
}

/// `item` 命令的内容：`with <物品>`、`from <来源> <槽位>[ <修饰器>]` 或 `modify <修饰器>`。
#[derive(Debug)]
pub enum ItemActionKind {
    With(String, Span),
    From {
        source: ItemConditionSource,
        slots: String,
        slots_span: Span,
        modifier: Option<String>,
    },
    Modifier(String, Span),
}

/// `data modify` 的操作。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DataOperationKind {
    Insert,
    Prepend,
    Append,
    Set,
    Merge,
}

/// `data modify` 的完整操作：操作种类、`insert` 下标与数据来源。
#[derive(Debug)]
pub struct DataOperation {
    pub kind: DataOperationKind,
    pub index: Option<i32>,
    pub source: DataSource,
}

/// `data modify` 的数据来源。
#[derive(Debug)]
pub enum DataSource {
    /// `from <来源目标> <路径>`
    From {
        target: NbtComponentSource,
        path: String,
        path_span: Span,
    },
    /// `value <NBT 标签>`
    Value(NbtValue),
    /// `string <来源目标> <路径> [起始 [结束]]`
    String {
        target: NbtComponentSource,
        path: String,
        path_span: Span,
        start: Option<i32>,
        end: Option<i32>,
    },
    /// `compute <上下文> float|integer <provider> [缩放]`
    Compute {
        source: ComputeSource,
        kind: ComputeKind,
        provider: String,
        provider_span: Span,
        scale: Option<String>,
    },
}

/// `scoreboard players operation` 的运算。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScoreboardOp {
    Set,
    Add,
    Subtract,
    Multiply,
    Divide,
    Modulo,
    Min,
    Max,
    Swap,
}

impl ScoreboardOp {
    pub fn symbol(self) -> &'static str {
        match self {
            Self::Set => "=",
            Self::Add => "+=",
            Self::Subtract => "-=",
            Self::Multiply => "*=",
            Self::Divide => "/=",
            Self::Modulo => "%=",
            Self::Min => "<",
            Self::Max => ">",
            Self::Swap => "><",
        }
    }
}

/// `teleport` 的落点：绝对/相对坐标，或一个单个实体（跟随它的位置与朝向）。
#[derive(Debug)]
pub enum TeleportDestination {
    Position(PositionValue),
    Entity { query: String, query_span: Span },
}

/// 位置值：方块坐标（整数）或精确坐标（小数）。
#[derive(Debug)]
pub enum PositionValue {
    Block(BlockPosition),
    Exact(Vec3Value),
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
    SetNoGravity(bool),
    SaveItems(String),
    RestoreItems(String),
    RemovePreservingItems(String),
    /// `self.remove_preserving_items(槽, 查询);`：把物品追加进数据槽，成功清空后再移除实体。
    RemovePreservingSlot {
        slot: String,
        slot_span: Span,
        query: String,
        query_span: Span,
    },
    GiveItem {
        item: String,
        count: Option<u32>,
        count_span: Option<Span>,
    },
    ClearItems,
    Remove,
    /// `self.deposit(槽, 目标查询);`：把 `@s.Items` 写入目标实体的数据槽。
    DataStore {
        slot: String,
        slot_span: Span,
        query: String,
        query_span: Span,
    },
    /// `self.withdraw(槽, 来源查询);`：把来源实体的数据槽读进 `@s.Items`，并清空来源槽。
    DataLoad {
        slot: String,
        slot_span: Span,
        query: String,
        query_span: Span,
    },
    /// `self.remove_data(槽);`：删除 `@s` 上的数据槽。
    DataClear {
        slot: String,
        slot_span: Span,
    },
}

#[derive(Debug)]
pub enum MessageTarget {
    All,
    SelfEntity,
    Nearest {
        within: u32,
    },
    /// `message.player(<查询>, <组件>)`：向查询命中的玩家广播。
    Query {
        name: String,
        name_span: Span,
    },
}
