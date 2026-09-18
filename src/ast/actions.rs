use super::*;

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

/// `item` 命令的方法。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ItemMethod {
    Replace,
    Fill,
    Override,
    Modify,
}
