//! 结构化 NBT 字面量：`nbt { ... }` 复合标签与全部 12 种标签类型。
//!
//! 语法沿用语言的块风格：复合写成 `{ 键 = 值; }`，列表写成 `[值, 值]`，
//! 数组写成 `[B; 1b, 2b]`、`[I; 1, 2]`、`[L; 1L, 2L]`。数值用后缀区分标签
//! 类型：`b` 字节、`s` 短整数、`i`/无后缀整数、`L` 长整数、`f` 单精度、`d`
//! 或无后缀小数双精度（后缀大小写皆可，与 26.3 SNBT 一致）。`true`/`false`
//! 按 SNBT 规则生成字节标签 1/0。
//!
//! 解析期完成全部静态检查：数值范围、浮点有限性、数组元素后缀、重复键；
//! 进入 AST 的 [`NbtValue`] 一定可以序列化为原版能解析的 SNBT。

use std::collections::HashSet;

use crate::ast::*;
use crate::diagnostic::Diagnostic;
use crate::lexer::TokenKind;
use crate::version::entity_nbt;

use super::Parser;
use super::keywords::boolean_word;

/// 三种整数数组对元素后缀的接受范围，对应 26.3 `SnbtGrammar.ArrayPrefix`。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ArrayKind {
    Byte,
    Int,
    Long,
}

impl ArrayKind {
    /// 元素后缀是否合法：字节数组只收字节，整数数组多收字节与短整数，
    /// 长整数数组再收整数；`i` 是无后缀整数的规范化写法。
    fn allows(self, suffix: char) -> bool {
        match self {
            Self::Byte => matches!(suffix, 'b' | 'i'),
            Self::Int => matches!(suffix, 'b' | 's' | 'i'),
            Self::Long => matches!(suffix, 'b' | 's' | 'i' | 'l'),
        }
    }

    /// 数组的类型名，用于诊断。
    fn label(self) -> &'static str {
        match self {
            Self::Byte => "字节数组",
            Self::Int => "整数数组",
            Self::Long => "长整数数组",
        }
    }

    fn prefix(self) -> &'static str {
        match self {
            Self::Byte => "B",
            Self::Int => "I",
            Self::Long => "L",
        }
    }

    /// 元素按后缀或数组默认类型检查范围：无后缀元素在长整数数组里就是长整数，
    /// 在字节数组里必须落在字节范围内，避免输出时被静默截断。
    fn element_range(self, suffix: char) -> (i64, i64, &'static str) {
        match suffix {
            'b' => (i64::from(i8::MIN), i64::from(i8::MAX), "字节元素"),
            's' => (i64::from(i16::MIN), i64::from(i16::MAX), "短整数元素"),
            'l' => (i64::MIN, i64::MAX, "长整数元素"),
            _ => match self {
                Self::Byte => (i64::from(i8::MIN), i64::from(i8::MAX), "字节元素"),
                Self::Int => (i64::from(i32::MIN), i64::from(i32::MAX), "整数元素"),
                Self::Long => (i64::MIN, i64::MAX, "长整数元素"),
            },
        }
    }
}

impl Parser {
    /// `nbt { 键 = 值; ... }`：命令参数位置使用的复合标签。
    pub(super) fn nbt_compound(&mut self, label: &str) -> Result<NbtValue, Diagnostic> {
        let Some(start) = self.take_word("nbt") else {
            return Err(Diagnostic::new(
                format!("{label}需要 `nbt {{ ... }}`（中文 `数据 {{ ... }}`）"),
                self.current().span,
            ));
        };
        self.nbt_compound_body(start.span, false)
    }

    /// `nbt { ... }`：允许顶层键写中文别名的位置（实体语句与方块实体数据）。
    ///
    /// 物品 `custom_data` 的键是用户数据，改用 [`Parser::nbt_compound`]。
    pub(super) fn nbt_compound_with_aliases(
        &mut self,
        label: &str,
    ) -> Result<NbtValue, Diagnostic> {
        let Some(start) = self.take_word("nbt") else {
            return Err(Diagnostic::new(
                format!("{label}需要 `nbt {{ ... }}`（中文 `数据 {{ ... }}`）"),
                self.current().span,
            ));
        };
        self.nbt_compound_body(start.span, true)
    }

