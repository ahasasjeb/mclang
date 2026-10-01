//! 翻译改写：逐 token 决定一个词在当前位置是否属于语言词汇。
//!
//! 算法是“把已识别的词改写成目标语言”，输入本来用哪种写法并不重要。改写规则与
//! `docs/tools/translate.mjs` 保持一致：关键词与函数属性出现即翻译（它们是保留字，
//! 不可能是用户标识符），方法、属性与枚举值只在能按位置确定身份时翻译——`@`
//! 之后、`.` 之后、后面跟 `=` 或 `(`、明确的调用实参、`属性 = 值` 的取值位置。
//! 其余位置的词保持原样，宁可少翻，不可错翻用户标识符。
//!
//! 位置规则仍可能撞上用户标识符（物品名 `reward`、查询名 `players` 恰好也是
//! 属性或枚举词），因此可能是用户引用的位置通过 [`Rewriter::alias`] 保护名字。
//! 已明确属于语法的方法、块成员与固定枚举参数通过 [`Rewriter::syntax_alias`] 翻译。
//!
//! `nbt { ... }` 与 `block_state("…") { ... }` 内部是数据而不是语言：前者是用户
//! NBT（只翻译布尔字面量），后者是原版方块状态属性名与取值，整块保持原样。

use std::collections::HashSet;

use super::KeywordLanguage;
use super::context;
use super::syntax::{self, Frame};
use super::tables::Tables;
use super::token::{Neighbors, Token, TokenKind, tokenize};

pub(super) fn translate(
    source: &str,
    tables: &Tables,
    language: KeywordLanguage,
    declared: &HashSet<String>,
) -> String {
    let tokens = tokenize(source);
    let neighbors = Neighbors::new(&tokens);
    let frames = syntax::frames(&tokens, &neighbors, tables, declared);
    let nbt = nbt_body_tokens(&tokens, tables);
    let block_states = block_state_body_tokens(&tokens, &neighbors, tables);
    let imports = import_name_tokens(&tokens, tables);
    let rewriter = Rewriter {
        tables,
        declared,
        language,
    };
    let mut output = String::with_capacity(source.len());
    for (index, token) in tokens.iter().enumerate() {
        if token.kind != TokenKind::Ident {
            output.push_str(token.text);
            continue;
        }
        if imports.contains(&index) {
            output.push_str(token.text);
            continue;
        }
        // NBT 字面量内部只翻译布尔字面量，键名与字符串是用户数据。
        if nbt.contains(&index) {
            let key = neighbors
                .next(index)
                .is_some_and(|next| matches!(tokens[next].text, "=" | ":"));
            match (!key)
                .then(|| rewriter.syntax_alias(token.text, "boolean_word"))
                .flatten()
            {
                Some(boolean) => output.push_str(boolean),
                None => output.push_str(token.text),
            }
            continue;
        }
        // 方块状态的属性名与取值是原版数据（`level`、`facing`……），整块保持原样。
        if block_states.contains(&index) {
            output.push_str(token.text);
            continue;
        }
        match translate_word(&tokens, &neighbors, &frames, index, &rewriter) {
            Some(word) => output.push_str(&word),
            None => output.push_str(token.text),
        }
    }
    output
}

/// 改写入口：已声明标识符不允许参与方法/属性/枚举值改写，避免把
/// `item reward = …` 的 `reward` 或查询名 `players` 当成语言词汇。
struct Rewriter<'a> {
    tables: &'a Tables,
    declared: &'a HashSet<String>,
    language: KeywordLanguage,
}

impl Rewriter<'_> {
    fn alias<'a>(&'a self, word: &'a str, family: &str) -> Option<&'a str> {
        if self.declared.contains(word) {
            return None;
        }
        self.tables.rewrite(word, family, self.language)
    }

    /// 明确的方法、块成员和固定枚举参数是语法，不受同拼写的用户声明影响。
    fn syntax_alias<'a>(&'a self, word: &'a str, family: &str) -> Option<&'a str> {
        self.tables.rewrite(word, family, self.language)
    }

    fn keyword(&self, word: &str) -> Option<&'static str> {
        self.tables.keyword(word, self.language)
    }

    fn attribute(&self, word: &str) -> Option<&'static str> {
        self.tables.attribute(word, self.language)
    }
}

