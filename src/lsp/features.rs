//! 语言特性：补全、悬停与跳转。
//!
//! 关键词与属性直接来自解析器的中英文关键词表，补全和悬停因此不会与语言定义脱节；
//! 声明名称来自 [`crate::analysis`] 的符号表。

use std::path::Path;

use serde_json::{Value, json};

use crate::analysis::{SourceFile, Symbol, SymbolKind};
use crate::parser::keywords::{ATTRIBUTES, KEYWORDS};

use super::convert::{offset_to_position, path_to_uri, word_at};

/// 每个关键词的一句话说明，悬停与补全文档使用；与 `KEYWORDS` 逐项对应。
const KEYWORD_DOCS: &[(&str, &str)] = &[
    (
        "namespace",
        "声明项目命名空间；同一项目内的所有文件必须一致。",
    ),
    ("score", "声明全局计分变量，并给出初始值。"),
    (
        "objective",
        "声明用户计分板目标；配合 `scoreboard.set/reset/get` 记录玩家或实体的状态。",
    ),
    (
        "scoreboard",
        "计分板读写：`scoreboard.set(持有者, 目标, 值)`、`scoreboard.reset(持有者, 目标)`，`scoreboard.get(持有者, 目标)` 是表达式。",
    ),
    (
        "query",
        "声明可复用的实体查询，供 each、give、effect 等语句使用。",
    ),
    ("entity", "实体查询构造器，参数是实体类型资源位置。"),
    ("item", "声明可复用的物品定义。"),
    ("item_stack", "物品堆构造器，参数是物品资源位置。"),
    (
        "item_list",
        "物品列表存储构造器，参数是存储资源位置与 NBT 路径。",
    ),
    ("storage", "声明物品列表存储，用于保存与恢复实体物品。"),
    (
        "data_slot",
        "声明数据槽：实体通用数据或物品堆自定义数据里的一个 NBT 位置。",
    ),
    (
        "item_data",
        "物品数据槽构造器，参数是物品堆 `minecraft:custom_data` 里的键。",
    ),
    (
        "entity_data",
        "实体数据槽构造器，参数是实体通用 `data` 字段里的键。",
    ),
    ("resource", "声明原始 JSON 资源，例如 predicate。"),
    (
        "predicate",
        "resource 可声明的资源类型；条件中可按名称引用谓词。",
    ),
    ("fn", "声明函数，可选参数与 `-> score` 返回值。"),
    (
        "fn_tag",
        "声明函数标签，供 `call #标签()` 与 schedule 使用。",
    ),
    ("let", "声明只在当前函数体内可见的局部变量。"),
    ("return", "结束函数：返回计分值、`fail` 或 `run` 原生命令。"),
    ("fail", "`return fail`：让调用方看到这次执行失败。"),
    ("if", "条件分支。"),
    ("else", "条件分支的否定分支。"),
    ("while", "条件循环。"),
    ("each", "对查询结果逐个执行，并进入实体上下文。"),
    ("call", "调用函数或 `#标签`。"),
    ("schedule", "延时调度函数或 `#标签`。"),
    ("after", "schedule 的延迟时间，例如 `after 2 s`。"),
    ("append", "调度模式：追加一次调度，而不是替换。"),
    (
        "replace",
        "调度模式：替换已有调度；也用于函数标签的 replace 属性。",
    ),
    ("give", "把物品定义交给查询中的玩家。"),
    ("origin", "give 的目标形式：投掷者或来源实体。"),
    ("in_dimension", "在指定维度的上下文中执行。"),
    ("spawn", "召唤实体并进入新的实体上下文。"),
    ("self", "当前实体上下文；`self.*` 是实体操作。"),
    (
        "message",
        "向玩家发送文本消息：`message.all/self/nearest/player` 接受文本组件或字符串。",
    ),
    ("sound", "播放声音。"),
    ("effect", "状态效果操作。"),
    ("xp", "经验值操作。"),
    ("clear", "清空查询玩家的物品。"),
    ("stopwatch", "秒表操作。"),
    ("run", "原生命令逃生口：执行未结构化的命令文本。"),
    (
        "execute",
        "原生命令子句逃生口：在额外执行上下文中运行语句。",
    ),
    ("contents", "物品查询的槽位名：容器内容。"),
    (
        "set_block",
        "放置单个方块：`set_block(pos, block_state[, 模式])`。",
    ),
    (
        "fill",
        "填充长方体区域：`fill(起点, 终点, block_state[, 模式][, replace 过滤器])`。",
    ),
    (
        "fill_biome",
        "填充生物群系区域：`fill_biome(起点, 终点, \"生物群系\"[, replace, \"过滤器\"])`。",
    ),
    (
        "clone",
        "复制区域：`clone(起点, 终点, 目标位置[, 选项...])`，支持跨维度与 filtered/masked。",
    ),
    (
        "place",
        "放置地物、拼图、结构或模板：`place.feature/jigsaw/structure/template(...)`。",
    ),
    (
        "forceload",
        "强制加载区块：`forceload.add/remove/remove_all/query(...)`。",
    ),
    (
        "time",
        "世界时钟：`time.set/add/pause/resume/rate(...)`，`time.query()` 与 `time.query_gametime()` 是表达式。",
    ),
    (
        "weather",
        "天气：`weather.clear/rain/thunder([持续时间])`。",
    ),
    (
        "gamerule",
        "游戏规则：`gamerule.set(\"规则\", 值)`，`gamerule.query(\"规则\")` 是表达式。",
    ),
    (
        "worldborder",
        "世界边界：`worldborder.add/set/center/damage_amount/damage_buffer/warning_distance/warning_time(...)`，`worldborder.get()` 是表达式。",
    ),
    (
        "locate",
        "定位结构、生物群系或兴趣点：`locate.structure/biome/poi(...)`。",
    ),
    (
        "teleport",
        "传送实体：`teleport(持有者, pos(...))` 或 `teleport(持有者, 单个实体查询)`。",
    ),
    (
        "pos",
        "方块坐标构造器：`pos(x, y, z)`，支持 `~` 相对与 `^` 局部坐标。",
    ),
    (
        "block_pos",
        "`pos` 的别名，强调方块坐标（绝对分量是整数）。",
    ),
    (
        "vec3",
        "精确坐标构造器：`vec3(x, y, z)`，绝对分量允许小数，支持 `~` 与 `^`。",
    ),
    (
        "vec2",
        "水平精确坐标构造器：`vec2(x, z)`，绝对分量允许小数，不支持 `^`。",
    ),
    (
        "rotation",
        "朝向构造器：`rotation(yaw, pitch)`，单位是度，用于 `teleport` 的可选朝向。",
    ),
    (
        "column",
        "列坐标构造器：`column(x, z)`，供 forceload 使用。",
    ),
    (
        "block_state",
        "方块状态构造器：`block_state(\"命名空间:方块\") { 属性 = \"值\"; }`。",
    ),
    (
        "nbt",
        "结构化 NBT 数据：`nbt { 键 = 值; }`，支持全部 12 种标签类型；可作 set_block/fill 的方块实体数据、物品的 custom_data，或作为语句合并到当前实体的具名标签。",
    ),
    (
        "text",
        "文本组件：`text(\"...\") { color = \"red\"; }`，样式块支持颜色、五个开关、点击与悬停事件。",
    ),
    (
        "translate",
        "翻译组件：`translate(\"chat.type.text\", [参数组件...])`。",
    ),
    ("keybind", "按键组件：`keybind(\"key.jump\")`。"),
    (
        "selector",
        "选择器组件：`selector(\"@a\")` 或 `selector(查询)`。",
    ),
    (
        "advancement",
        "声明进度或用 `advancement.grant/revoke(...)` 授予、撤销玩家进度。",
    ),
];

