use super::*;

/// 数字格式：`blank`、`fixed(<组件>)` 或 `styled`。
#[derive(Debug)]
pub enum NumberFormat {
    Blank,
    Fixed(Box<TextComponent>),
    Styled(String),
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
    /// `export data_slot`：是否对其他模块公开。
    pub exported: bool,
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
    /// `export fn_tag`：是否对其他模块公开。
    pub exported: bool,
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
    /// `export query`：是否对其他模块公开。
    pub exported: bool,
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
    pub item_id: ItemPredicate,
    pub count: Option<u32>,
    pub custom_name: Option<String>,
    pub span: Span,
}

#[derive(Debug)]
pub struct ItemStackDecl {
    /// `export item`：是否对其他模块公开。
    pub exported: bool,
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
    pub components: Option<NbtValue>,
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
    /// `export storage`：是否对其他模块公开。
    pub exported: bool,
    pub name: String,
    pub name_span: Span,
    pub storage_id: String,
    pub path: String,
    pub span: Span,
}

#[derive(Debug)]
pub struct ResourceDecl {
    /// `export resource`：是否对其他模块公开。
    pub exported: bool,
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
    /// `export advancement`：是否对其他模块公开。
    pub exported: bool,
    pub name: String,
    pub name_span: Span,
    pub parent: Option<AdvancementReference>,
    pub criteria: Vec<AdvancementCriterion>,
    pub requirements: AdvancementRequirements,
    pub reward: Option<AdvancementReward>,
    pub display: Option<AdvancementDisplay>,
    pub span: Span,
}

/// 单条准则：`trigger` 是 26.3 注册的触发器名，conditions 在解析时只做一次 JSON parse。
#[derive(Debug)]
pub struct AdvancementCriterion {
    pub name: String,
    pub name_span: Span,
    pub trigger: String,
    pub trigger_span: Span,
    pub conditions: Option<AdvancementConditions>,
    pub conditions_span: Option<Span>,
    pub span: Span,
}

/// 进度条件 JSON 的解析结果。
///
/// 将解析失败也保存在 AST 中，校验阶段仍能在原 conditions span 报告错误，
/// 同时成功解析的 JSON 可直接供校验和代码生成共用。
#[derive(Debug)]
pub enum AdvancementConditions {
    Parsed(serde_json::Value),
    Invalid {
        line: usize,
        column: usize,
        message: String,
    },
}

impl AdvancementConditions {
    pub fn parse(source: &str) -> Self {
        match serde_json::from_str(source) {
            Ok(value) => Self::Parsed(value),
            Err(error) => Self::Invalid {
                line: error.line(),
                column: error.column(),
                message: error.to_string(),
            },
        }
    }
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
    /// `export score`：是否对其他模块公开。
    pub exported: bool,
    pub name: String,
    pub name_span: Span,
    pub initial: i32,
    pub span: Span,
}

#[derive(Debug)]
pub struct Function {
    /// `export fn`：是否对其他模块公开。
    pub exported: bool,
    pub name: String,
    pub name_span: Span,
    pub parameters: Vec<Parameter>,
    pub macro_signature: Option<MacroSignature>,
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
    Macro(String),
    Absolute(String),
    Relative(String),
    Local(String),
}
