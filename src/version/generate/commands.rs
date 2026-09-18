use std::collections::BTreeMap;
use std::path::Path;

use serde_json::{Map, Value, json};

use super::SOURCE_DIR;
use super::extract::{Extractor, java_files, render_json};
use super::text::{first_string, is_identifier_byte, paren_range};

/// 提取 `commands.json`：26.3 的 Brigadier 命令树。
///
/// 反编译源码里的注册表达式是 `dispatcher.register(...)` 上的一串
/// `Commands.literal("…")` / `Commands.argument("…", 类型)` 与 `.then(...)`、
/// `.requires(...)`、`.redirect(...)` 调用。生成器只解析这一子集，
/// 记录根命令、字面量子命令、参数类型与 `requires` 权限等级。
pub fn generate_commands(root: &Path) -> Result<String, String> {
    let mut extractor = Extractor::new(root);
    let mut commands: BTreeMap<String, Value> = BTreeMap::new();
    let mut root_count = 0usize;
    for directory in [
        "net/minecraft/server/commands",
        "net/minecraft/server/commands/data",
        "net/minecraft/server/commands/item",
        "net/minecraft/gametest/framework",
        "net/minecraft/commands",
    ] {
        let path = root.join(directory);
        if !path.is_dir() {
            continue;
        }
        for file in java_files(&path)? {
            let relative = file
                .strip_prefix(root)
                .map_err(|error| error.to_string())?
                .to_string_lossy()
                .replace('\\', "/");
            let text = extractor.read(&relative)?;
            for node in parse_command_file(&text) {
                root_count += 1;
                commands.insert(node.name.clone(), node.to_value());
            }
        }
    }
    if root_count == 0 {
        return Err("没有解析出任何命令注册，命令树提取逻辑需要更新".into());
    }

    let mut root_object = Map::new();
    root_object.insert("source".into(), json!(SOURCE_DIR));
    root_object.insert("digest".into(), json!(extractor.finish()));
    root_object.insert("root_count".into(), json!(root_count));
    root_object.insert(
        "commands".into(),
        Value::Object(commands.into_iter().collect()),
    );
    render_json(&Value::Object(root_object))
}

/// 命令树节点。
#[derive(Clone, Debug)]
pub(super) struct CommandNode {
    pub(super) name: String,
    /// 参数节点的类型表达式；字面量节点为 `None`。
    pub(super) argument: Option<String>,
    /// `requires` 声明的最低权限等级（0–4）。
    pub(super) level: Option<u8>,
    executable: bool,
    redirect: bool,
    pub(super) children: Vec<CommandNode>,
}

impl CommandNode {
    pub(super) fn to_value(&self) -> Value {
        let mut object = Map::new();
        object.insert("name".into(), json!(self.name));
        if let Some(argument) = &self.argument {
            object.insert("argument".into(), json!(argument));
        }
        if let Some(level) = self.level {
            object.insert("level".into(), json!(level));
        }
        if self.executable {
            object.insert("executable".into(), json!(true));
        }
        if self.redirect {
            object.insert("redirect".into(), json!(true));
        }
        if !self.children.is_empty() {
            object.insert(
                "children".into(),
                Value::Array(self.children.iter().map(CommandNode::to_value).collect()),
            );
        }
        Value::Object(object)
    }
}

/// 解析一个 Java 文件里所有 `dispatcher.register(...)` 调用。
pub(super) fn parse_command_file(text: &str) -> Vec<CommandNode> {
    let variables = variable_literals(text);
    let mut roots = Vec::new();
    let mut search = 0;
    while let Some(index) = text[search..].find(".register(") {
        let dot = search + index;
        search = dot + 1;
        let receiver_start = text[..dot]
            .rfind(|character: char| {
                !(character.is_ascii_alphanumeric() || character == '_' || character == '.')
            })
            .map(|position| position + 1)
            .unwrap_or(0);
        let receiver = &text[receiver_start..dot];
        if !receiver.to_ascii_lowercase().ends_with("dispatcher") {
            continue;
        }
        let open = dot + ".register".len();
        let Some((inner, _)) = paren_range(text, open) else {
            continue;
        };
        if let Some((node, _)) = parse_node(inner, 0) {
            roots.push(node);
        } else if let Some(node) = fallback_node(inner, &variables) {
            // `dispatcher.register(variable)` 或 `register(helper(variable, ...))`：
            // 反查变量初始化里的字面量名，再从注册表达式补出子命令。
            roots.push(node);
        } else if let Some(node) = first_literal_node(inner) {
            // 注册表达式被辅助函数包住（`addTargets(Commands.literal("loot"), …)`）。
            roots.push(node);
        }
    }
    roots
}

