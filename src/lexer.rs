use crate::ast::Span;
use crate::diagnostic::Diagnostic;

#[derive(Clone, Debug, PartialEq)]
pub enum TokenKind {
    Ident(String),
    Number(i64),
    Decimal(f64),
    /// `1b`：TAG_Byte 字面量，解析期检查 -128 到 127。
    Byte(i64),
    /// `1s`：TAG_Short 字面量，解析期检查 -32768 到 32767。
    Short(i64),
    /// `1L`：TAG_Long 字面量。
    Long(i64),
    /// `1f` 或 `1.5f`：TAG_Float 字面量。
    Float(f64),
    /// `1d` 或 `1.5d`：TAG_Double 字面量。
    Double(f64),
    String(String),
    At,
    Hash,
    Tilde,
    Caret,
    LeftParen,
    RightParen,
    LeftBrace,
    RightBrace,
    LeftBracket,
    RightBracket,
    Semicolon,
    Comma,
    Dot,
    Equal,
    EqualEqual,
    Bang,
    BangEqual,
    AndAnd,
    OrOr,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    Plus,
    PlusEqual,
    Minus,
    MinusEqual,
    Arrow,
    Star,
    StarEqual,
    Slash,
    SlashEqual,
    Percent,
    PercentEqual,
    Eof,
}

#[derive(Clone, Debug)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

pub fn lex(source: &str, source_id: usize) -> Result<Vec<Token>, Vec<Diagnostic>> {
    let mut lexer = Lexer {
        source,
        source_id,
        cursor: 0,
        tokens: Vec::new(),
        diagnostics: Vec::new(),
    };
    lexer.scan();
    if lexer.diagnostics.is_empty() {
        Ok(lexer.tokens)
    } else {
        Err(lexer.diagnostics)
    }
}

struct Lexer<'a> {
    source: &'a str,
    source_id: usize,
    cursor: usize,
    tokens: Vec<Token>,
    diagnostics: Vec<Diagnostic>,
}