fn translate_word(
    tokens: &[Token],
    neighbors: &Neighbors,
    frames: &[Frame],
    index: usize,
    rewriter: &Rewriter,
) -> Option<String> {
    let word = tokens[index].text;
    let previous = neighbors.previous(index);
    let next = neighbors.next(index);

    if previous.is_some_and(|previous| tokens[previous].text == "@") {
        return rewriter.attribute(word).map(str::to_owned);
    }
    if previous.is_some_and(|previous| tokens[previous].text == "#") {
        return None;
    }
    if let Some(dot) = previous.filter(|dot| tokens[*dot].text == ".") {
        return chain_member(tokens, neighbors, dot, word, rewriter);
    }
    if let Some(family) = frames[index].property_family
        && previous.is_some_and(|previous| matches!(tokens[previous].text, "{" | ";" | "}"))
        && let Some(property) = rewriter.syntax_alias(word, family)
    {
        return Some(property.to_owned());
    }
    if frames[index].execute_modifier {
        return rewriter
            .syntax_alias(word, "execute_clause")
            .map(str::to_owned);
    }
    // 枚举位置优先于关键词：on(origin) 中的 origin 是“起源”，不是“投掷者”。
    if let Some(value) = argument_value(&frames[index], word, rewriter) {
        return Some(value);
    }
    if next.is_some_and(|next| tokens[next].text == "=")
        && let Some(rewritten) = declaration_property(word, rewriter)
    {
        return Some(rewritten);
    }
    if next.is_some_and(|next| tokens[next].text == "(")
        && let Some(rewritten) = call_position(word, rewriter)
    {
        return Some(rewritten);
    }
    if let Some(equals) = previous.filter(|equals| tokens[*equals].text == "=")
        && let Some(rewritten) =
            property_value(tokens, neighbors, equals, &frames[index], word, rewriter)
    {
        return Some(rewritten);
    }
    if let Some(frame) = frames[index].callee.as_deref()
        && let Some(value) = frame_value(frame, word, rewriter)
    {
        return Some(value);
    }

    if let Some(keyword) = rewriter.keyword(word) {
        return Some(keyword.to_owned());
    }
    if let Some(boolean) = rewriter.alias(word, "boolean_word") {
        return Some(boolean.to_owned());
    }
    if let Some(unit) = rewriter.alias(word, "time_unit")
        && next_to_number(tokens, neighbors, index)
    {
        return Some(unit.to_owned());
    }
    None
}

/// `.` 之后的词：链式调用的根决定整条链的身份。根是命令接收者时，每个词都按
/// 命令枚举值处理；否则只看紧邻的接收者是不是方法表。
fn chain_member(
    tokens: &[Token],
    neighbors: &Neighbors,
    dot: usize,
    word: &str,
    rewriter: &Rewriter,
) -> Option<String> {
    let receiver = neighbors.previous(dot);
    let mut root = receiver;
    while let Some(current) = root {
        match neighbors.previous(current) {
            Some(before) if tokens[before].text == "." => root = neighbors.previous(before),
            _ => break,
        }
    }
    let root_word = root.and_then(|root| rewriter.tables.canonical(tokens[root].text));
    if root_word.is_some_and(|root| context::COMMAND_RECEIVERS.contains(&root)) {
        return rewriter
            .syntax_alias(word, "command_value")
            .or_else(|| rewriter.syntax_alias(word, "ui_value"))
            .or_else(|| rewriter.keyword(word))
            .map(str::to_owned);
    }
    if root_word == Some("scoreboard") {
        // scoreboard.objectives.modify.rendertype 的多节命令链使用 command_value。
        let nested = root != receiver
            || rewriter
                .tables
                .canonical_in(word, "scoreboard_group")
                .is_some()
            || rewriter.tables.canonical_in(word, "ui_value") == Some("players");
        if nested {
            let rewritten = if root == receiver {
                rewriter
                    .syntax_alias(word, "scoreboard_group")
                    .or_else(|| rewriter.syntax_alias(word, "ui_value"))
            } else {
                rewriter.syntax_alias(word, "command_value")
            };
            return rewritten.map(str::to_owned);
        }
    }
    if let Some(receiver) = receiver
        && let Some(family) = rewriter.tables.receiver_family(tokens[receiver].text)
        && let Some(rewritten) = rewriter.syntax_alias(word, family)
    {
        return Some(rewritten.to_owned());
    }
    rewriter.keyword(word).map(str::to_owned)
}