/// 函数属性的说明，与 `ATTRIBUTES` 逐项对应。
const ATTRIBUTE_DOCS: &[(&str, &str)] = &[
    ("load", "服务器加载时运行。"),
    ("tick", "每游戏刻运行。"),
    ("entity", "要求任意实体上下文。"),
    ("player", "要求玩家上下文。"),
    ("non_player", "要求非玩家实体上下文。"),
];

/// 补全：根据光标前的字符区分属性、函数标签与普通名称。
pub fn completion(text: &str, offset: usize, path: &Path, symbols: &[Symbol]) -> Value {
    let (prefix, start) = prefix_at(text, offset);
    let preceding = text[..start].chars().next_back();
    let items = match preceding {
        Some('@') => attribute_items(),
        Some('#') => tag_items(symbols),
        // 点号成员（`self.add_tag` 等）暂不提供补全，避免给出错误的方法名。
        Some('.') => Vec::new(),
        _ => name_items(prefix, path, offset, symbols),
    };
    Value::Array(items)
}

/// 悬停：关键词显示中英文对照与说明，声明名称显示声明摘要和位置。
pub fn hover(
    text: &str,
    offset: usize,
    path: &Path,
    symbols: &[Symbol],
    sources: &[SourceFile],
) -> Option<Value> {
    let (word, start, end) = word_at(text, offset)?;
    let range = range_json(text, start, end);
    if let Some(contents) = keyword_hover(&word) {
        return Some(json!({"contents": contents, "range": range}));
    }
    let symbol = find_symbol(symbols, &word, path, offset)?;
    let mut value = format!(
        "**{}** `{}`\n\n```mclang\n{}\n```",
        symbol.kind.label(),
        symbol.name,
        symbol.detail
    );
    if let Some(source) = sources.iter().find(|source| source.path == symbol.path) {
        let (line, _) = offset_to_position(&source.text, symbol.name_span.start);
        value.push_str(&format!(
            "\n\n定义：`{}:{}`",
            symbol.path.display(),
            line + 1
        ));
    }
    Some(json!({
        "contents": {"kind": "markdown", "value": value},
        "range": range,
    }))
}