impl Lexer<'_> {
    fn scan(&mut self) {
        while let Some(character) = self.peek() {
            let start = self.cursor;
            match character {
                c if c.is_whitespace() => {
                    self.advance();
                }
                '/' if self.peek_second() == Some('/') => {
                    while !matches!(self.peek(), None | Some('\n')) {
                        self.advance();
                    }
                }
                c if identifier_start(c) => self.identifier(start),
                '0'..='9' => self.number(start),
                '"' if self.source[self.cursor..].starts_with("\"\"\"") => self.raw_string(start),
                '"' => self.string(start),
                '@' => self.single(TokenKind::At),
                '#' => self.single(TokenKind::Hash),
                '~' => self.single(TokenKind::Tilde),
                '^' => self.single(TokenKind::Caret),
                '(' => self.single(TokenKind::LeftParen),
                ')' => self.single(TokenKind::RightParen),
                '{' => self.single(TokenKind::LeftBrace),
                '}' => self.single(TokenKind::RightBrace),
                '[' => self.single(TokenKind::LeftBracket),
                ']' => self.single(TokenKind::RightBracket),
                ';' => self.single(TokenKind::Semicolon),
                ',' => self.single(TokenKind::Comma),
                '.' => self.single(TokenKind::Dot),
                '=' => self.one_or_two(TokenKind::Equal, '=', TokenKind::EqualEqual),
                '!' => self.one_or_two(TokenKind::Bang, '=', TokenKind::BangEqual),
                '&' if self.peek_second() == Some('&') => {
                    self.one_or_two(TokenKind::AndAnd, '&', TokenKind::AndAnd)
                }
                '|' if self.peek_second() == Some('|') => {
                    self.one_or_two(TokenKind::OrOr, '|', TokenKind::OrOr)
                }
                '<' => self.one_or_two(TokenKind::Less, '=', TokenKind::LessEqual),
                '>' => self.one_or_two(TokenKind::Greater, '=', TokenKind::GreaterEqual),
                '+' => self.one_or_two(TokenKind::Plus, '=', TokenKind::PlusEqual),
                '-' if self.peek_second() == Some('>') => {
                    self.one_or_two(TokenKind::Arrow, '>', TokenKind::Arrow)
                }
                '-' => self.one_or_two(TokenKind::Minus, '=', TokenKind::MinusEqual),
                '*' => self.one_or_two(TokenKind::Star, '=', TokenKind::StarEqual),
                '/' => self.one_or_two(TokenKind::Slash, '=', TokenKind::SlashEqual),
                '%' => self.one_or_two(TokenKind::Percent, '=', TokenKind::PercentEqual),
                _ => {
                    self.advance();
                    self.diagnostics.push(Diagnostic::new(
                        format!("无法识别字符 `{character}`"),
                        Span {
                            source: self.source_id,
                            start,
                            end: self.cursor,
                        },
                    ));
                }
            }
        }
        self.tokens.push(Token {
            kind: TokenKind::Eof,
            span: Span {
                source: self.source_id,
                start: self.cursor,
                end: self.cursor,
            },
        });
    }

    fn identifier(&mut self, start: usize) {
        self.advance();
        while self.peek().is_some_and(identifier_continue) {
            self.advance();
        }
        self.tokens.push(Token {
            kind: TokenKind::Ident(self.source[start..self.cursor].to_owned()),
            span: Span {
                source: self.source_id,
                start,
                end: self.cursor,
            },
        });
    }

    fn number(&mut self, start: usize) {
        while matches!(self.peek(), Some('0'..='9')) {
            self.advance();
        }
        // 小数只在点号后紧跟数字时成立，`self.remove()` 之类的成员访问不受影响。
        let mut decimal = false;
        if self.peek() == Some('.') && self.peek_second().is_some_and(|c| c.is_ascii_digit()) {
            self.advance();
            while matches!(self.peek(), Some('0'..='9')) {
                self.advance();
            }
            decimal = true;
        }
        // NBT 数值后缀：`1b`、`1s`、`1i`、`1L`、`1.5f`、`1d` 等。只有紧贴数字、
        // 且后面不再跟标识符字符时才成立，因此 `1bytes` 会照常拆成数字与标识符。
        if let Some(suffix) = self.number_suffix() {
            self.advance();
            let span = Span {
                source: self.source_id,
                start,
                end: self.cursor,
            };
            let digits = &self.source[start..self.cursor - suffix.len_utf8()];
            match suffix.to_ascii_lowercase() {
                suffix @ ('b' | 's' | 'i' | 'l') => {
                    if decimal {
                        self.diagnostics
                            .push(Diagnostic::new("小数只能使用 f 或 d 后缀", span));
                        return;
                    }
                    match digits.parse::<i64>() {
                        Ok(value) => {
                            let kind = match suffix {
                                'b' => TokenKind::Byte(value),
                                's' => TokenKind::Short(value),
                                'i' => TokenKind::Number(value),
                                _ => TokenKind::Long(value),
                            };
                            self.tokens.push(Token { kind, span });
                        }
                        Err(_) => self
                            .diagnostics
                            .push(Diagnostic::new("数字超出支持范围", span)),
                    }
                }
                suffix @ ('f' | 'd') => {
                    let single = suffix == 'f';
                    match digits.parse::<f64>() {
                        Ok(value)
                            if value.is_finite() && (!single || (value as f32).is_finite()) =>
                        {
                            let kind = if single {
                                TokenKind::Float(value)
                            } else {
                                TokenKind::Double(value)
                            };
                            self.tokens.push(Token { kind, span });
                        }
                        _ => {
                            let message = if single {
                                "单精度浮点超出范围"
                            } else {
                                "小数超出支持范围"
                            };
                            self.diagnostics.push(Diagnostic::new(message, span));
                        }
                    }
                }
                _ => unreachable!("number_suffix 只返回 b、s、i、l、f、d"),
            }
            return;
        }
        let text = &self.source[start..self.cursor];
        if decimal {
            return match text.parse::<f64>() {
                Ok(value) if value.is_finite() => self.tokens.push(Token {
                    kind: TokenKind::Decimal(value),
                    span: Span {
                        source: self.source_id,
                        start,
                        end: self.cursor,
                    },
                }),
                _ => self.diagnostics.push(Diagnostic::new(
                    "小数超出支持范围",
                    Span {
                        source: self.source_id,
                        start,
                        end: self.cursor,
                    },
                )),
            };
        }
        match text.parse::<i64>() {
            Ok(value) => self.tokens.push(Token {
                kind: TokenKind::Number(value),
                span: Span {
                    source: self.source_id,
                    start,
                    end: self.cursor,
                },
            }),
            Err(_) => self.diagnostics.push(Diagnostic::new(
                "整数超出支持范围",
                Span {
                    source: self.source_id,
                    start,
                    end: self.cursor,
                },
            )),
        }
    }

    /// 紧跟数字的单个 NBT 后缀字母；后面还有标识符字符时返回 `None`。
    fn number_suffix(&self) -> Option<char> {
        let suffix = self.peek()?;
        if !matches!(
            suffix,
            'b' | 'B' | 's' | 'S' | 'i' | 'I' | 'l' | 'L' | 'f' | 'F' | 'd' | 'D'
        ) {
            return None;
        }
        let mut following = self.source[self.cursor..].chars();
        following.next();
        if following.next().is_some_and(identifier_continue) {
            return None;
        }
        Some(suffix)
    }

    fn string(&mut self, start: usize) {
        self.advance();
        let mut value = String::new();
        let mut terminated = false;
        while let Some(character) = self.peek() {
            match character {
                '"' => {
                    self.advance();
                    terminated = true;
                    break;
                }
                '\n' | '\r' => break,
                '\\' => {
                    self.advance();
                    let Some(escaped) = self.peek() else { break };
                    self.advance();
                    match escaped {
                        '"' => value.push('"'),
                        '\\' => value.push('\\'),
                        'n' => value.push('\n'),
                        'r' => value.push('\r'),
                        't' => value.push('\t'),
                        _ => self.diagnostics.push(Diagnostic::new(
                            format!("不支持转义 `\\{escaped}`"),
                            Span {
                                source: self.source_id,
                                start: self.cursor - escaped.len_utf8() - 1,
                                end: self.cursor,
                            },
                        )),
                    }
                }
                _ => {
                    value.push(character);
                    self.advance();
                }
            }
        }
        if terminated {
            self.tokens.push(Token {
                kind: TokenKind::String(value),
                span: Span {
                    source: self.source_id,
                    start,
                    end: self.cursor,
                },
            });
        } else {
            self.diagnostics.push(Diagnostic::new(
                "字符串没有结束引号",
                Span {
                    source: self.source_id,
                    start,
                    end: self.cursor,
                },
            ));
        }
    }

    fn raw_string(&mut self, start: usize) {
        self.advance();
        self.advance();
        self.advance();
        let contents_start = self.cursor;
        let Some(offset) = self.source[self.cursor..].find("\"\"\"") else {
            self.cursor = self.source.len();
            self.diagnostics.push(Diagnostic::new(
                "跨行原始字符串缺少结束的 `\"\"\"`",
                Span {
                    source: self.source_id,
                    start,
                    end: self.cursor,
                },
            ));
            return;
        };
        let contents_end = self.cursor + offset;
        let value = self.source[contents_start..contents_end].to_owned();
        self.cursor = contents_end + 3;
        self.tokens.push(Token {
            kind: TokenKind::String(value),
            span: Span {
                source: self.source_id,
                start,
                end: self.cursor,
            },
        });
    }

    fn single(&mut self, kind: TokenKind) {
        let start = self.cursor;
        self.advance();
        self.tokens.push(Token {
            kind,
            span: Span {
                source: self.source_id,
                start,
                end: self.cursor,
            },
        });
    }

    fn one_or_two(&mut self, one: TokenKind, expected: char, two: TokenKind) {
        let start = self.cursor;
        self.advance();
        let kind = if self.peek() == Some(expected) {
            self.advance();
            two
        } else {
            one
        };
        self.tokens.push(Token {
            kind,
            span: Span {
                source: self.source_id,
                start,
                end: self.cursor,
            },
        });
    }

    fn peek(&self) -> Option<char> {
        self.source[self.cursor..].chars().next()
    }

    fn peek_second(&self) -> Option<char> {
        let mut chars = self.source[self.cursor..].chars();
        chars.next();
        chars.next()
    }

    fn advance(&mut self) {
        if let Some(character) = self.peek() {
            self.cursor += character.len_utf8();
        }
    }
}

fn identifier_start(character: char) -> bool {
    character == '_' || character.is_alphabetic()
}

fn identifier_continue(character: char) -> bool {
    character == '_' || character.is_alphanumeric()
}
