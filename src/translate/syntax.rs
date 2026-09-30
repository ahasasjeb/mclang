//! 翻译需要的浅层语境。只看分隔符与声明头，即使源码尚未写完也能工作。

use std::collections::HashSet;

use super::tables::Tables;
use super::token::{Neighbors, Token, TokenKind};

#[derive(Clone, Default)]
pub(super) struct Frame {
    pub callee: Option<String>,
    pub argument: usize,
    pub compute_source: Option<String>,
    pub property_family: Option<&'static str>,
    pub execute_modifier: bool,
}

pub(super) fn frames(
    tokens: &[Token],
    neighbors: &Neighbors,
    tables: &Tables,
    declared: &HashSet<String>,
) -> Vec<Frame> {
    let mut result = vec![Frame::default(); tokens.len()];
    let mut stack: Vec<(&str, usize, Frame)> = Vec::new();
    let mut opens = vec![None; tokens.len()];
    for (index, token) in tokens.iter().enumerate() {
        result[index] = stack
            .last()
            .map(|(_, _, frame)| frame.clone())
            .unwrap_or_default();
        if token.kind == TokenKind::Ident {
            result[index].execute_modifier =
                tables.canonical_in(token.text, "execute_clause").is_some()
                    && is_execute_modifier(tokens, neighbors, &opens, index, tables);
        }
        if token.kind != TokenKind::Punct {
            continue;
        }
        match token.text {
            "(" => {
                let mut callee = canonical_callee(tokens, neighbors, index, tables);
                if let Some(name) = neighbors.previous(index) {
                    let before = neighbors.previous(name);
                    let declaration =
                        before.is_some_and(|before| tables.is_keyword(tokens[before].text, "fn"));
                    let property = result[name].property_family.is_some_and(|family| {
                        tables.canonical_in(tokens[name].text, family).is_some()
                    }) && before
                        .is_some_and(|before| matches!(tokens[before].text, "{" | ";" | "}"));
                    // `on`、`sort` 等不是保留字，普通函数及其参数不能借用语法的枚举表。
                    // execute 子句和声明块成员仍按所在位置识别，不受同名函数影响。
                    if declaration
                        || (declared.contains(tokens[name].text)
                            && !before.is_some_and(|before| tokens[before].text == ".")
                            && !property
                            && !result[name].execute_modifier)
                    {
                        callee = None;
                    }
                }
                let compute_source = neighbors
                    .next(index)
                    .and_then(|first| tables.canonical_in(tokens[first].text, "compute_source"))
                    .map(str::to_owned);
                stack.push((
                    "(",
                    index,
                    Frame {
                        callee,
                        compute_source,
                        ..Frame::default()
                    },
                ));
            }
            "{" => {
                let parent = stack.last().and_then(|(_, _, frame)| frame.property_family);
                let property_family =
                    block_family(tokens, neighbors, &opens, index, parent, tables);
                stack.push((
                    "{",
                    index,
                    Frame {
                        property_family,
                        ..Frame::default()
                    },
                ));
            }
            "[" => stack.push(("[", index, Frame::default())),
            ")" | "}" | "]" => {
                let expected = match token.text {
                    ")" => "(",
                    "}" => "{",
                    _ => "[",
                };
                if stack
                    .last()
                    .is_some_and(|(delimiter, _, _)| *delimiter == expected)
                {
                    opens[index] = stack.pop().map(|(_, open, _)| open);
                }
            }
            "," => {
                if let Some(("(", _, frame)) = stack.last_mut() {
                    frame.argument += 1;
                }
            }
            _ => {}
        }
    }
    result
}

/// execute 的修饰符位于条件之前；向前跳过完整的修饰符调用即可识别子句头，
/// 而 `execute if on(value) == 1` 中的普通函数调用不会被误认。
fn is_execute_modifier(
    tokens: &[Token],
    neighbors: &Neighbors,
    opens: &[Option<usize>],
    mut index: usize,
    tables: &Tables,
) -> bool {
    while let Some(previous) = neighbors.previous(index) {
        if tables.is_keyword(tokens[previous].text, "execute") {
            return true;
        }
        if tokens[previous].text != ")" {
            return false;
        }
        let Some(name) = opens[previous].and_then(|open| neighbors.previous(open)) else {
            return false;
        };
        if tables
            .canonical_in(tokens[name].text, "execute_clause")
            .is_none()
        {
            return false;
        }
        index = name;
    }
    false
}