    /// `{ 键 = 值; ... }`：`nbt` 关键字之后与嵌套复合共用。
    ///
    /// `canonical_keys` 只在实体 `nbt` 语句的顶层为真：那里的键是原版具名标签，
    /// 中文别名（`无AI` → `NoAI`）在读取时归一化，重复检查也按英文键进行；
    /// 嵌套复合与物品/方块 NBT 的键是用户数据，保持原样。
    fn nbt_compound_body(
        &mut self,
        start: Span,
        canonical_keys: bool,
    ) -> Result<NbtValue, Diagnostic> {
        self.expect(TokenKind::LeftBrace, "NBT 复合后需要 `{`")?;
        let mut entries: Vec<NbtEntry> = Vec::new();
        let mut seen_keys = HashSet::new();
        while !self.check(&TokenKind::RightBrace) {
            if self.check(&TokenKind::Eof) {
                return Err(Diagnostic::new(
                    "NBT 复合缺少 `}`",
                    start.merge(self.current().span),
                ));
            }
            let (key, key_span) = if matches!(self.current().kind, TokenKind::String(_)) {
                self.string("NBT 键")?
            } else {
                self.ident("NBT 键（标识符或字符串）")?
            };
            let key = if canonical_keys {
                entity_nbt::chinese_alias(&key)
                    .map(str::to_owned)
                    .unwrap_or(key)
            } else {
                key
            };
            self.expect(TokenKind::Equal, "NBT 键后需要 `=`")?;
            let value = self.nbt_value()?;
            self.expect(TokenKind::Semicolon, "NBT 条目后需要 `;`")?;
            if !seen_keys.insert(key.clone()) {
                return Err(Diagnostic::new(format!("NBT 键 `{key}` 重复"), key_span));
            }
            entries.push(NbtEntry {
                key,
                key_span,
                value,
            });
        }
        let end = self.advance().span;
        Ok(NbtValue {
            kind: NbtValueKind::Compound(entries),
            span: start.merge(end),
        })
    }

    /// 一个 NBT 值：嵌套复合、列表/数组、数值、字符串或布尔。
    pub(super) fn nbt_value(&mut self) -> Result<NbtValue, Diagnostic> {
        let start = self.current().span;
        match &self.current().kind {
            TokenKind::LeftBrace => self.nbt_compound_body(start, false),
            TokenKind::LeftBracket => self.nbt_list_or_array(),
            TokenKind::String(_) => {
                let (value, span) = self.string("NBT 字符串")?;
                Ok(NbtValue {
                    kind: NbtValueKind::String(value),
                    span,
                })
            }
            TokenKind::Ident(word) => match boolean_word(word) {
                Some("true") => {
                    let span = self.advance().span;
                    Ok(NbtValue {
                        kind: NbtValueKind::Byte(1),
                        span,
                    })
                }
                Some("false") => {
                    let span = self.advance().span;
                    Ok(NbtValue {
                        kind: NbtValueKind::Byte(0),
                        span,
                    })
                }
                _ => Err(Diagnostic::new(
                    format!("无法识别的 NBT 值 `{word}`；字符串需要加引号，布尔写成 true/false"),
                    start,
                )),
            },
            TokenKind::Number(_)
            | TokenKind::Decimal(_)
            | TokenKind::Byte(_)
            | TokenKind::Short(_)
            | TokenKind::Long(_)
            | TokenKind::Float(_)
            | TokenKind::Double(_)
            | TokenKind::Minus
            | TokenKind::Plus => self.nbt_number(),
            TokenKind::Eof => Err(Diagnostic::new("NBT 值不完整", start)),
            _ => Err(Diagnostic::new(
                "NBT 值需要数字、字符串、列表、数组或 `{ ... }`",
                start,
            )),
        }
    }

    /// 可带符号的数值字面量；后缀决定标签类型，小数无后缀时是双精度。
    fn nbt_number(&mut self) -> Result<NbtValue, Diagnostic> {
        let start = self.current().span;
        let negative = self.negative_sign();
        let token = self.advance().clone();
        let span = start.merge(token.span);
        let kind = match token.kind {
            TokenKind::Byte(value) => NbtValueKind::Byte(signed_integer(
                value,
                negative,
                i64::from(i8::MIN),
                i64::from(i8::MAX),
                "字节",
                span,
            )? as i8),
            TokenKind::Short(value) => NbtValueKind::Short(signed_integer(
                value,
                negative,
                i64::from(i16::MIN),
                i64::from(i16::MAX),
                "短整数",
                span,
            )? as i16),
            TokenKind::Number(value) => NbtValueKind::Int(signed_integer(
                value,
                negative,
                i64::from(i32::MIN),
                i64::from(i32::MAX),
                "整数",
                span,
            )? as i32),
            TokenKind::Long(value) => NbtValueKind::Long(signed_integer(
                value,
                negative,
                i64::MIN,
                i64::MAX,
                "长整数",
                span,
            )?),
            TokenKind::Float(value) => NbtValueKind::Float(signed_float(value, negative) as f32),
            TokenKind::Double(value) | TokenKind::Decimal(value) => {
                NbtValueKind::Double(signed_float(value, negative))
            }
            _ => {
                return Err(Diagnostic::new(
                    "NBT 数值需要数字，例如 1、1b、1s、1L、1.5f 或 1.5d",
                    token.span,
                ));
            }
        };
        Ok(NbtValue { kind, span })
    }

