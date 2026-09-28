//! 翻译用的轻量词法扫描：保留空白与注释，只切出标识符、字符串、数字与标点。
//!
//! 与 `docs/tools/translate.mjs` 的 `tokenize` 保持一致：三引号字符串、双引号
//! 字符串、`//` 注释、空白、带 NBT 后缀的数字、Unicode 标识符和单字符标点。
//! 翻译只改写标识符 token，其余 token 原样保留。
//!
//! token 直接借用原始 UTF-8 源码的字节范围，不为每个 token 单独分配字符串；
//! 只有真正被改写的词才在输出里产生新字符串。

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum TokenKind {
    Ident,
    Text,
    Comment,
    Whitespace,
    Number,
    Punct,
}

pub(super) struct Token<'a> {
    pub(super) kind: TokenKind,
    pub(super) text: &'a str,
}

pub(super) fn tokenize(source: &str) -> Vec<Token<'_>> {
    let mut tokens = Vec::new();
    let mut index = 0usize;
    while index < source.len() {
        let rest = &source[index..];
        let current = rest.chars().next().unwrap_or_default();
        let length = if rest.starts_with("\"\"\"") {
            raw_string_length(rest)
        } else if current == '"' {
            string_length(rest)
        } else if rest.starts_with("//") {
            rest.find('\n').unwrap_or(rest.len())
        } else if current.is_whitespace() {
            whitespace_length(rest)
        } else if current.is_ascii_digit() {
            number_length(rest)
        } else if is_ident_start(current) {
            ident_length(rest)
        } else {
            current.len_utf8()
        };
        let kind = if rest.starts_with("//") {
            TokenKind::Comment
        } else if rest.starts_with("\"") {
            TokenKind::Text
        } else if current.is_whitespace() {
            TokenKind::Whitespace
        } else if current.is_ascii_digit() {
            TokenKind::Number
        } else if is_ident_start(current) {
            TokenKind::Ident
        } else {
            TokenKind::Punct
        };
        tokens.push(Token {
            kind,
            text: &rest[..length],
        });
        index += length;
    }
    tokens
}

/// 三引号原始字符串；没有结束标记时取到文件末尾。
fn raw_string_length(rest: &str) -> usize {
    match rest[3..].find("\"\"\"") {
        Some(offset) => 3 + offset + 3,
        None => rest.len(),
    }
}

/// 双引号字符串；`\` 跳过紧随其后的字符，结束引号或换行收尾。
fn string_length(rest: &str) -> usize {
    let mut characters = rest[1..].char_indices();
    while let Some((index, character)) = characters.next() {
        if character == '"' || character == '\n' {
            return 1 + index + character.len_utf8();
        }
        if character == '\\' {
            // 转义字符本身不参与闭合判断；再消费一个字符。
            characters.next();
        }
    }
    rest.len()
}

fn whitespace_length(rest: &str) -> usize {
    rest.char_indices()
        .find(|(_, character)| !character.is_whitespace())
        .map_or(rest.len(), |(index, _)| index)
}

fn ident_length(rest: &str) -> usize {
    rest.char_indices()
        .find(|(_, character)| !is_ident_continue(*character))
        .map_or(rest.len(), |(index, _)| index)
}

fn is_ident_start(character: char) -> bool {
    character.is_alphabetic() || character == '_'
}

fn is_ident_continue(character: char) -> bool {
    character.is_alphabetic() || character.is_numeric() || character == '_'
}

/// 数字与 NBT 后缀一起扫描：`1b`、`2s`、`3i`、`4L`、`5.5f`、`6d`。后缀之后
/// 紧跟标识符字符时按普通数字处理（`1bytes` = `1` + `bytes`）。
fn number_length(rest: &str) -> usize {
    let mut offset = 0usize;
    let digits = rest.len() - rest.trim_start_matches(|c: char| c.is_ascii_digit()).len();
    offset += digits;
    let tail = &rest[offset..];
    if tail.starts_with('.') && tail[1..].starts_with(|c: char| c.is_ascii_digit()) {
        offset += 1
            + (tail[1..].len()
                - tail[1..]
                    .trim_start_matches(|c: char| c.is_ascii_digit())
                    .len());
    }
    let tail = &rest[offset..];
    if let Some(suffix) = tail.chars().next()
        && matches!(
            suffix,
            'b' | 'B' | 's' | 'S' | 'i' | 'I' | 'l' | 'L' | 'f' | 'F' | 'd' | 'D'
        )
    {
        let blocked = tail[suffix.len_utf8()..]
            .chars()
            .next()
            .is_some_and(is_ident_continue);
        if !blocked {
            offset += suffix.len_utf8();
        }
    }
    offset
}

/// 每个 token 前后最近的非空白 token 索引。注释也算有效邻居，与文档工具的
/// `previousSignificant` / `nextSignificant` 一致。
pub(super) struct Neighbors {
    previous: Vec<Option<usize>>,
    next: Vec<Option<usize>>,
}

impl Neighbors {
    pub(super) fn new(tokens: &[Token<'_>]) -> Self {
        let mut previous = vec![None; tokens.len()];
        let mut last = None;
        for (index, token) in tokens.iter().enumerate() {
            previous[index] = last;
            if token.kind != TokenKind::Whitespace {
                last = Some(index);
            }
        }
        let mut next = vec![None; tokens.len()];
        let mut upcoming = None;
        for (index, token) in tokens.iter().enumerate().rev() {
            next[index] = upcoming;
            if token.kind != TokenKind::Whitespace {
                upcoming = Some(index);
            }
        }
        Self { previous, next }
    }

    pub(super) fn previous(&self, index: usize) -> Option<usize> {
        self.previous[index]
    }

    pub(super) fn next(&self, index: usize) -> Option<usize> {
        self.next[index]
    }
}
