//! 结构化 `execute` 子句：修饰符、条件与 store。
//!
//! `execute` 有两种写法：`execute "as @a at @s" { ... }` 原样转发子句字符串，
//! 以及 `execute as(查询) if 条件 store.result(持有者, 目标) { ... }` 的结构化
//! 子句。后者全部下降为真实命令形状，通过 `--deny-raw` 而不被拒绝。
//!
//! 子句顺序固定为“修饰符 → 条件 → store”：条件与 store 依赖修饰符建立的
//! 执行上下文，倒序书写会给出针对性的诊断（语义阶段检查）。
use crate::ast::{
    Anchor, ExecuteClause, ExecuteClauseKind, ExecuteClauses, ExecuteStoreData, ExecuteStoreTarget,
    ScoreTarget, Span, StatementKind, StoreDataMode,
};
use crate::diagnostic::Diagnostic;
use crate::lexer::TokenKind;

use super::Parser;
use super::keywords::{
    anchor_value, bossbar_field, entity_relation, execute_clause, store_data_mode, store_data_type,
    store_method, word_matches,
};

impl Parser {
    /// `execute` 语句：字符串子句或结构化子句 + 块。
    pub(super) fn execute_statement(&mut self) -> Result<StatementKind, Diagnostic> {
        if matches!(self.current().kind, TokenKind::String(_)) {
            let (clauses, span) = self.string("execute 后需要子句字符串或结构化子句")?;
            if clauses.trim().is_empty()
                || clauses.contains(['\n', '\r'])
                || clauses.trim_start().starts_with("execute ")
                || clauses.trim_start().starts_with("run ")
                || clauses.trim_end().ends_with(" run")
            {
                return Err(Diagnostic::new(
                    "execute 字符串应只包含子句，例如 `as @a at @s`；也可以直接写结构化子句 `as(查询)`",
                    span,
                ));
            }
            let (body, _) = self.block()?;
            return Ok(StatementKind::Execute {
                clauses: ExecuteClauses::Raw(clauses),
                body,
            });
        }
        let clauses = self.execute_clauses()?;
        let (body, _) = self.block()?;
        Ok(StatementKind::Execute {
            clauses: ExecuteClauses::Structured(clauses),
            body,
        })
    }

    /// 解析 `execute` 与块之间的全部子句，直到 `{`。
    fn execute_clauses(&mut self) -> Result<Vec<ExecuteClause>, Diagnostic> {
        let mut clauses = Vec::new();
        while !self.check(&TokenKind::LeftBrace) {
            if self.check(&TokenKind::Eof) {
                return Err(Diagnostic::new(
                    "execute 子句后需要 `{` 开始的代码块",
                    self.current().span,
                ));
            }
            clauses.push(self.execute_clause()?);
        }
        Ok(clauses)
    }

