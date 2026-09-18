use super::*;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
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

/// 一个模块（一个 `.mcl` 文件）解析出的整程序。
///
/// 模块系统就位后 `namespace` 只在入口模块（项目根目录的 `main.mcl`）必需，
/// 其余模块可以省略；若写了则必须与入口一致，缺省时空串由模块解析补上。
/// `imports` 在模块解析阶段消费，合并后的整程序里为空。
#[derive(Debug)]
pub struct Program {
    pub namespace: String,
    pub namespace_span: Option<Span>,
    pub imports: Vec<ImportDecl>,
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

/// `import 数学::几何;` 或 `import 数学::{加法, 减法 as 减};`
///
/// 路径始终相对项目根目录（入口模块所在目录），`a::b` 对应 `a/b.mcl` 或
/// `a/b/mod.mcl`。`items` 为 `None` 表示导入整个模块的公开声明。
#[derive(Debug)]
pub struct ImportDecl {
    pub path: Vec<String>,
    pub path_span: Span,
    pub items: Option<Vec<ImportItem>>,
}

/// 选择性导入中的单个名字，可带 `as` 别名。
#[derive(Debug)]
pub struct ImportItem {
    pub name: String,
    pub name_span: Span,
    pub alias: Option<String>,
}

/// `objective 名称;`：声明一个用户计分板目标（dummy 准则）。
///
/// 运行期目标名是 `<命名空间>_<名称>`，例如 `portable_chest_box_key`；
/// `__mcl/load` 负责创建，`/reload` 不会清空已有分数。
#[derive(Debug)]
pub struct ObjectiveDecl {
    /// `export objective`：是否对其他模块公开。
    pub exported: bool,
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
