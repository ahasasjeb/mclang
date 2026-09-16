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
    pub advancements: Vec<AdvancementDecl>,
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
    /// 准则；缺省是 `dummy`。
    pub criteria: Option<String>,
    /// 显示名，可以是组件或字符串。
    pub display_name: Option<TextComponent>,
    /// 渲染类型：`integer` 或 `hearts`。
    pub render_type: Option<String>,
    pub number_format: Option<NumberFormat>,
    /// 显示槽位，例如 `sidebar`。
    pub display_slot: Option<String>,
    pub display_slot_span: Option<Span>,
    pub span: Span,
}

/// 数字格式：`blank`、`fixed(<组件>)` 或 `styled`。
#[derive(Debug)]
pub enum NumberFormat {
    Blank,
    Fixed(TextComponent),
    Styled,
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
    /// 基础实体类型，`#` 前缀表示实体类型标签。
    pub entity_type: String,
    /// `type(...)`/`without_type(...)` 追加的类型约束。
    pub type_filters: Vec<EntityTypeFilter>,
    pub tags: Vec<String>,
    pub excluded_tags: Vec<String>,
    pub limit: Option<u32>,
    pub sort: Option<EntitySort>,
    pub within: Option<u32>,
    pub name_filter: Option<QueryTextFilter>,
    pub scores: Vec<QueryScoreFilter>,
    pub nbt_filter: Option<QueryTextFilter>,
    pub box_filter: Option<QueryBox>,
    pub distance: Option<String>,
    pub level: Option<String>,
    pub gamemode: Option<String>,
    pub team_filter: Option<QueryTextFilter>,
    pub rotation: Option<QueryRotation>,
    pub predicate: Option<String>,
    pub advancements: Option<String>,
    pub item: Option<ItemFilter>,
    pub span: Span,
}

/// 追加的实体类型约束：`type("#标签")` 或 `without_type("id")`。
#[derive(Debug)]
pub enum EntityTypeFilter {
    Include(String, Span),
    Exclude(String, Span),
}

/// 文本类选择器参数（`name`、`nbt`、`team`）：可带 `!` 否定。
#[derive(Debug)]
pub struct QueryTextFilter {
    pub value: String,
    pub negated: bool,
    pub span: Span,
}

/// `scores("目标", "区间")`。
#[derive(Debug)]
pub struct QueryScoreFilter {
    pub objective: String,
    pub range: String,
    pub span: Span,
}

/// `box(x, y, z, dx, dy, dz)`：选择器坐标盒。
#[derive(Debug)]
pub struct QueryBox {
    pub x: String,
    pub y: String,
    pub z: String,
    pub dx: String,
    pub dy: String,
    pub dz: String,
    pub span: Span,
}

/// `rotate("偏航区间", "俯仰区间")`。
#[derive(Debug)]
pub struct QueryRotation {
    pub yaw: String,
    pub pitch: String,
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
    /// `custom_data = nbt { ... };`：物品堆的 `minecraft:custom_data` 组件。
    pub custom_data: Option<NbtValue>,
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

/// 资源引用：标识符指本命名空间内的声明，字符串是完整的外部资源位置。
///
/// 进度声明的 `parent`、奖励的 `function`/`loot`/`recipe` 和
/// `advancement.grant/revoke` 的进度参数共用这套引用规则；外部引用只校验
/// 资源位置语法，本命名空间引用要求声明存在。
#[derive(Debug)]
pub struct AdvancementReference {
    pub name: String,
    pub span: Span,
    pub external: bool,
}

/// `advancement 名称 { ... }` 声明的结构化进度，输出到
/// `data/<命名空间>/advancement/<名称>.json`。
///
/// 进度是数据包唯一的事件入口：`criteria` 描述要监听的原版触发器，
/// 运行时命中后由 `rewards.function` 触发的函数接管后续逻辑。
#[derive(Debug)]
pub struct AdvancementDecl {
    pub name: String,
    pub name_span: Span,
    pub parent: Option<AdvancementReference>,
    pub criteria: Vec<AdvancementCriterion>,
    pub requirements: AdvancementRequirements,
    pub reward: Option<AdvancementReward>,
    pub display: Option<AdvancementDisplay>,
    pub span: Span,
}

/// 单条准则：`trigger` 是 26.3 注册的触发器名，`conditions` 是触发条件的原始 JSON。
#[derive(Debug)]
pub struct AdvancementCriterion {
    pub name: String,
    pub name_span: Span,
    pub trigger: String,
    pub trigger_span: Span,
    pub conditions: Option<String>,
    pub conditions_span: Option<Span>,
    pub span: Span,
}

/// `requirements` 的两种策略，对应 `AdvancementRequirements.Strategy`。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AdvancementRequirements {
    /// 全部准则都完成才算完成（默认）。
    All,
    /// 任意一条准则完成即算完成。
    Any,
}

