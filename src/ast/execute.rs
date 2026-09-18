use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AssignOp {
    Set,
    Add,
    Subtract,
    Multiply,
    Divide,
    Modulo,
}

/// `execute` 的子句列表：结构化子句，或原样转发的字符串子句。
///
/// 字符串子句是 `--deny-raw` 会拒绝的逃生口；结构化子句全部下降为真实命令形状。
#[derive(Debug)]
pub enum ExecuteClauses {
    /// `execute "as @a at @s" { ... }`：子句字符串原样拼进命令。
    Raw(String),
    /// `execute as(查询) if 条件 store.result(持有者, 目标) { ... }`。
    Structured(Vec<ExecuteClause>),
}

/// 一条结构化 `execute` 子句。
#[derive(Debug)]
pub struct ExecuteClause {
    pub kind: ExecuteClauseKind,
    pub span: Span,
}

/// 结构化 `execute` 子句：修饰符、条件与 store。
///
/// 书写顺序固定为“修饰符 → 条件 → store”，重复的修饰符在语义阶段报错。
#[derive(Debug)]
pub enum ExecuteClauseKind {
    /// `as(查询)`：以查询命中的每个实体作为执行实体。
    As { query: String, query_span: Span },
    /// `at(查询)`：在查询命中的每个实体处执行，但保留当前执行实体。
    At { query: String, query_span: Span },
    /// `positioned(坐标)`：设置执行位置，并把锚点重置为脚部。
    Positioned(PositionValue),
    /// `rotated(朝向)`：设置执行朝向。
    Rotated(RotationValue),
    /// `facing(坐标)`：把执行朝向改为面向一个坐标。
    FacingPosition(PositionValue),
    /// `facing(entity(查询), 锚点)`：把执行朝向改为面向实体的眼睛或脚部。
    FacingEntity {
        query: String,
        query_span: Span,
        anchor: Anchor,
    },
    /// `align(轴)`：把执行位置按 `x`/`y`/`z` 组合对齐到方块。
    Align { axes: String },
    /// `anchored(锚点)`：切换后续相对坐标的锚点。
    Anchored(Anchor),
    /// `in("维度")`：切换执行维度。
    In {
        dimension: String,
        dimension_span: Span,
    },
    /// `on(关系)`：以当前实体的指定关系实体继续执行。
    On(EntityRelation),
    /// `summon("实体类型")`：召唤实体并以新实体继续执行。
    Summon {
        entity_type: String,
        entity_type_span: Span,
    },
    /// `if <条件>`：条件全部成立才执行块。
    If(Condition),
    /// `unless <条件>`：条件不成立才执行块。
    Unless(Condition),
    /// `store.result(目标)`：把块内最后一条命令的结果写入目标。
    StoreResult(ExecuteStoreTarget),
    /// `store.success(目标)`：把块内最后一条命令的成功与否写入目标。
    StoreSuccess(ExecuteStoreTarget),
    /// `store.data(来源, "路径", 类型[, 缩放])`：把结果写入 NBT。
    StoreData(ExecuteStoreData),
}

/// `store.result/success` 的写入目标：用户计分板或 Boss 栏字段。
#[derive(Debug)]
pub enum ExecuteStoreTarget {
    /// `(<持有者>, <目标>)`：用户计分板，持有者支持 `self`、`origin` 与实体查询。
    Score(ScoreTarget),
    /// `(bossbar, "<资源位置>", value|max)`：Boss 栏的当前值或上限，对应原版
    /// `store result|success bossbar <id> value|max`。
    BossBar {
        id: String,
        id_span: Span,
        field: BossBarField,
    },
}

/// Boss 栏的可写字段。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BossBarField {
    /// `value`：当前值，可小于或大于上限。
    Value,
    /// `max`：上限。
    Max,
}

impl BossBarField {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Value => "value",
            Self::Max => "max",
        }
    }
}

/// `store.data` 的写入模式：结果或成功与否。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StoreDataMode {
    Result,
    Success,
}

impl StoreDataMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Result => "result",
            Self::Success => "success",
        }
    }
}

