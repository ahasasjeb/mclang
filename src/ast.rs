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
    pub objectives: Vec<ObjectiveDecl>,
    pub queries: Vec<EntityQueryDecl>,
    pub item_stacks: Vec<ItemStackDecl>,
    pub storages: Vec<StorageDecl>,
    pub data_slots: Vec<DataSlotDecl>,
    pub resources: Vec<ResourceDecl>,
    pub function_tags: Vec<FunctionTagDecl>,
    pub functions: Vec<Function>,
}

/// `objective 名称;`：声明一个用户计分板目标（dummy 准则）。
///
/// 运行期目标名是 `<命名空间>_<名称>`，例如 `portable_chest_box_key`；
/// `__mcl/load` 负责创建，`/reload` 不会清空已有分数。
#[derive(Debug)]
pub struct ObjectiveDecl {
    pub name: String,
    pub name_span: Span,
    pub span: Span,
}

/// 数据槽的来源：实体自带数据或物品堆自定义数据。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DataSlotKind {
    /// 实体的通用自定义数据，NBT 路径 `data.<键>`（26.3 `Entity` 的 `data` 字段）。
    EntityData,
    /// 物品堆的 `minecraft:custom_data` 组件，路径
    /// `Item.components."minecraft:custom_data".<键>`。
    ItemData,
}

/// `data_slot 名称 = item_data("键");` 或 `= entity_data("键");`
///
/// 数据槽是可读写任意 NBT 的命名位置，配合 `self.deposit/withdraw/remove_data`
/// 在容器物品与实体之间搬运数据。
#[derive(Debug)]
pub struct DataSlotDecl {
    pub name: String,
    pub name_span: Span,
    pub kind: DataSlotKind,
    pub key: String,
    pub key_span: Span,
    pub span: Span,
}

/// `fn_tag` 声明的函数标签，输出到 `data/<命名空间>/tags/function/<名称>.json`。
#[derive(Debug)]
pub struct FunctionTagDecl {
    pub name: String,
    pub name_span: Span,
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
    pub name_span: Span,
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
    pub name_span: Span,
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
    pub name_span: Span,
    pub storage_id: String,
    pub path: String,
    pub span: Span,
}

#[derive(Debug)]
pub struct ResourceDecl {
    pub kind: String,
    pub name: String,
    pub name_span: Span,
    pub json: String,
    pub span: Span,
}

#[derive(Debug)]
pub struct ScoreDecl {
    pub name: String,
    pub name_span: Span,
    pub initial: i32,
    pub span: Span,
}

#[derive(Debug)]
pub struct Function {
    pub name: String,
    pub name_span: Span,
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

/// 坐标分量：绝对数值、`~` 相对偏移或 `^` 局部偏移。
///
/// 文本已经规范化为 Minecraft 参数写法（`~1`、`^`、`-3.5`），代码生成直接转发。
#[derive(Clone, Debug)]
pub enum Coordinate {
    Absolute(String),
    Relative(String),
    Local(String),
}

impl Coordinate {
    pub fn text(&self) -> &str {
        match self {
            Self::Absolute(text) | Self::Relative(text) | Self::Local(text) => text,
        }
    }

    pub fn is_local(&self) -> bool {
        matches!(self, Self::Local(_))
    }