/// 进度奖励：函数、经验、战利品表与配方。
#[derive(Debug)]
pub struct AdvancementReward {
    pub function: Option<AdvancementReference>,
    pub experience: Option<i32>,
    pub loot: Vec<AdvancementReference>,
    pub recipes: Vec<AdvancementReference>,
}

/// 进度展示信息，对应 26.3 的 `DisplayInfo`。
///
/// 布尔字段为 `None` 表示没有声明，生成时省略以保留原版默认值。
#[derive(Debug)]
pub struct AdvancementDisplay {
    pub icon: String,
    pub icon_span: Span,
    pub title: String,
    pub description: String,
    pub frame: AdvancementFrame,
    pub background: Option<String>,
    pub background_span: Option<Span>,
    pub show_toast: Option<bool>,
    pub announce_to_chat: Option<bool>,
    pub hidden: Option<bool>,
    pub span: Span,
}

/// `display.frame` 的三种进度框样式。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AdvancementFrame {
    Task,
    Goal,
    Challenge,
}

impl AdvancementFrame {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Task => "task",
            Self::Goal => "goal",
            Self::Challenge => "challenge",
        }
    }
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

/// 精确坐标（`vec3(x, y, z)`）：绝对分量允许小数，对应原版 `Vec3Argument`。
#[derive(Debug)]
pub struct Vec3Value {
    pub x: Coordinate,
    pub y: Coordinate,
    pub z: Coordinate,
    pub span: Span,
}

/// 水平精确坐标（`vec2(x, z)`），对应原版 `Vec2Argument`。
#[derive(Debug)]
pub struct Vec2Value {
    pub x: Coordinate,
    pub z: Coordinate,
    pub span: Span,
}

/// 朝向（`rotation(yaw, pitch)`），单位是度，对应原版 `RotationArgument`。
#[derive(Debug)]
pub struct RotationValue {
    pub yaw: Coordinate,
    pub pitch: Coordinate,
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

/// 结构化 NBT 值，覆盖 26.3 SNBT 的全部 12 种标签类型。
///
/// 语法统一写成 `nbt { 键 = 值; }`，值可以是带后缀的数值、字符串、列表、
/// 嵌套复合、字节/整数/长整数数组；解析期已经检查数值范围、数组元素类型与
/// 重复键，因此这里保存的都是合法值，代码生成只负责序列化。
#[derive(Debug)]
pub struct NbtValue {
    pub kind: NbtValueKind,
    pub span: Span,
}

#[derive(Debug)]
pub enum NbtValueKind {
    /// `1b`：TAG_Byte。
    Byte(i8),
    /// `1s`：TAG_Short。
    Short(i16),
    /// `1`：TAG_Int。
    Int(i32),
    /// `1L`：TAG_Long。
    Long(i64),
    /// `1.0f` 或 `1f`：TAG_Float。
    Float(f32),
    /// `1.0`、`1.0d` 或 `1d`：TAG_Double。
    Double(f64),
    /// `"文本"`：TAG_String。
    String(String),
    /// `[值, 值]`：TAG_List，元素类型可以不同。
    List(Vec<NbtValue>),
    /// `{ 键 = 值; }`：TAG_Compound。
    Compound(Vec<NbtEntry>),
    /// `[B; 1b, 2b]`：TAG_Byte_Array。
    ByteArray(Vec<i8>),
    /// `[I; 1, 2]`：TAG_Int_Array。
    IntArray(Vec<i32>),
    /// `[L; 1L, 2L]`：TAG_Long_Array。
    LongArray(Vec<i64>),
}

impl NbtValue {
    pub fn category(&self) -> NbtCategory {
        match self.kind {
            NbtValueKind::Byte(_)
            | NbtValueKind::Short(_)
            | NbtValueKind::Int(_)
            | NbtValueKind::Long(_)
            | NbtValueKind::Float(_)
            | NbtValueKind::Double(_) => NbtCategory::Numeric,
            NbtValueKind::String(_) => NbtCategory::String,
            NbtValueKind::List(_) => NbtCategory::List,
            NbtValueKind::Compound(_) => NbtCategory::Compound,
            NbtValueKind::ByteArray(_) | NbtValueKind::IntArray(_) | NbtValueKind::LongArray(_) => {
                NbtCategory::NumericArray
            }
        }
    }
}

/// NBT 值的粗分类，用于比对具名标签的期望类型。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NbtCategory {
    /// 字节、短整数、整数、长整数、单精度、双精度与布尔。
    Numeric,
    /// 字符串。
    String,
    /// 列表；元素类型可以不同。
    List,
    /// 复合。
    Compound,
    /// 字节/整数/长整数数组。
    NumericArray,
}