fn block_family(
    tokens: &[Token],
    neighbors: &Neighbors,
    opens: &[Option<usize>],
    brace: usize,
    parent: Option<&str>,
    tables: &Tables,
) -> Option<&'static str> {
    let previous = neighbors.previous(brace)?;
    if tokens[previous].text == ")" {
        let open = opens[previous]?;
        let callee = canonical_callee(tokens, neighbors, open, tables)?;
        return match callee.as_str() {
            "item_stack" => Some("item_stack_property"),
            "entity" => {
                // 只有 `query name = entity(...)` 打开查询块；条件 `if entity(...)`
                // 后面的块是普通语句，里面的 limit 等名字仍是用户变量或函数。
                let name = neighbors.previous(open)?;
                let equals = neighbors.previous(name)?;
                let query_name = neighbors.previous(equals)?;
                let declaration = neighbors.previous(query_name)?;
                (tokens[equals].text == "="
                    && tokens[query_name].kind == TokenKind::Ident
                    && tables.is_keyword(tokens[declaration].text, "query"))
                .then_some("query_property")
            }
            "text" | "translate" | "keybind" | "object" | "score" | "selector" | "nbt" => {
                Some("text_style_property")
            }
            _ => None,
        };
    }
    // 从本成员的起点读取声明头；不能让外层声明的表泄漏到普通代码块。
    let mut start = previous;
    while let Some(before) = neighbors.previous(start) {
        if matches!(tokens[before].text, ";" | "{" | "}") {
            break;
        }
        start = before;
    }
    if parent == Some("advancement_property") {
        return match tables.canonical_in(tokens[start].text, "advancement_property")? {
            "criterion" => Some("criterion_property"),
            "reward" => Some("reward_property"),
            "display" => Some("display_property"),
            _ => None,
        };
    }
    if tables.is_keyword(tokens[start].text, "export") {
        start = neighbors.next(start)?;
    }
    match tables.canonical(tokens[start].text)? {
        "advancement" => Some("advancement_property"),
        "objective" => Some("objective_property"),
        "fn_tag" => Some("function_tag_property"),
        _ => None,
    }
}

/// 链中的每一节都根据接收者选表，避免“数字格式”等同形词落入属性表。
pub(super) fn canonical_callee(
    tokens: &[Token],
    neighbors: &Neighbors,
    open: usize,
    tables: &Tables,
) -> Option<String> {
    let method = neighbors.previous(open)?;
    if tokens[method].kind != TokenKind::Ident {
        return None;
    }
    let mut parts = vec![tokens[method].text];
    let mut current = method;
    while let Some(dot) = neighbors.previous(current) {
        if tokens[dot].text != "." {
            break;
        }
        let receiver = neighbors.previous(dot)?;
        if tokens[receiver].kind != TokenKind::Ident {
            break;
        }
        parts.insert(0, tokens[receiver].text);
        current = receiver;
    }
    let head = parts[0];
    let root = tables.canonical(head).unwrap_or(head);
    let family = tables.receiver_family(head);
    let mut canonical = vec![root];
    for (index, part) in parts.iter().enumerate().skip(1) {
        let method = if root == "scoreboard"
            && (parts.len() > 2
                || matches!(*part, "objectives" | "目标集" | "players" | "玩家分数"))
        {
            if index == 1 {
                tables.canonical_in(part, "scoreboard_group")
            } else {
                tables.canonical_in(part, "command_value")
            }
        } else if index == 1 {
            family.and_then(|family| tables.canonical_in(part, family))
        } else {
            None
        };
        canonical.push(
            method
                .or_else(|| tables.canonical_in(part, "command_value"))
                .or_else(|| tables.canonical(part))
                .unwrap_or(part),
        );
    }
    Some(canonical.join("."))
}

/// 解析失败时也保护已经写出的声明和参数。保守收集名字，不猜测引用的含义。
pub(super) fn declared_names(tokens: &[Token], neighbors: &Neighbors) -> HashSet<String> {
    use crate::parser::keywords::word_matches;

    let mut names = HashSet::new();
    for (index, token) in tokens.iter().enumerate() {
        if token.kind != TokenKind::Ident {
            continue;
        }
        let Some(mut name) = neighbors.next(index) else {
            continue;
        };
        let declaration = [
            "score",
            "objective",
            "query",
            "item",
            "storage",
            "data_slot",
            "advancement",
            "fn_tag",
            "fn",
            "let",
            "for",
        ]
        .iter()
        .any(|word| word_matches(token.text, word));
        let resource = word_matches(token.text, "resource");
        let criterion = matches!(token.text, "criterion" | "准则");
        if !declaration && !resource && !criterion {
            continue;
        }
        if resource {
            let Some(next) = neighbors.next(name) else {
                continue;
            };
            name = next;
        }
        if tokens[name].kind != TokenKind::Ident {
            continue;
        }
        names.insert(tokens[name].text.to_owned());
        if !word_matches(token.text, "fn") {
            continue;
        }
        let Some(open) = neighbors
            .next(name)
            .filter(|next| tokens[*next].text == "(")
        else {
            continue;
        };
        let mut cursor = neighbors.next(open);
        let mut parameter_start = true;
        while let Some(index) = cursor {
            if matches!(tokens[index].text, ")" | "{" | ";") {
                break;
            }
            if parameter_start && tokens[index].kind == TokenKind::Ident {
                names.insert(tokens[index].text.to_owned());
            }
            parameter_start = tokens[index].text == ",";
            cursor = neighbors.next(index);
        }
    }
    names
}