/// 注册表达式里第一个 `Commands.literal(...)` 节点。
pub(super) fn first_literal_node(argument: &str) -> Option<CommandNode> {
    let index = argument.find("Commands.literal(")?;
    parse_node(argument, index).map(|(node, _)| node)
}

/// 反编译代码常把根节点存进局部变量；这里收集 `变量 = Commands.literal("…")`。
///
/// 变量之间还会互相转手（`b = (LiteralArgumentBuilder) a.then(...)`），
/// 因此再做一轮传递闭包解析：`b` 引用已解析的 `a` 时沿用 `a` 的节点。
pub(super) fn variable_literals(text: &str) -> BTreeMap<String, CommandNode> {
    let mut variables = BTreeMap::new();
    let mut assignments: BTreeMap<String, String> = BTreeMap::new();
    let mut search = 0;
    while let Some(index) = text[search..].find('=') {
        let equal = search + index;
        search = equal + 1;
        if text[equal..].starts_with("==") {
            continue;
        }
        let before = text[..equal].trim_end();
        if before
            .chars()
            .last()
            .is_some_and(|character| matches!(character, '!' | '<' | '>' | '='))
        {
            continue;
        }
        let name: Vec<char> = before
            .chars()
            .rev()
            .take_while(|character| character.is_ascii_alphanumeric() || *character == '_')
            .collect();
        let name: String = name.into_iter().rev().collect();
        if name.is_empty() {
            continue;
        }
        let rest = &text[equal + 1..];
        if let Some((node, _)) = parse_node(rest.trim_start(), 0) {
            variables.entry(name).or_insert(node);
            continue;
        }
        let end = rest.find([';', '\n']).unwrap_or(rest.len());
        assignments
            .entry(name)
            .or_insert_with(|| rest[..end].trim().to_string());
    }
    // 传递闭包：变量转手时沿用最先引用到的已解析变量。
    loop {
        let mut progressed = false;
        for (name, rhs) in &assignments {
            if variables.contains_key(name) {
                continue;
            }
            let best = variables
                .iter()
                .filter_map(|(candidate, node)| {
                    identifier_position(rhs, candidate).map(|position| (position, node))
                })
                .min_by_key(|(position, _)| *position);
            if let Some((_, node)) = best {
                let node = node.clone();
                variables.insert(name.clone(), node);
                progressed = true;
            }
        }
        if !progressed {
            break;
        }
    }
    variables
}

/// 变量注册时的兜底：用变量名查回字面量，再从注册表达式补子命令与权限。
pub(super) fn fallback_node(
    argument: &str,
    variables: &BTreeMap<String, CommandNode>,
) -> Option<CommandNode> {
    let mut best: Option<(usize, CommandNode)> = None;
    for (name, node) in variables {
        if let Some(position) = identifier_position(argument, name)
            && best
                .as_ref()
                .is_none_or(|(best_position, _)| position < *best_position)
        {
            best = Some((position, node.clone()));
        }
    }
    let (_, mut node) = best?;
    node.level = permission_level(argument).or(node.level);
    if argument.contains(".executes(") {
        node.executable = true;
    }
    if argument.contains(".redirect(") {
        node.redirect = true;
    }
    // 注册表达式里出现的字面量都属于这棵子树；作为一级子命令补入。
    let mut search = 0;
    while let Some(index) = argument[search..].find("Commands.literal(") {
        let start = search + index;
        search = start + 1;
        if let Some((child, _)) = parse_node(argument, start)
            && !node
                .children
                .iter()
                .any(|existing| existing.name == child.name)
        {
            node.children.push(child);
        }
    }
    Some(node)
}

/// `word` 作为独立标识符在 `text` 中第一次出现的位置。
pub(super) fn identifier_position(text: &str, word: &str) -> Option<usize> {
    let bytes = text.as_bytes();
    let mut search = 0;
    while let Some(index) = text[search..].find(word) {
        let start = search + index;
        let end = start + word.len();
        let before_ok = start == 0 || !is_identifier_byte(bytes[start - 1]);
        let after_ok = end >= bytes.len() || !is_identifier_byte(bytes[end]);
        if before_ok && after_ok {
            return Some(start);
        }
        search = end;
    }
    None
}

