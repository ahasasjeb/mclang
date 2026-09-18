use crate::ast::*;
use crate::diagnostic::Diagnostic;
use crate::lexer::TokenKind;

use crate::parser::Parser;
use crate::parser::keywords::{data_method, item_method};

impl Parser {
    pub(super) fn item_statement(&mut self) -> Result<StatementKind, Diagnostic> {
        self.expect(TokenKind::Dot, "item 后需要 `.`")?;
        let (method, method_span) = self.ident("item 方法")?;
        let Some(method) = item_method(&method) else {
            return Err(Diagnostic::new(
                format!(
                    "未知 item 方法 `{method}`，可用 replace/替换、fill/填充、override/覆盖、modify/修改"
                ),
                method_span,
            ));
        };
        let method = match method {
            "replace" => ItemMethod::Replace,
            "fill" => ItemMethod::Fill,
            "override" => ItemMethod::Override,
            _ => ItemMethod::Modify,
        };
        self.expect(TokenKind::LeftParen, "item 方法后需要 `(`")?;
        let target = self.item_condition_source("item 目标")?;
        self.expect(TokenKind::Comma, "item 目标后需要 `,`")?;
        let (slots, slots_span) = self.string("item 需要槽位字符串")?;
        self.expect(TokenKind::Comma, "槽位后需要 `,`")?;
        let action = if method == ItemMethod::Modify {
            let (modifier, span) = self.string("item.modify 需要修饰器资源位置字符串")?;
            ItemActionKind::Modifier(modifier, span)
        } else {
            let (kind, kind_span) = self.ident("item 内容 with 或 from")?;
            match kind.as_str() {
                "with" | "用" => {
                    self.expect(TokenKind::LeftParen, "with 后需要 `(`")?;
                    let (item, span) = self.ident("with 需要已声明的物品名称")?;
                    self.expect(TokenKind::RightParen, "with 缺少 `)`")?;
                    ItemActionKind::With(item, span)
                }
                "from" | "来源" => {
                    self.expect(TokenKind::LeftParen, "from 后需要 `(`")?;
                    let source = self.item_condition_source("from 来源")?;
                    self.expect(TokenKind::Comma, "from 来源后需要 `,`")?;
                    let (source_slots, source_slots_span) =
                        self.string("from 需要来源槽位字符串")?;
                    let modifier = if self.take(&TokenKind::Comma).is_some() {
                        Some(self.string("from 修饰器资源位置字符串")?.0)
                    } else {
                        None
                    };
                    self.expect(TokenKind::RightParen, "from 缺少 `)`")?;
                    ItemActionKind::From {
                        source,
                        slots: source_slots,
                        slots_span: source_slots_span,
                        modifier,
                    }
                }
                _ => {
                    return Err(Diagnostic::new(
                        format!("item 内容只能是 with/用 或 from/来源，实际为 `{kind}`"),
                        kind_span,
                    ));
                }
            }
        };
        self.expect(TokenKind::RightParen, "item 调用缺少 `)`")?;
        self.expect(TokenKind::Semicolon, "item 调用后需要 `;`")?;
        Ok(StatementKind::ItemAction {
            method,
            target,
            slots,
            slots_span,
            action,
        })
    }

