//! 递归下降解析器：把词法单元转换为带源范围的 AST。
//!
//! 入口是 [`parse`]。语法产生式按区域拆分成子模块，每个子模块用自己的
//! `impl Parser` 块实现：
//!
//! - [`declarations`]：命名空间、计分、实体查询、物品、存储、资源和函数声明；
//! - [`statements`]：函数体语句；
//! - [`conditions`]：布尔条件和比较；
//! - [`expressions`]：算术表达式与实参列表；
//! - [`keywords`]：中英文关键词与枚举值的规范化表。
//!
//! 本模块只保留 [`Parser`] 的游标导航和顶层声明分派。子模块定义的语法产生式
//! 若需要跨模块调用，统一使用 `pub(super)`。

mod conditions;
mod declarations;
mod expressions;
mod items;
pub(crate) mod keywords;
mod statements;

#[cfg(test)]
mod tests;

use crate::ast::*;
use crate::diagnostic::Diagnostic;
use crate::lexer::{Token, TokenKind};

use keywords::{keyword_alias, word_matches};

pub fn parse(tokens: Vec<Token>) -> Result<Program, Vec<Diagnostic>> {
    Parser { tokens, cursor: 0 }
        .program()
        .map_err(|error| vec![error])
}

struct Parser {
    tokens: Vec<Token>,
    cursor: usize,
}

impl Parser {
    fn program(&mut self) -> Result<Program, Diagnostic> {
        self.expect_word("namespace")?;
        let (namespace, namespace_span) = self.ident("命名空间名称")?;
        self.expect(TokenKind::Semicolon, "命名空间声明后需要 `;`")?;

        let mut scores = Vec::new();
        let mut queries = Vec::new();
        let mut item_stacks = Vec::new();
        let mut storages = Vec::new();
        let mut resources = Vec::new();
        let mut functions = Vec::new();
        while !self.check(&TokenKind::Eof) {
            if self.check_word("score") {
                scores.push(self.score()?);
            } else if self.check_word("query") {
                queries.push(self.query()?);
            } else if self.check_word("item") {
                item_stacks.push(self.item_stack()?);
            } else if self.check_word("storage") {
                storages.push(self.storage()?);
            } else if self.check_word("resource") {
                resources.push(self.resource()?);
            } else {
                functions.push(self.function()?);
            }
        }
        Ok(Program {
            namespace,
            namespace_span,
            scores,
            queries,
            item_stacks,
            storages,
            resources,
            functions,
        })
    }

    fn ident(&mut self, expected: &str) -> Result<(String, Span), Diagnostic> {
        let token = self.advance().clone();
        match token.kind {
            TokenKind::Ident(value) => Ok((value, token.span)),
            _ => Err(Diagnostic::new(format!("这里需要{expected}"), token.span)),
        }
    }

    fn string(&mut self, message: &str) -> Result<(String, Span), Diagnostic> {
        let token = self.advance().clone();
        match token.kind {
            TokenKind::String(value) => Ok((value, token.span)),
            _ => Err(Diagnostic::new(message, token.span)),
        }
    }

    fn expect_word(&mut self, word: &str) -> Result<Token, Diagnostic> {
        if self.check_word(word) {
            Ok(self.advance().clone())
        } else {
            let expected = keyword_alias(word)
                .map(|alias| format!("`{word}` 或 `{alias}`"))
                .unwrap_or_else(|| format!("`{word}`"));
            Err(Diagnostic::new(
                format!("这里需要 {expected}"),
                self.current().span,
            ))
        }
    }

    fn take_word(&mut self, word: &str) -> Option<Token> {
        if self.check_word(word) {
            Some(self.advance().clone())
        } else {
            None
        }
    }

    fn check_word(&self, word: &str) -> bool {
        matches!(&self.current().kind, TokenKind::Ident(value) if word_matches(value, word))
    }

    fn expect(&mut self, kind: TokenKind, message: &str) -> Result<Token, Diagnostic> {
        if self.check(&kind) {
            Ok(self.advance().clone())
        } else {
            Err(Diagnostic::new(message, self.current().span))
        }
    }

    fn take(&mut self, kind: &TokenKind) -> Option<Token> {
        if self.check(kind) {
            Some(self.advance().clone())
        } else {
            None
        }
    }

    fn check(&self, kind: &TokenKind) -> bool {
        std::mem::discriminant(&self.current().kind) == std::mem::discriminant(kind)
    }

    fn current(&self) -> &Token {
        &self.tokens[self.cursor]
    }

    fn previous(&self) -> &Token {
        &self.tokens[self.cursor - 1]
    }

    fn advance(&mut self) -> &Token {
        let index = self.cursor;
        if !self.check(&TokenKind::Eof) {
            self.cursor += 1;
        }
        &self.tokens[index]
    }
}