/// 解析一个构建器链：`(cast) Commands.literal("x").requires(...).then(...)`。
pub(super) fn parse_node(text: &str, mut position: usize) -> Option<(CommandNode, usize)> {
    // 跳过 `(LiteralArgumentBuilder)` 之类的强制转换与空白。
    'skip: loop {
        while position < text.len() && text.as_bytes()[position].is_ascii_whitespace() {
            position += 1;
        }
        let rest = &text[position..];
        // 连续多个左括号：`((LiteralArgumentBuilder) ...`。
        let mut cursor = 0;
        while rest.as_bytes().get(cursor) == Some(&b'(') {
            cursor += 1;
        }
        let candidate = rest[cursor..].trim_start();
        if cursor > 0
            && [
                "LiteralArgumentBuilder)",
                "RequiredArgumentBuilder)",
                "ArgumentBuilder)",
            ]
            .iter()
            .any(|cast| candidate.starts_with(cast))
        {
            position += cursor + (rest[cursor..].len() - candidate.len());
            continue 'skip;
        }
        for cast in [
            "LiteralArgumentBuilder",
            "RequiredArgumentBuilder",
            "ArgumentBuilder",
        ] {
            if let Some(after_cast) = rest.strip_prefix(cast)
                && (after_cast.trim_start().starts_with(')')
                    || after_cast.trim_start().starts_with('<'))
                && let Some(close) = rest.find(')')
            {
                position += close + 1;
                continue 'skip;
            }
        }
        break;
    }

    let rest = &text[position..];
    let (name, argument, after) = if let Some(open) =
        ["Commands.literal", "LiteralArgumentBuilder.literal"]
            .iter()
            .find_map(|call| call_open(rest, call))
    {
        let (inner, after) = paren_range(text, position + open)?;
        (first_string(inner)?, None, after)
    } else {
        let open = ["Commands.argument", "RequiredArgumentBuilder.argument"]
            .iter()
            .find_map(|call| call_open(rest, call))?;
        let (inner, after) = paren_range(text, position + open)?;
        let name = first_string(inner)?;
        let argument = inner
            .split_once(',')
            .map(|(_, rest)| rest.trim().to_string())
            .filter(|rest| !rest.is_empty());
        (name, argument, after)
    };

    let mut node = CommandNode {
        name,
        argument,
        level: None,
        executable: false,
        redirect: false,
        children: Vec::new(),
    };
    let mut position = after;
    loop {
        let rest = &text[position..];
        let trimmed = rest.trim_start();
        // 反编译代码会用 `((LiteralArgumentBuilder) 节点链)` 包住子表达式，
        // 越过这些包裹用的右括号后可能还有 `.then(...)`。
        if trimmed.starts_with(')') {
            let mut cursor = position + (rest.len() - trimmed.len());
            while text.as_bytes().get(cursor) == Some(&b')') {
                cursor += 1;
            }
            position = cursor;
            continue;
        }
        if !trimmed.starts_with('.') {
            break;
        }
        let offset = rest.len() - trimmed.len();
        let after_dot = &trimmed[1..];
        let ident_end = after_dot
            .find(|character: char| !(character.is_ascii_alphanumeric() || character == '_'))
            .unwrap_or(after_dot.len());
        let method = &after_dot[..ident_end];
        let mut open = position + offset + 1 + ident_end;
        while open < text.len() && text.as_bytes()[open].is_ascii_whitespace() {
            open += 1;
        }
        if text.as_bytes().get(open) != Some(&b'(') {
            break;
        }
        let (inner, after_call) = paren_range(text, open)?;
        match method {
            "then" => {
                if let Some((child, _)) = parse_node(inner, 0) {
                    node.children.push(child);
                }
            }
            "requires" => node.level = permission_level(inner),
            "executes" => node.executable = true,
            "redirect" => node.redirect = true,
            _ => {}
        }
        position = after_call;
    }
    Some((node, position))
}

/// `text` 是否以 `call(` 开头；返回左括号的位置。
pub(super) fn call_open(text: &str, call: &str) -> Option<usize> {
    text.strip_prefix(call)?;
    let mut offset = call.len();
    while offset < text.len() && text.as_bytes()[offset].is_ascii_whitespace() {
        offset += 1;
    }
    if text.as_bytes().get(offset) == Some(&b'(') {
        Some(offset)
    } else {
        None
    }
}

/// 从 `requires` 表达式里取权限等级。
pub(super) fn permission_level(text: &str) -> Option<u8> {
    if text.contains("LEVEL_OWNERS") {
        Some(4)
    } else if text.contains("LEVEL_ADMINS") {
        Some(3)
    } else if text.contains("LEVEL_GAMEMASTERS") {
        Some(2)
    } else if text.contains("LEVEL_MODERATORS") {
        Some(1)
    } else if text.contains("LEVEL_ALL") {
        Some(0)
    } else {
        None
    }
}
