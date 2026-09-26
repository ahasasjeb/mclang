//! 递归下降解析器：把词法单元转换为带源范围的 AST。
//!
//! 入口是 [`parse`]。语法产生式按区域拆分成子模块，每个子模块用自己的
//! `impl Parser` 块实现：
//!
//! - [`declarations`]：命名空间、计分、实体查询、物品、存储、资源和函数声明；
//! - [`statements`]：函数体语句；
//! - [`conditions`]：布尔条件和比较；
//! - [`execute`]：结构化 `execute` 子句；
//! - [`expressions`]：算术表达式与实参列表；
//! - [`keywords`]：中英文关键词与枚举值的规范化表。
//!
//! 本模块只保留 [`Parser`] 的游标导航和顶层声明分派。子模块定义的语法产生式
//! 若需要跨模块调用，统一使用 `pub(super)`。

mod components;
mod conditions;
mod core_commands;
mod declarations;
mod entity_commands;
mod execute;
mod expressions;
mod item_predicates;
mod items;
pub(crate) mod keywords;
mod macros;
mod nbt;
mod statements;
mod world;

use crate::ast::*;
use crate::diagnostic::Diagnostic;
use crate::lexer::{Token, TokenKind};

use keywords::{keyword_alias, word_matches};

pub fn parse(tokens: Vec<Token>) -> Result<Program, Vec<Diagnostic>> {
    let paren_matches = matching_parens(&tokens);
    Parser {
        tokens,
        cursor: 0,
        paren_matches,
        active_macro_parameters: std::collections::HashMap::new(),
        active_macro_uses: Vec::new(),
        active_macro_coordinates: Vec::new(),
    }
    .program()
    .map_err(|error| vec![error])
}

fn matching_parens(tokens: &[Token]) -> Vec<Option<usize>> {
    let mut matches = vec![None; tokens.len()];
    let mut stack = Vec::new();
    for (index, token) in tokens.iter().enumerate() {
        match &token.kind {
            TokenKind::LeftParen => stack.push(index),
            TokenKind::RightParen => {
                if let Some(open) = stack.pop() {
                    matches[open] = Some(index);
                    matches[index] = Some(open);
                }
            }
            _ => {}
        }
    }
    matches
}

struct Parser {
    tokens: Vec<Token>,
    cursor: usize,
    /// 每个圆括号记号对应的配对位置；解析条件分组时可 O(1) 判断后继记号。
    paren_matches: Vec<Option<usize>>,
    active_macro_parameters: std::collections::HashMap<String, MacroType>,
    active_macro_uses: Vec<(String, Span)>,
    active_macro_coordinates: Vec<(String, MacroCoordinateKind)>,
}

impl Parser {
    fn program(&mut self) -> Result<Program, Diagnostic> {
        // 命名空间只在入口模块必需；其他模块可以省略，写了则必须与入口一致。
        let mut namespace = String::new();
        let mut namespace_span = None;
        if self.check_word("namespace") {
            let start = self.advance().span;
            let (name, span) = self.ident("命名空间名称")?;
            self.expect(TokenKind::Semicolon, "命名空间声明后需要 `;`")?;
            namespace = name;
            namespace_span = Some(start.merge(span));
        }

        let mut imports = Vec::new();
        let mut scores = Vec::new();
        let mut objectives = Vec::new();
        let mut queries = Vec::new();
        let mut item_stacks = Vec::new();
        let mut storages = Vec::new();
        let mut data_slots = Vec::new();
        let mut resources = Vec::new();
        let mut advancements = Vec::new();
        let mut function_tags = Vec::new();
        let mut functions = Vec::new();
        while !self.check(&TokenKind::Eof) {
            if self.check_word("import") {
                imports.push(self.import()?);
                continue;
            }
            let exported = self.take_word("export").is_some();
            if self.check_word("score") {
                let mut declaration = self.score()?;
                declaration.exported = exported;
                scores.push(declaration);
            } else if self.check_word("objective") {
                let mut declaration = self.objective()?;
                declaration.exported = exported;
                objectives.push(declaration);
            } else if self.check_word("query") {
                let mut declaration = self.query()?;
                declaration.exported = exported;
                queries.push(declaration);
            } else if self.check_word("item") {
                let mut declaration = self.item_stack()?;
                declaration.exported = exported;
                item_stacks.push(declaration);
            } else if self.check_word("storage") {
                let mut declaration = self.storage()?;
                declaration.exported = exported;
                storages.push(declaration);
            } else if self.check_word("data_slot") {
                let mut declaration = self.data_slot()?;
                declaration.exported = exported;
                data_slots.push(declaration);
            } else if self.check_word("resource") {
                let mut declaration = self.resource()?;
                declaration.exported = exported;
                resources.push(declaration);
            } else if self.check_word("advancement") {
                let mut declaration = self.advancement()?;
                declaration.exported = exported;
                advancements.push(declaration);
            } else if self.check_word("fn_tag") {
                let mut declaration = self.function_tag()?;
                declaration.exported = exported;
                function_tags.push(declaration);
            } else {
                let mut function = self.function()?;
                function.exported = exported;
                functions.push(function);
            }
        }
        Ok(Program {
            namespace,
            namespace_span,
            imports,
            scores,
            objectives,
            queries,
            item_stacks,
            storages,
            data_slots,
            resources,
            advancements,
            function_tags,
            functions,
        })
    }

    /// `import 数学::几何;` 或 `import 数学::{加法, 减法 as 减};`
    ///
    /// 路径相对项目根目录；`items` 为 `None` 时导入整个模块的公开声明。
    fn import(&mut self) -> Result<ImportDecl, Diagnostic> {
        self.expect_word("import")?;
        let (first, first_span) = self.ident("模块名称")?;
        let mut path = vec![first];
        let mut path_span = first_span;
        let mut items = None;
        loop {
            if self.take(&TokenKind::ColonColon).is_none() {
                break;
            }
            if self.take(&TokenKind::LeftBrace).is_some() {
                items = Some(self.import_items()?);
                break;
            }
            let (segment, span) = self.ident("模块路径分段")?;
            path_span = path_span.merge(span);
            path.push(segment);
        }
        self.expect(TokenKind::Semicolon, "import 声明后需要 `;`")?;
        Ok(ImportDecl {
            path,
            path_span,
            items,
        })
    }

    fn import_items(&mut self) -> Result<Vec<ImportItem>, Diagnostic> {
        let mut items = Vec::new();
        loop {
            let (name, name_span) = self.ident("导入的名称")?;
            let mut alias = None;
            if self.check_word("as") {
                self.advance();
                let (value, _) = self.ident("导入别名")?;
                alias = Some(value);
            }
            items.push(ImportItem {
                name,
                name_span,
                alias,
            });
            if self.take(&TokenKind::Comma).is_none() {
                break;
            }
        }
        self.expect(TokenKind::RightBrace, "导入列表缺少 `}`")?;
        Ok(items)
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

    fn message_argument(&mut self, label: &str) -> Result<MessageArgument, Diagnostic> {
        let (text, span) = self.string(&format!("{label}需要单行消息字符串"))?;
        Ok(MessageArgument { text, span })
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

    /// 向前看 `offset` 个词法单元；越界时返回文件末尾的 `Eof`。
    fn peek_kind(&self, offset: usize) -> &Token {
        self.tokens
            .get(self.cursor + offset)
            .unwrap_or_else(|| self.tokens.last().expect("词法单元列表一定以 Eof 结尾"))
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