    /// 绝对坐标的整数值；相对与局部坐标返回 `None`。
    pub fn absolute_integer(&self) -> Option<i32> {
        match self {
            Self::Absolute(text) => text.parse().ok(),
            Self::Relative(_) | Self::Local(_) => None,
        }
    }
}

/// 三段方块坐标（`setblock`、`fill` 等的 `<pos>`）。
#[derive(Debug)]
pub struct BlockPosition {
    pub x: Coordinate,
    pub y: Coordinate,
    pub z: Coordinate,
    pub span: Span,
}

/// 两段列坐标（`forceload` 的 `<column>`），不包含 Y 轴。
#[derive(Debug)]
pub struct ColumnPosition {
    pub x: Coordinate,
    pub z: Coordinate,
    pub span: Span,
}

/// 方块状态字面量或方块谓词：`block_state("minecraft:oak_stairs") { facing = "east"; }`。
///
/// `#` 前缀的 id 是方块标签谓词，只能用在 `fill` 的过滤器与 `clone filtered` 里。
#[derive(Debug)]
pub struct BlockStateValue {
    pub id: String,
    pub properties: Vec<BlockProperty>,
    pub span: Span,
}

#[derive(Debug)]
pub struct BlockProperty {
    pub name: String,
    pub value: String,
    pub span: Span,
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
    StopwatchAction {
        operation: StopwatchOperation,
        id: String,
    },
    ClearInventory {
        target: String,
        item: Option<String>,
        max_count: Option<u32>,
    },
    SetBlock {
        pos: BlockPosition,
        block: BlockStateValue,
        mode: SetBlockMode,
    },
    Fill {
        from: BlockPosition,
        to: BlockPosition,
        block: BlockStateValue,
        mode: FillMode,
        filter: Option<BlockStateValue>,
    },
    FillBiome {
        from: BlockPosition,
        to: BlockPosition,
        biome: String,
        filter: Option<String>,
    },
    Clone {
        begin: BlockPosition,
        end: BlockPosition,
        destination: BlockPosition,
        from_dimension: Option<String>,
        to_dimension: Option<String>,
        filter: CloneFilter,
        mode: CloneMode,
        strict: bool,
    },
    PlaceFeature {
        feature: String,
        pos: Option<BlockPosition>,
    },
    PlaceJigsaw {
        pool: String,
        target: String,
        max_depth: u32,
        pos: Option<BlockPosition>,
    },
    PlaceStructure {
        structure: String,
        pos: Option<BlockPosition>,
    },
    PlaceTemplate {
        template: String,
        pos: BlockPosition,
        rotation: Option<TemplateRotation>,
        mirror: Option<TemplateMirror>,
        integrity: Option<String>,
        seed: Option<i32>,
        strict: bool,
    },
    ForceLoad(ForceLoadOperation),
    TimeAction {
        operation: TimeOperation,
        clock: Option<String>,
    },
    Weather {
        kind: WeatherKind,
        duration: Option<String>,
    },
    GameRuleSet {
        name: String,
        value: GameRuleValue,
    },
    WorldBorder(WorldBorderOperation),
    Locate {
        kind: LocateKind,
        target: String,
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
        name_span: Span,
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
    /// `scoreboard.set(持有者, 目标, 值);`：写用户计分板。
    ScoreSet {
        target: ScoreTarget,
        value: Expr,
    },
    /// `scoreboard.reset(持有者, 目标);`：删除计分项。
    ScoreReset {
        target: ScoreTarget,
    },
    /// `teleport(持有者, 坐标或实体查询);`：把实体移动到目标位置。
    Teleport {
        targets: Holder,
        destination: TeleportDestination,
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

/// `setblock` 的方块放置模式，对应原版可选字面量。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SetBlockMode {
    Destroy,
    Keep,
    Replace,
    Strict,
}

impl SetBlockMode {
    pub fn as_str(self) -> Option<&'static str> {
        match self {
            Self::Destroy => Some("destroy"),
            Self::Keep => Some("keep"),
            Self::Strict => Some("strict"),
            Self::Replace => None,
        }
    }
}

/// `fill` 的填充模式，对应原版可选字面量。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FillMode {
    Replace,
    Outline,
    Hollow,
    Destroy,
    Strict,
    Keep,
}

impl FillMode {
    pub fn as_str(self) -> Option<&'static str> {
        match self {
            Self::Replace => None,
            Self::Outline => Some("outline"),
            Self::Hollow => Some("hollow"),
            Self::Destroy => Some("destroy"),
            Self::Strict => Some("strict"),
            Self::Keep => Some("keep"),
        }
    }
}

/// `clone` 的方块过滤方式：全部、只复制非空气或按方块谓词过滤。
#[derive(Debug)]
pub enum CloneFilter {
    Replace,
    Masked,
    Filtered(BlockStateValue),
}

/// `clone` 的复制模式。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CloneMode {
    Normal,
    Force,
    Move,
}