/// 后面跟 `=` 的声明属性（`count = 3;`、`准则 = "trigger";`）。
fn declaration_property(word: &str, rewriter: &Rewriter) -> Option<String> {
    context::PROPERTY_FAMILIES
        .iter()
        .find_map(|family| rewriter.alias(word, family))
        .map(str::to_owned)
}

/// 函数调用位置：声明属性和 execute 子句都写在这。关键词表优先于同名属性
/// （`count` 在物品属性里是“数量”，在表达式里是“计数”）。
fn call_position(word: &str, rewriter: &Rewriter) -> Option<String> {
    let canonical = rewriter.tables.canonical(word);
    if rewriter.tables.is_keyword(word, "function") {
        return Some(rewriter.keyword(word).unwrap_or(word).to_owned());
    }
    if canonical.is_some_and(|canonical| {
        context::COMMAND_CALLS.contains(&canonical)
            || context::COMMAND_RECEIVERS.contains(&canonical)
            || context::EXPRESSION_CALLS.contains(&canonical)
    }) {
        let rewritten = rewriter
            .keyword(word)
            .or_else(|| rewriter.alias(word, "command_value"))
            .or_else(|| rewriter.alias(word, "ui_value"));
        if let Some(rewritten) = rewritten {
            return Some(rewritten.to_owned());
        }
    }
    context::PROPERTY_FAMILIES
        .iter()
        .chain(context::CALL_FAMILIES)
        .find_map(|family| rewriter.alias(word, family))
        .map(str::to_owned)
}

/// `属性 = 值` 里的枚举值优先于关键词表：`frame = 目标` 的“目标”是进度框样式，
/// 不是 objective 关键词。
fn property_value(
    tokens: &[Token],
    neighbors: &Neighbors,
    equals: usize,
    frame: &Frame,
    word: &str,
    rewriter: &Rewriter,
) -> Option<String> {
    let property_index = neighbors.previous(equals)?;
    let property_word = tokens[property_index].text;
    let property = rewriter.tables.canonical(property_word)?;
    let syntax_property = frame.property_family.is_some_and(|family| {
        rewriter
            .tables
            .canonical_in(property_word, family)
            .is_some()
    }) && neighbors
        .previous(property_index)
        .is_some_and(|previous| matches!(tokens[previous].text, "{" | ";" | "}"));
    context::property_value_families(property)
        .iter()
        .find_map(|family| {
            if syntax_property {
                rewriter.syntax_alias(word, family)
            } else {
                rewriter.alias(word, family)
            }
        })
        .map(str::to_owned)
}

/// 只在解析器要求枚举的实参位置绕过名字保护，其他实参继续当作用户引用。
fn argument_value(frame: &Frame, word: &str, rewriter: &Rewriter) -> Option<String> {
    let callee = frame.callee.as_deref()?;
    if callee == "bossbar.get" && frame.argument == 1 {
        return rewriter
            .syntax_alias(word, "command_value")
            .or_else(|| rewriter.syntax_alias(word, "ui_value"))
            .map(str::to_owned);
    }
    let compute_kind_argument = if frame.compute_source.as_deref() == Some("default") {
        1
    } else {
        2
    };
    let family = match (callee, frame.argument) {
        ("on", 0) => "entity_relation",
        ("store.result" | "store.success", 2) => "bossbar_field",
        ("compute", 0) => "compute_source",
        ("compute", argument) if argument == compute_kind_argument => "compute_kind",
        ("scoreboard.operation", 2) => "score_operation",
        ("scoreboard.objectives.modify.rendertype", 1) => "render_type",
        ("scoreboard.objectives.modify.numberformat", 1) => "number_format_kind",
        ("scoreboard.players.numberformat" | "scoreboard.players.display.numberformat", 2) => {
            "number_format_kind"
        }
        _ => return None,
    };
    rewriter.syntax_alias(word, family).map(str::to_owned)
}

