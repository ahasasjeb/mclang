use crate::ast::*;
use crate::diagnostic::Diagnostic;
use crate::lexer::TokenKind;

use crate::parser::Parser;
use crate::parser::keywords::{
    advancement_method, score_operation, scoreboard_method, stopwatch_method,
};

impl Parser {
    pub(super) fn scoreboard_statement(&mut self) -> Result<StatementKind, Diagnostic> {
        self.expect(TokenKind::Dot, "scoreboard 后需要 `.`")?;
        let (method, method_span) = self.ident("scoreboard 方法")?;
        let Some(method) = scoreboard_method(&method) else {
            return Err(Diagnostic::new(
                format!("未知 scoreboard 方法 `{method}`"),
                method_span,
            ));
        };
        if method == "display" {
            return self.scoreboard_display_statement();
        }
        let target = self.score_target("scoreboard 方法")?;
        match method {
            "set" => {
                self.expect(TokenKind::Comma, "计分目标后需要 `,`")?;
                let value = self.expression()?;
                self.expect(TokenKind::RightParen, "scoreboard.set 调用缺少 `)`")?;
                self.expect(TokenKind::Semicolon, "scoreboard.set 调用后需要 `;`")?;
                Ok(StatementKind::ScoreSet { target, value })
            }
            "reset" => {
                self.expect(TokenKind::RightParen, "scoreboard.reset 调用缺少 `)`")?;
                self.expect(TokenKind::Semicolon, "scoreboard.reset 调用后需要 `;`")?;
                Ok(StatementKind::ScoreReset { target })
            }
            "enable" => {
                self.expect(TokenKind::RightParen, "scoreboard.enable 调用缺少 `)`")?;
                self.expect(TokenKind::Semicolon, "scoreboard.enable 调用后需要 `;`")?;
                Ok(StatementKind::ScoreboardEnable { target })
            }
            "operation" => {
                self.expect(TokenKind::Comma, "结果计分目标后需要 `,`")?;
                let (operation, operation_span) =
                    self.ident("运算名称 set/add/subtract/multiply/divide/modulo/min/max/swap")?;
                let Some(operation) = score_operation(&operation) else {
                    return Err(Diagnostic::new(
                        format!(
                            "未知运算 `{operation}`，可用 set、add、subtract、multiply、divide、modulo、min、max、swap"
                        ),
                        operation_span,
                    ));
                };
                let operation = match operation {
                    "set" => ScoreboardOp::Set,
                    "add" => ScoreboardOp::Add,
                    "subtract" => ScoreboardOp::Subtract,
                    "multiply" => ScoreboardOp::Multiply,
                    "divide" => ScoreboardOp::Divide,
                    "modulo" => ScoreboardOp::Modulo,
                    "min" => ScoreboardOp::Min,
                    "max" => ScoreboardOp::Max,
                    _ => ScoreboardOp::Swap,
                };
                self.expect(TokenKind::Comma, "运算名称后需要 `,`")?;
                let source = self.score_target_body("来源计分目标")?;
                self.expect(TokenKind::RightParen, "scoreboard.operation 调用缺少 `)`")?;
                self.expect(TokenKind::Semicolon, "scoreboard.operation 调用后需要 `;`")?;
                Ok(StatementKind::ScoreboardOperation {
                    result: target,
                    operation,
                    source,
                })
            }
            "get" => Err(Diagnostic::new(
                "scoreboard.get 只能出现在表达式里，例如 `let id = scoreboard.get(self, box_key);`",
                method_span,
            )),
            _ => unreachable!("scoreboard_method 只返回已知方法"),
        }
    }

    /// `scoreboard.display("侧边栏"[, 目标]);`
    pub(super) fn scoreboard_display_statement(&mut self) -> Result<StatementKind, Diagnostic> {
        self.expect(TokenKind::LeftParen, "scoreboard.display 后需要 `(`")?;
        let (slot, slot_span) = self.string("scoreboard.display 需要显示槽字符串")?;
        let objective = if self.take(&TokenKind::Comma).is_some() {
            Some(self.ident("scoreboard.display 需要已声明的目标名称")?)
        } else {
            None
        };
        self.expect(TokenKind::RightParen, "scoreboard.display 调用缺少 `)`")?;
        self.expect(TokenKind::Semicolon, "scoreboard.display 调用后需要 `;`")?;
        Ok(StatementKind::ScoreboardDisplay {
            slot,
            slot_span,
            objective,
        })
    }

    /// `teleport(持有者, 坐标或实体查询);`
    ///
    /// 落点写成 `pos`/`block_pos`/`vec3` 时传送到该坐标（可选 `rotation(...)`），
    /// 写成查询名称时跟随该单个实体的位置与朝向。
    pub(super) fn teleport_statement(&mut self) -> Result<StatementKind, Diagnostic> {
        self.expect(TokenKind::LeftParen, "teleport 后需要 `(`")?;
        let targets = self.score_holder()?;
        self.expect(TokenKind::Comma, "teleport 目标后需要 `,`")?;
        let destination =
            if self.check_word("pos") || self.check_word("block_pos") || self.check_word("vec3") {
                TeleportDestination::Position(self.position_value("传送坐标")?)
            } else {
                let (query, query_span) = self.ident("实体查询名称")?;
                TeleportDestination::Entity { query, query_span }
            };
        let rotation = if self.take(&TokenKind::Comma).is_some() {
            Some(self.rotation_value("传送朝向")?)
        } else {
            None
        };
        self.expect(TokenKind::RightParen, "teleport 调用缺少 `)`")?;
        self.expect(TokenKind::Semicolon, "teleport 调用后需要 `;`")?;
        Ok(StatementKind::Teleport {
            targets,
            destination,
            rotation,
        })
    }