#[derive(Debug)]
pub struct NbtEntry {
    pub key: String,
    pub key_span: Span,
    pub value: NbtValue,
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
        /// 召唤位置；缺省时沿用执行位置。
        position: Option<PositionValue>,
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
        /// 方块实体数据，写在方块状态之后：`<block>{<nbt>}`。
        nbt: Option<NbtValue>,
    },
    Fill {
        from: BlockPosition,
        to: BlockPosition,
        block: BlockStateValue,
        mode: FillMode,
        filter: Option<BlockStateValue>,
        /// 方块实体数据，写在方块状态之后、模式与过滤器之前。
        nbt: Option<NbtValue>,
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
        component: TextComponent,
    },
    PlaySound {
        sound: String,
        source: String,
        /// sound.self 为 None（目标 @s）；sound.play 是玩家查询名。
        targets: Option<String>,
        position: Option<PositionValue>,
        volume: Option<String>,
        pitch: Option<String>,
        min_volume: Option<String>,
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
    /// `scoreboard.enable(持有者, 目标);`：允许玩家用 `/trigger` 修改。
    ScoreboardEnable {
        target: ScoreTarget,
    },
    /// `scoreboard.operation(结果, 运算, 来源);`：原版 `scoreboard players operation`。
    ScoreboardOperation {
        result: ScoreTarget,
        operation: ScoreboardOp,
        source: ScoreTarget,
    },
    /// `scoreboard.display("槽位"[, 目标]);`：设置或清除显示槽。
    ScoreboardDisplay {
        slot: String,
        slot_span: Span,
        objective: Option<(String, Span)>,
    },
    /// `teleport(持有者, 坐标或实体查询[, rotation(朝向)]);`：把实体移动到目标位置。
    Teleport {
        targets: Holder,
        destination: TeleportDestination,
        rotation: Option<RotationValue>,
    },
    /// `nbt { ... };`（中文 `数据 { ... };`）：把结构化 NBT 合并到当前实体。
    ///
    /// 生成 `data merge entity @s {...}`；键会对照 26.3 源码提取的实体标签表
    /// 检查（见 `version::entity_nbt`）。
    NbtMerge {
        nbt: NbtValue,
    },
    /// `advancement.grant/revoke(...)`：给玩家授予或撤销进度。
    AdvancementAction {
        operation: AdvancementOperation,
        scope: AdvancementScope,
        targets: Holder,
        advancement: Option<AdvancementReference>,
        criterion: Option<String>,
        criterion_span: Option<Span>,
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
    Center(Vec2Value),
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

/// `advancement grant/revoke` 的操作。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AdvancementOperation {
    Grant,
    Revoke,
}

impl AdvancementOperation {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Grant => "grant",
            Self::Revoke => "revoke",
        }
    }
}

/// 进度操作的作用范围，对应原生命令 `only`/`through`/`from`/`until`/`everything`。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AdvancementScope {
    Only,
    Through,
    From,
    Until,
    Everything,
}