/// 调用实参里的枚举值：只在明确的取值位置翻译，避免命中同名标识符。
fn frame_value(frame: &str, word: &str, rewriter: &Rewriter) -> Option<String> {
    let root = frame.split('.').next().expect("split 至少返回一个片段");
    if context::COMMAND_RECEIVERS.contains(&root) || context::COMMAND_CALLS.contains(&frame) {
        let ui = if frame.starts_with("bossbar.")
            || frame.starts_with("title.")
            || frame == "particle"
        {
            rewriter.alias(word, "ui_value")
        } else {
            None
        };
        let rewritten = rewriter
            .alias(word, "command_value")
            .or(ui)
            .or_else(|| rewriter.alias(word, "text_color"));
        if let Some(rewritten) = rewritten {
            return Some(rewritten.to_owned());
        }
    }
    context::call_value_families(frame)
        .iter()
        .find_map(|family| rewriter.alias(word, family))
        .map(str::to_owned)
}

/// `t`/`s`/`d` 前面是数字，或前面是逗号且逗号前面是数字（`time.set(6000, t)`）。
fn next_to_number(tokens: &[Token], neighbors: &Neighbors, index: usize) -> bool {
    let Some(previous) = neighbors.previous(index) else {
        return false;
    };
    if tokens[previous].kind == TokenKind::Number {
        return true;
    }
    if tokens[previous].text == "," {
        return neighbors
            .previous(previous)
            .is_some_and(|before| tokens[before].kind == TokenKind::Number);
    }
    false
}

/// `nbt { ... }` 里除开头的 `nbt` 之外全部保持原样：键名与字符串是用户数据，
/// 恰好叫 `count`、`data` 之类的属性名时不能被翻译。
fn nbt_body_tokens(tokens: &[Token], tables: &Tables) -> HashSet<usize> {
    let mut opaque = HashSet::new();
    let mut depth = 0;
    let mut pending = false;
    for (index, token) in tokens.iter().enumerate() {
        if depth > 0 {
            opaque.insert(index);
            match token.text {
                "{" => depth += 1,
                "}" => depth -= 1,
                _ => {}
            }
            continue;
        }
        if matches!(token.kind, TokenKind::Whitespace | TokenKind::Comment) {
            continue;
        }
        if pending && token.text == "{" {
            depth = 1;
            opaque.insert(index);
            pending = false;
            continue;
        }
        pending = token.kind == TokenKind::Ident && tables.is_keyword(token.text, "nbt");
    }
    opaque
}

/// `block_state("minecraft:light") { level = "15"; }` 的属性名与取值是原版数据，
/// 与 `nbt` 字面量一样整块保持原样；只有 `block_state` 关键词本身参与翻译。
fn block_state_body_tokens(
    tokens: &[Token],
    neighbors: &Neighbors,
    tables: &Tables,
) -> HashSet<usize> {
    let mut opaque = HashSet::new();
    for index in 0..tokens.len() {
        if tokens[index].kind != TokenKind::Ident
            || !tables.is_keyword(tokens[index].text, "block_state")
        {
            continue;
        }
        let Some(open) = neighbors
            .next(index)
            .filter(|next| tokens[*next].text == "(")
        else {
            continue;
        };
        let mut depth = 0;
        let mut close = None;
        for (cursor, token) in tokens.iter().enumerate().skip(open + 1) {
            match token.text {
                "(" => depth += 1,
                ")" if depth == 0 => {
                    close = Some(cursor);
                    break;
                }
                ")" => depth -= 1,
                _ => {}
            }
        }
        let Some(brace) = close
            .and_then(|close| neighbors.next(close))
            .filter(|next| tokens[*next].text == "{")
        else {
            continue;
        };
        let mut depth = 0;
        let mut cursor = brace;
        while cursor < tokens.len() {
            opaque.insert(cursor);
            match tokens[cursor].text {
                "{" => depth += 1,
                "}" => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                _ => {}
            }
            cursor += 1;
        }
    }
    opaque
}

/// 导入路径与导出名是标识符，即使拼写恰好是 `time`、`random` 这样的关键词也不
/// 翻译；只有 `import` 与别名 `as` 是语法。
fn import_name_tokens(tokens: &[Token], tables: &Tables) -> HashSet<usize> {
    let mut opaque = HashSet::new();
    let mut in_import = false;
    for (index, token) in tokens.iter().enumerate() {
        if !in_import {
            if token.kind == TokenKind::Ident && tables.canonical(token.text) == Some("import") {
                in_import = true;
            }
            continue;
        }
        if token.text == ";" {
            in_import = false;
        } else if token.kind == TokenKind::Ident && tables.canonical(token.text) != Some("as") {
            opaque.insert(index);
        }
    }
    opaque
}