impl CloneMode {
    pub fn as_str(self) -> Option<&'static str> {
        match self {
            Self::Normal => None,
            Self::Force => Some("force"),
            Self::Move => Some("move"),
        }
    }
}

/// 26.3 `Rotation` 枚举的模板旋转值。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TemplateRotation {
    None,
    Clockwise90,
    Clockwise180,
    Counterclockwise90,
}

impl TemplateRotation {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Clockwise90 => "clockwise_90",
            Self::Clockwise180 => "180",
            Self::Counterclockwise90 => "counterclockwise_90",
        }
    }
}

/// 26.3 `Mirror` 枚举的模板镜像值。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TemplateMirror {
    None,
    LeftRight,
    FrontBack,
}

impl TemplateMirror {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::LeftRight => "left_right",
            Self::FrontBack => "front_back",
        }
    }
}

/// `forceload` 的四种操作。
#[derive(Debug)]
pub enum ForceLoadOperation {
    Add {
        from: ColumnPosition,
        to: Option<ColumnPosition>,
    },
    Remove {
        from: ColumnPosition,
        to: Option<ColumnPosition>,
    },
    RemoveAll,
    Query {
        pos: Option<ColumnPosition>,
    },
}

/// `time` 的无返回值操作；查询在表达式中使用。
#[derive(Debug)]
pub enum TimeOperation {
    Set(String),
    Add(String),
    Pause,
    Resume,
    Rate(String),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WeatherKind {
    Clear,
    Rain,
    Thunder,
}

impl WeatherKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Clear => "clear",
            Self::Rain => "rain",
            Self::Thunder => "thunder",
        }
    }
}

/// `gamerule` 的取值：布尔规则或整数规则。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GameRuleValue {
    Bool(bool),
    Integer(i32),
}

/// `worldborder` 的无返回值操作；`get` 在表达式中使用。
#[derive(Debug)]
pub enum WorldBorderOperation {
    Add {
        distance: String,
        time: Option<String>,
    },
    Set {
        distance: String,
        time: Option<String>,
    },
    Center {
        x: String,
        z: String,
    },
    DamageAmount(String),
    DamageBuffer(String),
    WarningDistance(u32),
    WarningTime(String),
}

/// `locate` 的三类目标。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LocateKind {
    Structure,
    Biome,
    Poi,
}

impl LocateKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Structure => "structure",
            Self::Biome => "biome",
            Self::Poi => "poi",
        }
    }
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

/// 26.3 `stopwatch` 的三种无返回值操作。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StopwatchOperation {
    Create,
    Restart,
    Remove,
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

/// 持有者引用：当前实体、投掷者（`origin`）或实体查询。
///
/// 计分读写与传送目标共用这套引用；中文分别是 `自身`、`投掷者` 和查询名称。
#[derive(Debug)]
pub enum Holder {
    SelfEntity,
    Origin,
    Query(String, Span),
}

/// 「持有者 + 用户计分板目标」的组合，供计分读写使用。
#[derive(Debug)]
pub struct ScoreTarget {
    pub holder: Holder,
    pub objective: String,
    pub objective_span: Span,
}

/// `teleport` 的落点：绝对/相对坐标，或一个单个实体（跟随它的位置与朝向）。
#[derive(Debug)]
pub enum TeleportDestination {
    Position(BlockPosition),
    Entity { query: String, query_span: Span },
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
    /// `scoreboard.get(持有者, 目标)`：读取用户计分板的值。
    ScoreQuery {
        target: ScoreTarget,
    },
    StopwatchQuery {
        id: String,
        /// 原版 `scale` 参数，缺省为 1；保留规范化文本以避免双精度往返误差。
        scale: Option<String>,
    },
    /// `time.query([时钟])`：默认或指定世界时钟的总游戏刻。
    TimeQuery {
        clock: Option<String>,
    },
    /// `time.query_gametime()`：世界的游戏时间。
    GameTimeQuery,
    /// `gamerule.query(规则)`：游戏规则的命令结果值。
    GameRuleQuery {
        name: String,
    },
    /// `worldborder.get()`：世界边界边长。
    WorldBorderSize,
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