    fn execute_clause(&mut self) -> Result<ExecuteClause, Diagnostic> {
        let start = self.current().span;
        if self.take_word("if").is_some() {
            let condition = self.condition()?;
            return Ok(ExecuteClause {
                kind: ExecuteClauseKind::If(condition),
                span: start.merge(self.previous().span),
            });
        }
        if self.take_word("unless").is_some() {
            let condition = self.condition()?;
            return Ok(ExecuteClause {
                kind: ExecuteClauseKind::Unless(condition),
                span: start.merge(self.previous().span),
            });
        }

        let (word, word_span) = self.ident("execute 子句")?;
        if word_matches(&word, "store") {
            return self.execute_store(start);
        }
        let Some(clause) = execute_clause(&word) else {
            return Err(Diagnostic::new(
                format!(
                    "未知 execute 子句 `{word}`；可用 as(查询)、at(查询)、positioned(坐标)、\
                     rotated(朝向)、facing(...)、align(轴)、anchored(锚点)、in(\"维度\")、\
                     on(关系)、summon(\"实体类型\")、if 条件、unless 条件 或 store.result/success/data"
                ),
                word_span,
            ));
        };
        let kind = match clause {
            "as" | "at" => {
                self.expect(TokenKind::LeftParen, "as/at 子句后需要 `(`")?;
                let (query, query_span) = self.ident("实体查询名称")?;
                self.expect(TokenKind::RightParen, "实体查询名称后需要 `)`")?;
                if clause == "as" {
                    ExecuteClauseKind::As { query, query_span }
                } else {
                    ExecuteClauseKind::At { query, query_span }
                }
            }
            "positioned" => {
                self.expect(TokenKind::LeftParen, "positioned 后需要 `(`")?;
                let position = self.position_value("positioned 坐标")?;
                self.expect(TokenKind::RightParen, "坐标后需要 `)`")?;
                ExecuteClauseKind::Positioned(position)
            }
            "rotated" => {
                self.expect(TokenKind::LeftParen, "rotated 后需要 `(`")?;
                let rotation = self.rotation_value("rotated 朝向")?;
                self.expect(TokenKind::RightParen, "朝向参数后需要 `)`")?;
                ExecuteClauseKind::Rotated(rotation)
            }
            "facing" => self.facing_value()?,
            "align" => {
                self.expect(TokenKind::LeftParen, "align 子句后需要 `(`")?;
                let axes = self.align_axes()?;
                self.expect(TokenKind::RightParen, "align 轴列表后需要 `)`")?;
                ExecuteClauseKind::Align { axes }
            }
            "anchored" => {
                self.expect(TokenKind::LeftParen, "anchored 子句后需要 `(`")?;
                let (anchor, _) = self.anchor_argument("anchored 需要 eyes 或 feet")?;
                self.expect(TokenKind::RightParen, "锚点后需要 `)`")?;
                ExecuteClauseKind::Anchored(anchor)
            }
            "in" => {
                self.expect(TokenKind::LeftParen, "in 子句后需要 `(`")?;
                let (dimension, dimension_span) = self.string("in 子句需要维度资源位置字符串")?;
                self.expect(TokenKind::RightParen, "维度资源位置后需要 `)`")?;
                ExecuteClauseKind::In {
                    dimension,
                    dimension_span,
                }
            }
            "on" => {
                self.expect(TokenKind::LeftParen, "on 子句后需要 `(`")?;
                let (relation, relation_span) = self.ident("on 子句的实体关系")?;
                let Some(relation) = entity_relation(&relation) else {
                    return Err(Diagnostic::new(
                        format!(
                            "未知实体关系 `{relation}`，可用 owner、leasher、target、attacker、\
                             vehicle、controller、origin 或 passengers"
                        ),
                        relation_span,
                    ));
                };
                self.expect(TokenKind::RightParen, "实体关系后需要 `)`")?;
                ExecuteClauseKind::On(relation)
            }
            "summon" => {
                self.expect(TokenKind::LeftParen, "summon 子句后需要 `(`")?;
                let (entity_type, entity_type_span) =
                    self.string("summon 子句需要实体类型资源位置字符串")?;
                self.expect(TokenKind::RightParen, "实体类型后需要 `)`")?;
                ExecuteClauseKind::Summon {
                    entity_type,
                    entity_type_span,
                }
            }
            _ => unreachable!("execute_clause 与解析分支不同步"),
        };
        Ok(ExecuteClause {
            kind,
            span: start.merge(self.previous().span),
        })
    }

    /// `facing(<坐标>)` 或 `facing(entity(<查询>), <锚点>)`。
    fn facing_value(&mut self) -> Result<ExecuteClauseKind, Diagnostic> {
        self.expect(TokenKind::LeftParen, "facing 后需要 `(`")?;
        if self.check_word("entity") && matches!(self.peek_kind(1).kind, TokenKind::LeftParen) {
            self.take_word("entity");
            self.expect(TokenKind::LeftParen, "entity 后需要 `(`")?;
            let (query, query_span) = self.ident("实体查询名称")?;
            self.expect(TokenKind::RightParen, "查询名称后需要 `)`")?;
            self.expect(TokenKind::Comma, "查询名称后需要 `,` 和锚点")?;
            let (anchor, _) = self.anchor_argument("facing entity 需要 eyes 或 feet")?;
            self.expect(TokenKind::RightParen, "锚点后需要 `)`")?;
            Ok(ExecuteClauseKind::FacingEntity {
                query,
                query_span,
                anchor,
            })
        } else {
            let position = self.position_value("facing 坐标")?;
            self.expect(TokenKind::RightParen, "坐标后需要 `)`")?;
            Ok(ExecuteClauseKind::FacingPosition(position))
        }
    }

    /// `align(...)` 的轴列表：`xyz` 的非空子集，或字符串 `"xz"`。
    fn align_axes(&mut self) -> Result<String, Diagnostic> {
        let (axes, span) = match &self.current().kind {
            TokenKind::String(_) => self.string("align 需要轴列表")?,
            _ => self.ident("轴列表（xyz 的非空子集，例如 xyz、xz 或 y）")?,
        };
        let mut seen = [false; 3];
        for character in axes.chars() {
            let index = match character.to_ascii_lowercase() {
                'x' => 0,
                'y' => 1,
                'z' => 2,
                _ => {
                    return Err(Diagnostic::new(
                        format!("align 轴列表只能包含 x、y、z，实际为 `{axes}`"),
                        span,
                    ));
                }
            };
            if seen[index] {
                return Err(Diagnostic::new(
                    format!("align 轴列表里的 `{character}` 重复"),
                    span,
                ));
            }
            seen[index] = true;
        }
        if !seen.iter().any(|value| *value) {
            return Err(Diagnostic::new("align 轴列表不能为空", span));
        }
        Ok(axes.to_ascii_lowercase())
    }