/// 跳转：光标处的名称跳到它的声明位置，可以跨文件。
pub fn definition(
    text: &str,
    offset: usize,
    path: &Path,
    symbols: &[Symbol],
    sources: &[SourceFile],
) -> Option<Value> {
    let (word, _, _) = word_at(text, offset)?;
    let symbol = find_symbol(symbols, &word, path, offset)?;
    let source = sources.iter().find(|source| source.path == symbol.path)?;
    Some(json!({
        "uri": path_to_uri(&symbol.path),
        "range": range_json(&source.text, symbol.name_span.start, symbol.name_span.end),
    }))
}

fn keyword_hover(word: &str) -> Option<Value> {
    for entry in KEYWORDS.iter().chain(ATTRIBUTES.iter()) {
        let english = entry.english == word;
        let chinese = entry.chinese == word;
        if !english && !chinese {
            continue;
        }
        let docs = if ATTRIBUTES
            .iter()
            .any(|attribute| attribute.english == entry.english)
        {
            ("函数属性", attribute_doc(entry.english))
        } else {
            ("关键词", keyword_doc(entry.english))
        };
        let label = if english {
            format!("`{}` / `{}`", entry.english, entry.chinese)
        } else {
            format!("`{}` / `{}`", entry.chinese, entry.english)
        };
        let text = match docs.1 {
            Some(doc) => format!("**{}** {label}\n\n{doc}", docs.0),
            None => format!("**{}** {label}", docs.0),
        };
        return Some(json!({"kind": "markdown", "value": text}));
    }
    None
}

fn keyword_doc(english: &str) -> Option<&'static str> {
    KEYWORD_DOCS
        .iter()
        .find(|(name, _)| *name == english)
        .map(|(_, doc)| *doc)
}

fn attribute_doc(english: &str) -> Option<&'static str> {
    ATTRIBUTE_DOCS
        .iter()
        .find(|(name, _)| *name == english)
        .map(|(_, doc)| *doc)
}

