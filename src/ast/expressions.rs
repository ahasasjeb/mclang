use super::*;

#[derive(Debug)]
pub enum ExprKind {
    CoreCommand(Box<CoreCommand>),
    EntityCommand(Box<EntityCommand>),
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
    BossBarGet { id: String, property: BossBarQuery },
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