    fn anchor_argument(&mut self, message: &str) -> Result<(Anchor, Span), Diagnostic> {
        let (value, span) = self.ident(message)?;
        match anchor_value(&value) {
            Some(anchor) => Ok((anchor, span)),
            None => Err(Diagnostic::new(
                format!("未知锚点 `{value}`，可用 eyes（眼睛）或 feet（脚）"),
                span,
            )),
        }
    }

    /// `store.result/success(持有者, 目标)`、`store.result/success(bossbar, "id", value|max)`
    /// 或 `store.data([result|success,] 来源, "路径", 类型[, 缩放])`。
    fn execute_store(&mut self, start: Span) -> Result<ExecuteClause, Diagnostic> {
        self.expect(TokenKind::Dot, "store 后需要 `.`")?;
        let (method, method_span) = self.ident("store 方法")?;
        let Some(method) = store_method(&method) else {
            return Err(Diagnostic::new(
                format!(
                    "未知 store 方法 `{method}`，可用 result（结果）、success（成功）或 data（数据）"
                ),
                method_span,
            ));
        };
        let kind = match method {
            "result" | "success" => {
                self.expect(TokenKind::LeftParen, "store 目标后需要 `(`")?;
                let target = if self.take_word("bossbar").is_some() {
                    self.expect(TokenKind::Comma, "bossbar 后需要 `,` 和资源位置")?;
                    let (id, id_span) = self.string("bossbar 需要资源位置字符串")?;
                    self.expect(TokenKind::Comma, "bossbar 资源位置后需要 `,`")?;
                    let (field, field_span) = self.ident("bossbar 字段 value 或 max")?;
                    let Some(field) = bossbar_field(&field) else {
                        return Err(Diagnostic::new(
                            format!("未知 bossbar 字段 `{field}`，可用 value（值）或 max（上限）"),
                            field_span,
                        ));
                    };
                    ExecuteStoreTarget::BossBar { id, id_span, field }
                } else {
                    let holder = self.score_holder()?;
                    self.expect(TokenKind::Comma, "store 持有者后需要 `,`")?;
                    let (objective, objective_span) = self.ident("store 的计分板目标名称")?;
                    ExecuteStoreTarget::Score(ScoreTarget {
                        holder,
                        objective,
                        objective_span,
                    })
                };
                self.expect(TokenKind::RightParen, "store 目标后需要 `)`")?;
                if method == "result" {
                    ExecuteClauseKind::StoreResult(target)
                } else {
                    ExecuteClauseKind::StoreSuccess(target)
                }
            }
            "data" => {
                self.expect(TokenKind::LeftParen, "store.data 后需要 `(`")?;
                let explicit = match &self.current().kind {
                    TokenKind::Ident(name) => store_data_mode(name),
                    _ => None,
                };
                let mode = if let Some(mode) = explicit {
                    self.advance();
                    self.expect(TokenKind::Comma, "store.data 模式后需要 `,`")?;
                    mode
                } else {
                    StoreDataMode::Result
                };
                let source = self.nbt_source_value("store.data 来源")?;
                self.expect(TokenKind::Comma, "store.data 来源后需要 `,`")?;
                let (path, path_span) = self.string("store.data 需要 NBT 路径字符串")?;
                self.expect(TokenKind::Comma, "store.data 路径后需要 `,`")?;
                let (name, kind_span) = self.ident("store.data 数值类型")?;
                let Some(kind) = store_data_type(&name) else {
                    return Err(Diagnostic::new(
                        format!(
                            "未知 store.data 数值类型 `{name}`，可用 byte、short、int、long、float 或 double"
                        ),
                        kind_span,
                    ));
                };
                let scale = self.optional_scale("store.data 缩放")?;
                self.expect(TokenKind::RightParen, "store.data 调用缺少 `)`")?;
                ExecuteClauseKind::StoreData(ExecuteStoreData {
                    mode,
                    source,
                    path,
                    path_span,
                    kind,
                    scale,
                })
            }
            _ => unreachable!("store_method 只返回 result、success、data"),
        };
        Ok(ExecuteClause {
            kind,
            span: start.merge(self.previous().span),
        })
    }

    /// 可选的小数参数：`..., 1.5`；缺省返回 `None`，由代码生成补 1。
    fn optional_scale(&mut self, label: &str) -> Result<Option<String>, Diagnostic> {
        if self.take(&TokenKind::Comma).is_none() {
            return Ok(None);
        }
        let negative = self.negative_sign();
        let token = self.advance().clone();
        let text = match token.kind {
            TokenKind::Number(value) => value.to_string(),
            TokenKind::Decimal(value) => format!("{value}"),
            _ => {
                return Err(Diagnostic::new(
                    format!("{label}需要数字，例如 1 或 0.5"),
                    token.span,
                ));
            }
        };
        Ok(Some(if negative { format!("-{text}") } else { text }))
    }
}