    /// `[...]`：`[B; ...]` 等三种整数数组，其余是列表。
    fn nbt_list_or_array(&mut self) -> Result<NbtValue, Diagnostic> {
        let start = self.advance().span;
        if let Some(kind) = self.array_prefix() {
            self.advance();
            self.advance();
            return self.nbt_array(kind, start);
        }
        let mut values = Vec::new();
        if !self.check(&TokenKind::RightBracket) {
            loop {
                values.push(self.nbt_value()?);
                if self.take(&TokenKind::Comma).is_none() {
                    break;
                }
            }
        }
        let end = self
            .expect(TokenKind::RightBracket, "NBT 列表缺少 `]`")?
            .span;
        Ok(NbtValue {
            kind: NbtValueKind::List(values),
            span: start.merge(end),
        })
    }

    /// 数组前缀：`[B;`、`[I;`、`[L;`，大小写与 26.3 SNBT 一致只接受大写。
    fn array_prefix(&self) -> Option<ArrayKind> {
        let TokenKind::Ident(prefix) = &self.current().kind else {
            return None;
        };
        let kind = match prefix.as_str() {
            "B" => ArrayKind::Byte,
            "I" => ArrayKind::Int,
            "L" => ArrayKind::Long,
            _ => return None,
        };
        matches!(self.peek_kind(1).kind, TokenKind::Semicolon).then_some(kind)
    }

    fn nbt_array(&mut self, kind: ArrayKind, start: Span) -> Result<NbtValue, Diagnostic> {
        let mut bytes = Vec::new();
        let mut ints = Vec::new();
        let mut longs = Vec::new();
        if !self.check(&TokenKind::RightBracket) {
            loop {
                let value = self.nbt_array_element(kind)?;
                match kind {
                    ArrayKind::Byte => bytes.push(value as i8),
                    ArrayKind::Int => ints.push(value as i32),
                    ArrayKind::Long => longs.push(value),
                }
                if self.take(&TokenKind::Comma).is_none() {
                    break;
                }
            }
        }
        let end = self
            .expect(
                TokenKind::RightBracket,
                &format!("{}缺少 `]`", kind.label()),
            )?
            .span;
        let value = match kind {
            ArrayKind::Byte => NbtValueKind::ByteArray(bytes),
            ArrayKind::Int => NbtValueKind::IntArray(ints),
            ArrayKind::Long => NbtValueKind::LongArray(longs),
        };
        Ok(NbtValue {
            kind: value,
            span: start.merge(end),
        })
    }

    /// 数组元素：整数后缀必须被数组类型接受，并按后缀检查取值范围。
    fn nbt_array_element(&mut self, kind: ArrayKind) -> Result<i64, Diagnostic> {
        let negative = self.negative_sign();
        let token = self.advance().clone();
        let (value, suffix) = match token.kind {
            TokenKind::Number(value) => (value, 'i'),
            TokenKind::Byte(value) => (value, 'b'),
            TokenKind::Short(value) => (value, 's'),
            TokenKind::Long(value) => (value, 'l'),
            _ => {
                return Err(Diagnostic::new(
                    format!(
                        "{}只能包含整数，可使用 {} 前缀、无后缀整数或更窄的整数后缀",
                        kind.label(),
                        kind.prefix()
                    ),
                    token.span,
                ));
            }
        };
        if !kind.allows(suffix) {
            return Err(Diagnostic::new(
                format!(
                    "{}的元素不能使用 `{suffix}` 后缀；26.3 只接受 {} 与更窄的整数后缀",
                    kind.label(),
                    kind.prefix()
                ),
                token.span,
            ));
        }
        let (minimum, maximum, label) = kind.element_range(suffix);
        signed_integer(value, negative, minimum, maximum, label, token.span)
    }
}

/// 应用符号并检查范围；词法器解析出的无符号数字一定在 i64 内，取反不会溢出。
fn signed_integer(
    value: i64,
    negative: bool,
    minimum: i64,
    maximum: i64,
    label: &str,
    span: Span,
) -> Result<i64, Diagnostic> {
    let signed = if negative { -value } else { value };
    if signed < minimum || signed > maximum {
        return Err(Diagnostic::new(
            format!("{label}超出 {minimum} 到 {maximum} 的范围"),
            span,
        ));
    }
    Ok(signed)
}

fn signed_float(value: f64, negative: bool) -> f64 {
    if negative { -value } else { value }
}
