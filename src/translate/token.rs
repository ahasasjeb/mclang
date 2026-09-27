//! 翻译用的轻量词法扫描：保留空白与注释，只切出标识符、字符串、数字与标点。
//!
//! 与 `docs/tools/translate.mjs` 的 `tokenize` 保持一致：三引号字符串、双引号
//! 字符串、`//` 注释、空白、带 NBT 后缀的数字、Unicode 标识符和单字符标点。
//! 翻译只改写标识符 token，其余 token 原样保留。

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum TokenKind {
    Ident,
    Text,
    Comment,
    Whitespace,
    Number,
    Punct,
}

pub(super) struct Token {
    pub(super) kind: TokenKind,
    pub(super) text: String,
}

pub(super) fn tokenize(source: &str) -> Vec<Token> {
    let chars: Vec<char> = source.chars().collect();
    let mut tokens = Vec::new();
    let mut index = 0;
    while index < chars.len() {
        if chars.get(index) == Some(&'"')
            && chars.get(index + 1) == Some(&'"')
            && chars.get(index + 2) == Some(&'"')
        {
            let mut stop = index + 3;
            let mut closed = false;
            while stop + 3 <= chars.len() {
                if chars[stop] == '"' && chars[stop + 1] == '"' && chars[stop + 2] == '"' {
                    closed = true;
                    break;
                }
                stop += 1;
            }
            let end = if closed { stop + 3 } else { chars.len() };
            tokens.push(Token {
                kind: TokenKind::Text,
                text: collect(&chars, index, end),
            });
            index = end;
            continue;
        }
        if chars[index] == '"' {
            let mut stop = index + 1;
            while stop < chars.len() && chars[stop] != '"' && chars[stop] != '\n' {
                if chars[stop] == '\\' {
                    stop += 1;
                }
                stop += 1;
            }
            if stop < chars.len() && chars[stop] == '"' {
                stop += 1;
            }
            tokens.push(Token {
                kind: TokenKind::Text,
                text: collect(&chars, index, stop),
            });
            index = stop;
            continue;
        }
        if chars[index] == '/' && chars.get(index + 1) == Some(&'/') {
            let mut stop = index;
            while stop < chars.len() && chars[stop] != '\n' {
                stop += 1;
            }
            tokens.push(Token {
                kind: TokenKind::Comment,
                text: collect(&chars, index, stop),
            });
            index = stop;
            continue;
        }
        if chars[index].is_whitespace() {
            let mut stop = index;
            while stop < chars.len() && chars[stop].is_whitespace() {
                stop += 1;
            }
            tokens.push(Token {
                kind: TokenKind::Whitespace,
                text: collect(&chars, index, stop),
            });
            index = stop;
            continue;
        }
        if chars[index].is_ascii_digit() {
            let stop = number_end(&chars, index);
            tokens.push(Token {
                kind: TokenKind::Number,
                text: collect(&chars, index, stop),
            });
            index = stop;
            continue;
        }
        if is_ident_start(chars[index]) {
            let mut stop = index + 1;
            while stop < chars.len() && is_ident_continue(chars[stop]) {
                stop += 1;
            }
            tokens.push(Token {
                kind: TokenKind::Ident,
                text: collect(&chars, index, stop),
            });
            index = stop;
            continue;
        }
        tokens.push(Token {
            kind: TokenKind::Punct,
            text: chars[index].to_string(),
        });
        index += 1;
    }
    tokens
}

fn collect(chars: &[char], start: usize, end: usize) -> String {
    chars[start..end].iter().collect()
}

fn is_ident_start(character: char) -> bool {
    character.is_alphabetic() || character == '_'
}

fn is_ident_continue(character: char) -> bool {
    character.is_alphabetic() || character.is_numeric() || character == '_'
}

/// 数字与 NBT 后缀一起扫描：`1b`、`2s`、`3i`、`4L`、`5.5f`、`6d`。后缀之后
/// 紧跟标识符字符时按普通数字处理（`1bytes` = `1` + `bytes`）。
fn number_end(chars: &[char], start: usize) -> usize {
    let mut index = start;
    while index < chars.len() && chars[index].is_ascii_digit() {
        index += 1;
    }
    if chars.get(index) == Some(&'.') && chars.get(index + 1).is_some_and(char::is_ascii_digit) {
        index += 1;
        while index < chars.len() && chars[index].is_ascii_digit() {
            index += 1;
        }
    }
    if let Some(&suffix) = chars.get(index)
        && matches!(
            suffix,
            'b' | 'B' | 's' | 'S' | 'i' | 'I' | 'l' | 'L' | 'f' | 'F' | 'd' | 'D'
        )
    {
        let blocked = chars
            .get(index + 1)
            .is_some_and(|character| is_ident_continue(*character));
        if !blocked {
            index += 1;
        }
    }
    index
}

/// 每个 token 前后最近的非空白 token 索引。注释也算有效邻居，与文档工具的
/// `previousSignificant` / `nextSignificant` 一致。
pub(super) struct Neighbors {
    previous: Vec<Option<usize>>,
    next: Vec<Option<usize>>,
}

impl Neighbors {
    pub(super) fn new(tokens: &[Token]) -> Self {
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