    /// `advancement.grant(目标, 进度[, 准则]);` 及其余四种作用范围的方法。
    ///
    /// `everything` 不需要进度参数；只有 `only` 可以带准则名。
    pub(super) fn advancement_statement(&mut self) -> Result<StatementKind, Diagnostic> {
        self.expect(TokenKind::Dot, "advancement 后需要 `.`")?;
        let (method, method_span) = self.ident("advancement 方法")?;
        let Some((operation, scope)) = advancement_method(&method) else {
            return Err(Diagnostic::new(
                format!("未知 advancement 方法 `{method}`"),
                method_span,
            ));
        };
        self.expect(TokenKind::LeftParen, "advancement 方法后需要 `(`")?;
        let targets = self.holder("advancement 目标")?;
        let mut advancement = None;
        let mut criterion = None;
        let mut criterion_span = None;
        if scope != "everything" {
            self.expect(TokenKind::Comma, "advancement 目标后需要 `,`")?;
            advancement = Some(self.advancement_reference("进度")?);
        }
        if scope == "only" && self.take(&TokenKind::Comma).is_some() {
            let (value, span) = self.ident("准则名称")?;
            criterion = Some(value);
            criterion_span = Some(span);
        }
        self.expect(TokenKind::RightParen, "advancement 调用缺少 `)`")?;
        self.expect(TokenKind::Semicolon, "advancement 调用后需要 `;`")?;
        Ok(StatementKind::AdvancementAction {
            operation: if operation == "grant" {
                AdvancementOperation::Grant
            } else {
                AdvancementOperation::Revoke
            },
            scope: match scope {
                "only" => AdvancementScope::Only,
                "through" => AdvancementScope::Through,
                "from" => AdvancementScope::From,
                "until" => AdvancementScope::Until,
                _ => AdvancementScope::Everything,
            },
            targets,
            advancement,
            criterion,
            criterion_span,
        })
    }

    /// 读取计分持有者：`self`/`自身`、`origin`/`投掷者` 或实体查询名称。
    pub(in crate::parser) fn score_holder(&mut self) -> Result<Holder, Diagnostic> {
        self.holder("计分持有者")
    }

    /// 读取实体持有者：`self`/`自身`、`origin`/`投掷者` 或实体查询名称。
    pub(in crate::parser) fn holder(&mut self, label: &str) -> Result<Holder, Diagnostic> {
        if self.take_word("self").is_some() {
            return Ok(Holder::SelfEntity);
        }
        if self.take_word("origin").is_some() {
            return Ok(Holder::Origin);
        }
        let (name, span) =
            self.ident(&format!("{label} self/自身、origin/投掷者 或实体查询名称"))?;
        Ok(Holder::Query(name, span))
    }

    /// 读取计分目标的 `(持有者, 目标)` 部分，右括号留给调用方。
    pub(in crate::parser) fn score_target(
        &mut self,
        label: &str,
    ) -> Result<ScoreTarget, Diagnostic> {
        self.expect(TokenKind::LeftParen, &format!("{label} 后需要 `(`"))?;
        self.score_target_body(label)
    }

    /// `(持有者, 目标)` 的内部形式：`scoreboard.operation` 的来源参数不带括号。
    pub(in crate::parser) fn score_target_body(
        &mut self,
        _label: &str,
    ) -> Result<ScoreTarget, Diagnostic> {
        let holder = self.score_holder()?;
        self.expect(TokenKind::Comma, "计分持有者后需要 `,`")?;
        let (objective, objective_span) = self.ident("计分板目标名称")?;
        Ok(ScoreTarget {
            holder,
            objective,
            objective_span,
        })
    }

    /// `stopwatch.create/restart/remove("命名空间:id")`。
    ///
    /// `stopwatch.query` 有返回值，只能在表达式里使用，语句形式会给出引导性诊断。
    pub(super) fn stopwatch_statement(&mut self) -> Result<StatementKind, Diagnostic> {
        self.expect(TokenKind::Dot, "stopwatch 后需要 `.`")?;
        let (method, method_span) = self.ident("stopwatch 方法")?;
        let Some(method) = stopwatch_method(&method) else {
            return Err(Diagnostic::new(
                format!("未知 stopwatch 方法 `{method}`"),
                method_span,
            ));
        };
        if method == "query" {
            return Err(Diagnostic::new(
                "stopwatch.query 只能出现在表达式里，例如 `let seconds = stopwatch.query(\"demo:timer\");`",
                method_span,
            ));
        }
        self.expect(TokenKind::LeftParen, "stopwatch 方法后需要 `(`")?;
        let (id, _) = self.string("stopwatch 需要秒表资源位置")?;
        self.expect(TokenKind::RightParen, "stopwatch 调用缺少 `)`")?;
        self.expect(TokenKind::Semicolon, "stopwatch 调用后需要 `;`")?;
        Ok(StatementKind::StopwatchAction {
            operation: match method {
                "create" => StopwatchOperation::Create,
                "restart" => StopwatchOperation::Restart,
                _ => StopwatchOperation::Remove,
            },
            id,
        })
    }
}
