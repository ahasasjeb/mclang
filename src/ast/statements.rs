use super::*;

#[derive(Debug)]
pub enum StatementKind {
    MacroCall {
        target: CallTarget,
        arguments: MacroArguments,
    },
    CoreCommand(Box<CoreCommand>),
    EntityCommand(Box<EntityCommand>),
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
        target: Option<Holder>,
        item: Option<ItemPredicate>,
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
        feature: PlaceFeatureSource,
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
        target: CallTarget,
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
        rotation: Option<Facing>,
    },
    /// `nbt { ... };`（中文 `数据 { ... };`）：把结构化 NBT 合并到当前实体。
    ///
    /// 生成 `data merge entity @s {...}`；键会对照 26.3 源码提取的实体标签表
    /// 检查（见 `version::entity_nbt`）。
    NbtMerge {
        nbt: NbtValue,
    },
    /// `data.merge(<目标>, nbt { ... });`：把复合标签合并进目标 NBT。
    DataMerge {
        target: NbtComponentSource,
        nbt: NbtValue,
    },
    /// `data.remove(<目标>, "<路径>");`：删除路径。
    DataRemove {
        target: NbtComponentSource,
        path: String,
        path_span: Span,
    },
    /// `data.modify(<目标>, "<路径>", <操作>[, <参数>]);`。
    DataModify {
        target: NbtComponentSource,
        path: String,
        path_span: Span,
        operation: DataOperation,
    },
    /// `item.replace/fill/override/modify(...)`：原版 `item` 命令。
    ItemAction {
        method: ItemMethod,
        target: ItemConditionSource,
        slots: String,
        slots_span: Span,
        action: ItemActionKind,
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
    /// `execute` 结构子句块；旧式字符串子句见 [`ExecuteClauses::Raw`]。
    Execute {
        clauses: ExecuteClauses,
        body: Vec<Statement>,
    },
    While {
        condition: Condition,
        body: Vec<Statement>,
    },
    /// `for <变量> in <起点>..<终点> { ... }`：半开区间，变量是循环体内的局部计分项。
    For {
        variable: String,
        variable_span: Span,
        start: Expr,
        end: Expr,
        body: Vec<Statement>,
    },
    /// `break`：跳出最近一层 `for`/`while`。
    Break,
    /// `continue`：进入最近一层 `for`/`while` 的下一次迭代。
    Continue,
    Return(ReturnKind),
}

#[derive(Debug)]
pub enum PlaceFeatureSource {
    Registered(String),
    Inline(NbtValue),
}

/// `setblock` 的方块放置模式，对应原版可选字面量。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SetBlockMode {
    Destroy,
    Keep,
    Replace,
    Strict,
}
