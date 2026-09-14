use crate::ast::Span;
use crate::diagnostic::Diagnostic;

#[derive(Clone, Debug, PartialEq)]
pub enum TokenKind {
    Ident(String),
    Number(i64),
    String(String),
    At,
    LeftParen,
    RightParen,
    LeftBrace,
    RightBrace,
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
                '(' => self.single(TokenKind::LeftParen),
                ')' => self.single(TokenKind::RightParen),
                '{' => self.single(TokenKind::LeftBrace),
                '}' => self.single(TokenKind::RightBrace),
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
        let text = &self.source[start..self.cursor];
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lexes_command_strings_and_operators() {
        let tokens = lex("run \"say \\\"hi\\\"\"; value += 2;", 0).unwrap();
        assert!(matches!(&tokens[1].kind, TokenKind::String(value) if value == "say \"hi\""));
        assert!(
            tokens
                .iter()
                .any(|token| token.kind == TokenKind::PlusEqual)
        );
    }

    #[test]
    fn reports_unterminated_string() {
        assert!(lex("run \"oops", 0).is_err());
    }

    #[test]
    fn lexes_chinese_keywords_as_words() {
        let tokens = lex("命名空间 demo; @每刻 函数 tick() {}", 0).unwrap();
        assert!(matches!(&tokens[0].kind, TokenKind::Ident(value) if value == "命名空间"));
        assert!(matches!(&tokens[4].kind, TokenKind::Ident(value) if value == "每刻"));
        assert!(matches!(&tokens[5].kind, TokenKind::Ident(value) if value == "函数"));
    }
}