/// 实体锚点：眼睛或脚部，对应原版 `EntityAnchorArgument.Anchor`。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Anchor {
    Eyes,
    Feet,
}

impl Anchor {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Eyes => "eyes",
            Self::Feet => "feet",
        }
    }
}

/// `execute on` 的实体关系，对应原版 `ExecuteCommand.createRelationOperations`。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EntityRelation {
    Owner,
    Leasher,
    Target,
    Attacker,
    Vehicle,
    Controller,
    Origin,
    Passengers,
}

impl EntityRelation {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Owner => "owner",
            Self::Leasher => "leasher",
            Self::Target => "target",
            Self::Attacker => "attacker",
            Self::Vehicle => "vehicle",
            Self::Controller => "controller",
            Self::Origin => "origin",
            Self::Passengers => "passengers",
        }
    }
}

/// `store.data` 的数值类型，对应原版 `store result <accessor> <path>` 后的标签类型。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StoreDataType {
    Byte,
    Short,
    Int,
    Long,
    Float,
    Double,
}

impl StoreDataType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Byte => "byte",
            Self::Short => "short",
            Self::Int => "int",
            Self::Long => "long",
            Self::Float => "float",
            Self::Double => "double",
        }
    }
}

/// `store.data(<模式>, <来源>, "<路径>", <类型>[, <缩放>])`：把结果或成功与否写进 NBT。
#[derive(Debug)]
pub struct ExecuteStoreData {
    /// `result`（缺省）或 `success`。
    pub mode: StoreDataMode,
    pub source: NbtComponentSource,
    pub path: String,
    pub path_span: Span,
    pub kind: StoreDataType,
    /// 原版 `scale` 参数，缺省为 1。
    pub scale: Option<String>,
}

#[derive(Debug)]
pub enum Condition {
    Predicate {
        name: String,
        span: Span,
    },
    Compare {
        left: Box<Expr>,
        comparison: Comparison,
        right: Box<Expr>,
    },
    /// `if block(pos, block_state)`：方块谓词（方块或 `#标签`）。
    Block {
        pos: BlockPosition,
        block: BlockStateValue,
        span: Span,
    },
    /// `if blocks(起点, 终点, 目标[, masked])`：区域方块比较。
    Blocks {
        start: BlockPosition,
        end: BlockPosition,
        destination: BlockPosition,
        masked: bool,
        span: Span,
    },
    /// `if biome(pos, "生物群系或 #标签")`。
    Biome {
        pos: BlockPosition,
        biome: String,
        biome_span: Span,
        span: Span,
    },
    /// `if loaded(pos)`：区块已加载。
    Loaded {
        pos: BlockPosition,
        span: Span,
    },
    /// `if dimension("维度")`：当前维度。
    Dimension {
        dimension: String,
        dimension_span: Span,
        span: Span,
    },
    /// `if entity(查询)`：查询是否命中实体。
    Entity {
        query: String,
        query_span: Span,
        span: Span,
    },
    /// `if data(来源, 路径)`：NBT 路径是否存在。
    Data {
        source: NbtComponentSource,
        path: String,
        path_span: Span,
        span: Span,
    },
    /// `if items(来源, 槽位, 物品谓词)`：槽位里是否有匹配物品。
    Items {
        source: ItemConditionSource,
        slots: String,
        slots_span: Span,
        item: String,
        item_span: Span,
        span: Span,
    },
    /// `if slots(来源, 槽位)`：槽位里是否有物品。
    Slots {
        source: ItemConditionSource,
        slots: String,
        slots_span: Span,
        span: Span,
    },
    /// `if function(函数或 #标签)`：函数是否返回成功。
    Function {
        target: CallTarget,
        span: Span,
    },
    /// `if stopwatch("id")`：秒表是否在运行。
    Stopwatch {
        id: String,
        span: Span,
    },
    Not(Box<Condition>),
    And(Box<Condition>, Box<Condition>),
    Or(Box<Condition>, Box<Condition>),
}

/// `if items`/`if slots` 的来源：实体或方块。
#[derive(Debug)]
pub enum ItemConditionSource {
    Entity(Holder),
    Block(BlockPosition),
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
