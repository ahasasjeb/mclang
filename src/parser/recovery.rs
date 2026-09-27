//! 语法错误后的同步与上下文回滚；恢复得到的部分 AST 不会送入编译器。

use super::Parser;
use super::keywords::word_matches;
use crate::lexer::TokenKind;

pub(super) struct RecoveryContext {
    runtime_loop_depth: usize,
    runtime_loop_variables: usize,
    constant_bindings: usize,
    constant_variables: usize,
    macro_uses: usize,
    macro_coordinates: usize,
}

impl Parser {
    pub(super) fn recovery_context(&self) -> RecoveryContext {
        RecoveryContext {
            runtime_loop_depth: self.runtime_loop_depth,
            runtime_loop_variables: self.runtime_loop_variables.len(),
            constant_bindings: self.constant_bindings.len(),
            constant_variables: self.constant_variables.len(),
            macro_uses: self.active_macro_uses.len(),
            macro_coordinates: self.active_macro_coordinates.len(),
        }
    }

    pub(super) fn restore_context(&mut self, context: RecoveryContext) {
        self.runtime_loop_depth = context.runtime_loop_depth;
        self.runtime_loop_variables
            .truncate(context.runtime_loop_variables);
        self.constant_bindings.truncate(context.constant_bindings);
        self.constant_variables.truncate(context.constant_variables);
        self.active_macro_uses.truncate(context.macro_uses);
        self.active_macro_coordinates
            .truncate(context.macro_coordinates);
    }

    pub(super) fn clear_function_context(&mut self) {
        self.runtime_loop_depth = 0;
        self.runtime_loop_variables.clear();
        self.constant_bindings.clear();
        self.constant_variables.clear();
        self.function_parameters.clear();
        self.active_macro_parameters.clear();
        self.active_macro_uses.clear();
        self.active_macro_coordinates.clear();
    }

    pub(super) fn function_starts(&self) -> bool {
        self.function_starts_at(self.cursor)
    }

    fn function_starts_at(&self, index: usize) -> bool {
        let kind = &self.tokens[index].kind;
        matches!(kind, TokenKind::At)
            || matches!(kind, TokenKind::Ident(word)
                if ["fn", "macro", "export"].iter().any(|key| word_matches(word, key)))
    }

    fn declaration_starts_at(&self, index: usize) -> bool {
        self.function_starts_at(index)
            || matches!(&self.tokens[index].kind, TokenKind::Ident(word)
                if ["namespace", "import", "score", "objective", "query", "item",
                    "storage", "data_slot", "resource", "advancement", "fn_tag"]
                    .iter().any(|key| word_matches(word, key)))
    }

    fn statement_starts_at(&self, index: usize) -> bool {
        matches!(&self.tokens[index].kind, TokenKind::Ident(word)
            if ["let", "if", "while", "for", "unroll", "each", "in_dimension", "spawn",
                "execute", "return", "break", "continue", "run"]
                .iter().any(|key| word_matches(word, key)))
    }

    /// 从失败产生式的起点扫描，避免将嵌套块内部的分号当作外层边界。
    /// 分号允许跨过未闭合的圆括号；右花括号留给当前 block 消费。
    pub(super) fn synchronize(&mut self, start: usize, in_block: bool) {
        let failed_at = self.cursor;
        let mut braces = 0usize;
        let mut index = start;
        while index < self.tokens.len() {
            let kind = &self.tokens[index].kind;
            if matches!(kind, TokenKind::Eof) {
                break;
            }
            if index > start
                && index >= failed_at
                && (self.function_starts_at(index)
                    || (braces == 0
                        && if in_block {
                            self.statement_starts_at(index)
                        } else {
                            self.declaration_starts_at(index)
                        }))
            {
                break;
            }
            match kind {
                TokenKind::LeftBrace => braces += 1,
                TokenKind::RightBrace if braces == 0 => {
                    if !in_block {
                        index += 1;
                    }
                    break;
                }
                TokenKind::RightBrace => {
                    braces -= 1;
                    if braces == 0 && index + 1 >= failed_at {
                        index += 1;
                        break;
                    }
                }
                TokenKind::Semicolon if braces == 0 && index + 1 >= failed_at => {
                    index += 1;
                    break;
                }
                _ => {}
            }
            index += 1;
        }
        self.cursor = index.min(self.tokens.len() - 1);
    }
}