    /// `data.merge/remove/modify`；`data.get` 只能出现在表达式里。
    pub(super) fn data_statement(&mut self) -> Result<StatementKind, Diagnostic> {
        self.expect(TokenKind::Dot, "data 后需要 `.`")?;
        let (method, method_span) = self.ident("data 方法")?;
        let Some(method) = data_method(&method) else {
            return Err(Diagnostic::new(
                format!(
                    "未知 data 方法 `{method}`，可用 merge/合并、remove/移除、modify/修改；get/取 是表达式"
                ),
                method_span,
            ));
        };
        if method == "get" {
            return Err(Diagnostic::new(
                "data.get 只能出现在表达式里，例如 `let health = data.get(entity, self, \"Health\");`",
                method_span,
            ));
        }
        self.expect(TokenKind::LeftParen, "data 方法后需要 `(`")?;
        let target = self.nbt_source_value("data 目标")?;
        match method {
            "merge" => {
                self.expect(TokenKind::Comma, "data 目标后需要 `,`")?;
                let nbt = if matches!(target, NbtComponentSource::Entity(_)) {
                    self.nbt_compound_with_aliases("data.merge 数据")?
                } else {
                    self.nbt_compound("data.merge 数据")?
                };
                self.expect(TokenKind::RightParen, "data.merge 调用缺少 `)`")?;
                self.expect(TokenKind::Semicolon, "data.merge 调用后需要 `;`")?;
                Ok(StatementKind::DataMerge { target, nbt })
            }
            "remove" => {
                self.expect(TokenKind::Comma, "data 目标后需要 `,`")?;
                let (path, path_span) = self.string("data.remove 需要 NBT 路径字符串")?;
                self.expect(TokenKind::RightParen, "data.remove 调用缺少 `)`")?;
                self.expect(TokenKind::Semicolon, "data.remove 调用后需要 `;`")?;
                Ok(StatementKind::DataRemove {
                    target,
                    path,
                    path_span,
                })
            }
            _ => {
                self.expect(TokenKind::Comma, "data 目标后需要 `,`")?;
                let (path, path_span) = self.string("data.modify 需要 NBT 路径字符串")?;
                self.expect(TokenKind::Comma, "路径后需要 `,`")?;
                let (operation, operation_span) =
                    self.ident("data.modify 操作 insert/prepend/append/set/merge")?;
                let kind = match operation.as_str() {
                    "insert" | "插入" => DataOperationKind::Insert,
                    "prepend" | "前插" => DataOperationKind::Prepend,
                    "append" | "追加" => DataOperationKind::Append,
                    "set" | "设置" => DataOperationKind::Set,
                    "merge" | "合并" => DataOperationKind::Merge,
                    _ => {
                        return Err(Diagnostic::new(
                            format!(
                                "未知 data.modify 操作 `{operation}`，可用 insert、prepend、append、set、merge"
                            ),
                            operation_span,
                        ));
                    }
                };
                let index = if kind == DataOperationKind::Insert {
                    self.expect(TokenKind::Comma, "insert 后需要 `,`")?;
                    Some(self.signed("insert 下标")?)
                } else {
                    None
                };
                self.expect(TokenKind::Comma, "操作后需要 `,`")?;
                let source = self.data_source()?;
                self.expect(TokenKind::RightParen, "data.modify 调用缺少 `)`")?;
                self.expect(TokenKind::Semicolon, "data.modify 调用后需要 `;`")?;
                Ok(StatementKind::DataModify {
                    target,
                    path,
                    path_span,
                    operation: DataOperation {
                        kind,
                        index,
                        source,
                    },
                })
            }
        }
    }

    /// `from(...)`、`value(nbt { ... })`、`string(...)` 或 `compute(...)`。
    pub(super) fn data_source(&mut self) -> Result<DataSource, Diagnostic> {
        let (name, span) = self.ident("数据来源 from、value、string 或 compute")?;
        match name.as_str() {
            "from" | "来源" => {
                self.expect(TokenKind::LeftParen, "from 后需要 `(`")?;
                let target = self.nbt_source_value("from 来源")?;
                self.expect(TokenKind::Comma, "from 来源后需要 `,`")?;
                let (path, path_span) = self.string("from 需要 NBT 路径字符串")?;
                self.expect(TokenKind::RightParen, "from 缺少 `)`")?;
                Ok(DataSource::From {
                    target,
                    path,
                    path_span,
                })
            }
            "value" | "值" => {
                self.expect(TokenKind::LeftParen, "value 后需要 `(`")?;
                // `value(nbt { ... })` 也可写成 `value({ ... })`，两者等价。
                self.take_word("nbt");
                let nbt = self.nbt_value()?;
                self.expect(TokenKind::RightParen, "value 缺少 `)`")?;
                Ok(DataSource::Value(nbt))
            }
            "string" | "字符串" => {
                self.expect(TokenKind::LeftParen, "string 后需要 `(`")?;
                let target = self.nbt_source_value("string 来源")?;
                self.expect(TokenKind::Comma, "string 来源后需要 `,`")?;
                let (path, path_span) = self.string("string 需要 NBT 路径字符串")?;
                let start = if self.take(&TokenKind::Comma).is_some() {
                    Some(self.signed("string 起始下标")?)
                } else {
                    None
                };
                let end = if start.is_some() && self.take(&TokenKind::Comma).is_some() {
                    Some(self.signed("string 结束下标")?)
                } else {
                    None
                };
                self.expect(TokenKind::RightParen, "string 缺少 `)`")?;
                Ok(DataSource::String {
                    target,
                    path,
                    path_span,
                    start,
                    end,
                })
            }
            "compute" | "计算" => {
                self.expect(TokenKind::LeftParen, "compute 后需要 `(`")?;
                let (source, kind, provider, provider_span, scale) = self.compute_payload()?;
                self.expect(TokenKind::RightParen, "compute 缺少 `)`")?;
                Ok(DataSource::Compute {
                    source,
                    kind,
                    provider,
                    provider_span,
                    scale,
                })
            }
            _ => Err(Diagnostic::new(
                format!(
                    "未知数据来源 `{name}`，可用 from/来源、value/值、string/字符串、compute/计算"
                ),
                span,
            )),
        }
    }
}
