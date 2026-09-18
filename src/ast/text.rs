use super::*;

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