fn name_items(prefix: &str, path: &Path, offset: usize, symbols: &[Symbol]) -> Vec<Value> {
    let mut items = Vec::new();
    for symbol in visible_symbols(symbols, path, offset) {
        if !symbol.name.starts_with(prefix) {
            continue;
        }
        items.push(json!({
            "label": symbol.name,
            "kind": completion_kind(symbol.kind),
            "detail": symbol.detail,
            "sortText": format!("0{}", symbol.name),
        }));
    }
    for entry in KEYWORDS {
        for (label, alias) in [
            (entry.english, entry.chinese),
            (entry.chinese, entry.english),
        ] {
            if !label.starts_with(prefix) {
                continue;
            }
            items.push(json!({
                "label": label,
                "kind": 14,
                "detail": format!("关键词 · {alias}"),
                "documentation": {"kind": "markdown", "value": keyword_doc(entry.english).unwrap_or_default()},
                "sortText": format!("1{label}"),
            }));
        }
    }
    items
}

fn attribute_items() -> Vec<Value> {
    let mut items = Vec::new();
    for entry in ATTRIBUTES {
        for (label, alias) in [
            (entry.english, entry.chinese),
            (entry.chinese, entry.english),
        ] {
            items.push(json!({
                "label": format!("@{label}"),
                "kind": 14,
                "detail": format!("函数属性 · {alias}"),
                "documentation": {"kind": "markdown", "value": attribute_doc(entry.english).unwrap_or_default()},
                "sortText": format!("2{label}"),
            }));
        }
    }
    items
}

fn tag_items(symbols: &[Symbol]) -> Vec<Value> {
    symbols
        .iter()
        .filter(|symbol| symbol.kind == SymbolKind::FunctionTag)
        .map(|symbol| {
            json!({
                "label": symbol.name,
                "kind": 18,
                "detail": symbol.detail,
                "sortText": format!("0{}", symbol.name),
            })
        })
        .collect()
}

/// 参数与局部变量只在所属函数体内可见；顶层声明处处可见。
fn visible_symbols<'a>(
    symbols: &'a [Symbol],
    path: &Path,
    offset: usize,
) -> impl Iterator<Item = &'a Symbol> {
    symbols.iter().filter(move |symbol| match symbol.scope {
        None => true,
        Some(scope) => symbol.path == path && scope.start <= offset && offset <= scope.end,
    })
}

fn find_symbol<'a>(
    symbols: &'a [Symbol],
    word: &str,
    path: &Path,
    offset: usize,
) -> Option<&'a Symbol> {
    symbols
        .iter()
        .find(|symbol| {
            symbol.name == word
                && symbol.path == path
                && symbol
                    .scope
                    .is_none_or(|scope| scope.start <= offset && offset <= scope.end)
        })
        .or_else(|| {
            symbols
                .iter()
                .find(|symbol| symbol.name == word && symbol.scope.is_none())
        })
}

fn completion_kind(kind: SymbolKind) -> u8 {
    match kind {
        SymbolKind::Function => 3,
        SymbolKind::Score | SymbolKind::Objective | SymbolKind::Parameter | SymbolKind::Local => 6,
        SymbolKind::Query => 18,
        SymbolKind::ItemStack => 12,
        SymbolKind::Storage => 9,
        SymbolKind::DataSlot => 9,
        SymbolKind::Resource => 17,
        SymbolKind::Advancement => 17,
        SymbolKind::FunctionTag => 18,
    }
}

/// 光标前的标识符前缀及其起始偏移；`@`、`#`、`.` 等前缀字符不属于标识符。
fn prefix_at(text: &str, offset: usize) -> (&str, usize) {
    let mut start = offset.min(text.len());
    while start > 0 {
        let previous = match text[..start].chars().next_back() {
            Some(character) => character,
            None => break,
        };
        if previous == '_' || previous.is_alphanumeric() {
            start -= previous.len_utf8();
        } else {
            break;
        }
    }
    (&text[start..offset.min(text.len())], start)
}

fn range_json(text: &str, start: usize, end: usize) -> Value {
    let (start_line, start_character) = offset_to_position(text, start);
    let (end_line, end_character) = offset_to_position(text, end);
    json!({
        "start": {"line": start_line, "character": start_character},
        "end": {"line": end_line, "character": end_character},
    })
}