impl AdvancementScope {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Only => "only",
            Self::Through => "through",
            Self::From => "from",
            Self::Until => "until",
            Self::Everything => "everything",
        }
    }
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

/// 文本组件（1.3）：`tellraw` 与后续界面命令共用的富文本。
///
/// 语法统一为 `<构造器>(...)` 加可选样式块，例如
/// `text("你好") { color = "red"; click = run_command("/say hi"); }`。
#[derive(Debug)]
pub struct TextComponent {
    pub kind: TextComponentKind,
    pub style: TextStyle,
    pub span: Span,
}

#[derive(Debug)]
pub enum TextComponentKind {
    /// `text("...")`：纯文本。
    Text(String),
    /// `translate("键", [参数...])`：本地化键与可选参数。
    Translate {
        key: String,
        args: Vec<TextComponent>,
    },
    /// `keybind("key.jump")`：按键名。
    Keybind(String),
    /// `score(持有者, 目标)`：计分板分数。目标可以是已声明的 `objective`
    /// 名称，也可以是字符串形式的运行期目标名。
    Score {
        holder: Holder,
        objective: ObjectiveRef,
        objective_span: Span,
    },
    /// `selector("@a")` 或 `selector(查询)`：实体选择器。
    Selector(SelectorValue),
    /// `nbt(实体/方块/存储, "路径")`：NBT 值。
    Nbt {
        source: NbtComponentSource,
        path: String,
        path_span: Span,
        interpret: bool,
        plain: bool,
        separator: Option<Box<TextComponent>>,
    },
}

/// 选择器组件的取值：字面选择器或已声明的实体查询。
#[derive(Debug)]
pub enum SelectorValue {
    /// 原样选择器文本（`@a`、`@e[...]`）。
    Raw(String, Span),
    /// 查询名；生成查询的选择器文本。
    Query(String, Span),
}

/// 计分组件的目标：已声明目标或运行期字符串。
#[derive(Debug)]
pub enum ObjectiveRef {
    /// 已声明的 `objective` 名称；生成 `<命名空间>_<名称>`。
    Declared(String),
    /// 字符串形式的运行期目标名，原样输出。
    Raw(String),
}

/// NBT 组件的数据来源。
#[derive(Debug)]
pub enum NbtComponentSource {
    Entity(Holder),
    Block(BlockPosition),
    Storage(String, Span),
}

/// 组件的样式与事件。
#[derive(Debug, Default)]
pub struct TextStyle {
    pub color: Option<String>,
    pub bold: Option<bool>,
    pub italic: Option<bool>,
    pub underlined: Option<bool>,
    pub strikethrough: Option<bool>,
    pub obfuscated: Option<bool>,
    pub click: Option<ClickEvent>,
    pub hover: Option<Box<TextComponent>>,
}

/// 点击事件（26.3 的 `click_event`）。
#[derive(Debug)]
pub enum ClickEvent {
    OpenUrl(String),
    RunCommand(String),
    SuggestCommand(String),
    CopyToClipboard(String),
    ChangePage(u32),
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
    /// `count(查询)`：查询命中的实体数量。
    Count {
        query: String,
        query_span: Span,
    },
    /// `random(最小值, 最大值)`：闭区间随机整数。
    Random {
        min: i32,
        max: i32,
    },
    /// `data.get(来源, 路径)`：NBT 数值或列表长度。
    DataGet {
        source: NbtComponentSource,
        path: String,
        path_span: Span,
    },
    /// `compute(来源, float|integer, "provider"[, 缩放])`。
    Compute {
        source: ComputeSource,
        kind: ComputeKind,
        provider: String,
        provider_span: Span,
        scale: Option<String>,
    },
    Negate(Box<Expr>),
    Binary {
        left: Box<Expr>,
        operation: BinaryOp,
        right: Box<Expr>,
    },
}

/// `compute` 的上下文来源。
#[derive(Debug)]
pub enum ComputeSource {
    Default,
    Block(BlockPosition),
    Entity(Holder),
}

/// `compute` 的数值类型。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ComputeKind {
    Float,
    Integer,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BinaryOp {
    Add,
    Subtract,
    Multiply,
    Divide,
    Modulo,
}
