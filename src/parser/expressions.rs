//! 算术表达式、括号分组和实参列表。

use crate::ast::*;
use crate::diagnostic::Diagnostic;
use crate::lexer::TokenKind;

use super::Parser;
use super::keywords::{
    compute_kind, compute_source, data_method, gamerule_method, scoreboard_method, time_method,
    word_matches, worldborder_method, xp_kind,
};

impl Parser {
    pub(super) fn expression(&mut self) -> Result<Expr, Diagnostic> {
        self.additive()
    }

    fn additive(&mut self) -> Result<Expr, Diagnostic> {
        let mut expression = self.multiplicative()?;
        loop {
            let operation = if self.take(&TokenKind::Plus).is_some() {
                Some(BinaryOp::Add)
            } else if self.take(&TokenKind::Minus).is_some() {
                Some(BinaryOp::Subtract)
            } else {
                None
            };
            let Some(operation) = operation else { break };
            let right = self.multiplicative()?;
            let span = expression.span.merge(right.span);
            expression = Expr {
                kind: ExprKind::Binary {
                    left: Box::new(expression),
                    operation,
                    right: Box::new(right),
                },
                span,
            };
        }
        Ok(expression)
    }

    fn multiplicative(&mut self) -> Result<Expr, Diagnostic> {
        let mut expression = self.unary()?;
        loop {
            let operation = if self.take(&TokenKind::Star).is_some() {
                Some(BinaryOp::Multiply)
            } else if self.take(&TokenKind::Slash).is_some() {
                Some(BinaryOp::Divide)
            } else if self.take(&TokenKind::Percent).is_some() {
                Some(BinaryOp::Modulo)
            } else {
                None
            };
            let Some(operation) = operation else { break };
            let right = self.unary()?;
            let span = expression.span.merge(right.span);
            expression = Expr {
                kind: ExprKind::Binary {
                    left: Box::new(expression),
                    operation,
                    right: Box::new(right),
                },
                span,
            };
        }
        Ok(expression)
    }

    fn unary(&mut self) -> Result<Expr, Diagnostic> {
        if let Some(minus) = self.take(&TokenKind::Minus) {
            if let TokenKind::Number(number) = &self.current().kind {
                let number = *number;
                let number_span = self.advance().span;
                let signed = number
                    .checked_neg()
                    .and_then(|value| i32::try_from(value).ok())
                    .ok_or_else(|| Diagnostic::new("整数超出 32 位范围", number_span))?;
                return Ok(Expr {
                    kind: ExprKind::Integer(signed),
                    span: minus.span.merge(number_span),
                });
            }
            let value = self.unary()?;
            let span = minus.span.merge(value.span);
            return Ok(Expr {
                kind: ExprKind::Negate(Box::new(value)),
                span,
            });
        }
        self.primary()
    }

    fn primary(&mut self) -> Result<Expr, Diagnostic> {
        let token = self.advance().clone();
        match token.kind {
            TokenKind::Number(number) => {
                let value = i32::try_from(number)
                    .map_err(|_| Diagnostic::new("整数超出 32 位范围", token.span))?;
                Ok(Expr {
                    kind: ExprKind::Integer(value),
                    span: token.span,
                })
            }
            TokenKind::Ident(name) => {
                if self.check(&TokenKind::LeftParen) {
                    // 命令结果表达式使用独立语法，先于普通函数调用识别。
                    if word_matches(&name, "count") {
                        return self.count_expression(token.span);
                    }
                    if word_matches(&name, "random") {
                        return self.random_expression(token.span);
                    }
                    if word_matches(&name, "compute") {
                        return self.compute_expression(token.span);
                    }
                    let arguments = self.call_arguments()?;
                    let span = token.span.merge(self.previous().span);
                    Ok(Expr {
                        kind: ExprKind::Call {
                            function: name,
                            arguments,
                        },
                        span,
                    })
                } else if self.check(&TokenKind::Dot) {
                    self.named_expression(name, token.span)
                } else {
                    Ok(Expr {
                        kind: ExprKind::Score(name),
                        span: token.span,
                    })
                }
            }
            TokenKind::LeftParen => {
                let expression = self.expression()?;
                let end = self.expect(TokenKind::RightParen, "表达式缺少 `)`")?.span;
                Ok(Expr {
                    span: token.span.merge(end),
                    ..expression
                })
            }
            _ => Err(Diagnostic::new(
                "这里需要整数、计分变量或括号表达式",
                token.span,
            )),
        }
    }

    pub(super) fn call_arguments(&mut self) -> Result<Vec<Expr>, Diagnostic> {
        self.expect(TokenKind::LeftParen, "函数名称后需要 `(`")?;
        let mut arguments = Vec::new();
        if !self.check(&TokenKind::RightParen) {
            loop {
                arguments.push(self.expression()?);
                if self.take(&TokenKind::Comma).is_none() {
                    break;
                }
            }
        }
        self.expect(TokenKind::RightParen, "实参列表缺少 `)`")?;
        Ok(arguments)
    }

    /// 具名表达式：目前只有 `xp.query(查询, points|levels)`。
    fn named_expression(&mut self, receiver: String, start_span: Span) -> Result<Expr, Diagnostic> {
        self.expect(TokenKind::Dot, "名称后需要 `.`")?;
        let (method, method_span) = self.ident("名称方法")?;
        let span = start_span.merge(method_span);
        if word_matches(&receiver, "xp") && word_matches(&method, "query") {
            self.expect(TokenKind::LeftParen, "xp.query 后需要 `(`")?;
            let (target, _) = self.ident("xp.query 目标查询名称")?;
            self.expect(TokenKind::Comma, "xp.query 目标后需要 `,`")?;
            let (kind, kind_span) = self.ident("xp 类型 points 或 levels")?;
            let Some(kind) = xp_kind(&kind) else {
                return Err(Diagnostic::new(
                    "xp 类型只能是 points/点数 或 levels/等级",
                    kind_span,
                ));
            };
            self.expect(TokenKind::RightParen, "xp.query 调用缺少 `)`")?;
            return Ok(Expr {
                kind: ExprKind::XpQuery { target, kind },
                span: start_span.merge(self.previous().span),
            });
        }
        if word_matches(&receiver, "time") && time_method(&method) == Some("query") {
            self.expect(TokenKind::LeftParen, "time.query 后需要 `(`")?;
            let clock = if self.check(&TokenKind::RightParen) {
                None
            } else {
                Some(self.string("time.query 需要世界时钟资源位置字符串")?.0)
            };
            self.expect(TokenKind::RightParen, "time.query 调用缺少 `)`")?;
            return Ok(Expr {
                kind: ExprKind::TimeQuery { clock },
                span: start_span.merge(self.previous().span),
            });
        }
        if word_matches(&receiver, "time") && time_method(&method) == Some("query_gametime") {
            self.expect(TokenKind::LeftParen, "time.query_gametime 后需要 `(`")?;
            self.expect(
                TokenKind::RightParen,
                "time.query_gametime 不接受参数，需要 `)`",
            )?;
            return Ok(Expr {
                kind: ExprKind::GameTimeQuery,
                span: start_span.merge(self.previous().span),
            });
        }
        if word_matches(&receiver, "gamerule") && gamerule_method(&method) == Some("query") {
            self.expect(TokenKind::LeftParen, "gamerule.query 后需要 `(`")?;
            let (name, _) = self.string("gamerule.query 需要规则名称字符串")?;
            self.expect(TokenKind::RightParen, "gamerule.query 调用缺少 `)`")?;
            return Ok(Expr {
                kind: ExprKind::GameRuleQuery { name },
                span: start_span.merge(self.previous().span),
            });
        }
        if word_matches(&receiver, "worldborder") && worldborder_method(&method) == Some("get") {
            self.expect(TokenKind::LeftParen, "worldborder.get 后需要 `(`")?;
            self.expect(
                TokenKind::RightParen,
                "worldborder.get 不接受参数，需要 `)`",
            )?;
            return Ok(Expr {
                kind: ExprKind::WorldBorderSize,
                span: start_span.merge(self.previous().span),
            });
        }
        if word_matches(&receiver, "scoreboard") && scoreboard_method(&method) == Some("get") {
            let target = self.score_target("scoreboard.get")?;
            self.expect(TokenKind::RightParen, "scoreboard.get 调用缺少 `)`")?;
            return Ok(Expr {
                kind: ExprKind::ScoreQuery { target },
                span: start_span.merge(self.previous().span),
            });
        }
        if word_matches(&receiver, "stopwatch") && word_matches(&method, "query") {
            self.expect(TokenKind::LeftParen, "stopwatch.query 后需要 `(`")?;
            let (id, _) = self.string("stopwatch.query 需要秒表资源位置")?;
            let scale = if self.take(&TokenKind::Comma).is_some() {
                Some(self.signed_number_text("秒表缩放比例")?)
            } else {
                None
            };
            self.expect(TokenKind::RightParen, "stopwatch.query 调用缺少 `)`")?;
            return Ok(Expr {
                kind: ExprKind::StopwatchQuery { id, scale },
                span: start_span.merge(self.previous().span),
            });
        }
        if word_matches(&receiver, "data") && data_method(&method) == Some("get") {
            self.expect(TokenKind::LeftParen, "data.get 后需要 `(`")?;
            let source = self.nbt_source_value("data.get 来源")?;
            self.expect(TokenKind::Comma, "data.get 来源后需要 `,`")?;
            let (path, path_span) = self.string("data.get 需要 NBT 路径字符串")?;
            self.expect(TokenKind::RightParen, "data.get 调用缺少 `)`")?;
            return Ok(Expr {
                kind: ExprKind::DataGet {
                    source,
                    path,
                    path_span,
                },
                span: start_span.merge(self.previous().span),
            });
        }
        Err(Diagnostic::new(
            format!(
                "未知的具名表达式 `{receiver}.{method}`；目前支持 `xp.query(查询, points|levels)`、`scoreboard.get(持有者, 目标)`、`stopwatch.query(\"命名空间:id\"[, 缩放])`、`time.query([时钟])`、`time.query_gametime()`、`gamerule.query(\"规则\")`、`worldborder.get()` 和 `data.get(来源, 路径)`"
            ),
            span,
        ))
    }

    /// `count(查询)`：查询命中的实体数量。
    fn count_expression(&mut self, start_span: Span) -> Result<Expr, Diagnostic> {
        self.expect(TokenKind::LeftParen, "count 后需要 `(`")?;
        let (query, query_span) = self.ident("count 需要已声明的实体查询名称")?;
        self.expect(TokenKind::RightParen, "count 调用缺少 `)`")?;
        Ok(Expr {
            kind: ExprKind::Count { query, query_span },
            span: start_span.merge(self.previous().span),
        })
    }

    /// `random(最小值, 最大值)`：闭区间随机整数。
    fn random_expression(&mut self, start_span: Span) -> Result<Expr, Diagnostic> {
        self.expect(TokenKind::LeftParen, "random 后需要 `(`")?;
        let min = self.signed("random 最小值")?;
        self.expect(TokenKind::Comma, "random 最小值后需要 `,`")?;
        let max = self.signed("random 最大值")?;
        self.expect(TokenKind::RightParen, "random 调用缺少 `)`")?;
        Ok(Expr {
            kind: ExprKind::Random { min, max },
            span: start_span.merge(self.previous().span),
        })
    }

    /// `compute(来源, float|integer, "provider"[, 缩放])`。
    fn compute_expression(&mut self, start_span: Span) -> Result<Expr, Diagnostic> {
        self.expect(TokenKind::LeftParen, "compute 后需要 `(`")?;
        let (source, kind, provider, provider_span, scale) = self.compute_payload()?;
        self.expect(TokenKind::RightParen, "compute 调用缺少 `)`")?;
        Ok(Expr {
            kind: ExprKind::Compute {
                source,
                kind,
                provider,
                provider_span,
                scale,
            },
            span: start_span.merge(self.previous().span),
        })
    }

    /// `compute` 的公共参数（表达式与 `data.modify` 的 compute 来源共用）。
    pub(super) fn compute_payload(
        &mut self,
    ) -> Result<(ComputeSource, ComputeKind, String, Span, Option<String>), Diagnostic> {
        let (source_name, source_span) = self.ident("compute 来源 default/block/entity")?;
        let Some(source_kind) = compute_source(&source_name) else {
            return Err(Diagnostic::new(
                format!(
                    "未知 compute 来源 `{source_name}`，可用 default（默认）、block（方块）或 entity（实体）"
                ),
                source_span,
            ));
        };
        let source = match source_kind {
            "default" => ComputeSource::Default,
            "block" => {
                self.expect(TokenKind::Comma, "compute block 来源后需要 `,`")?;
                ComputeSource::Block(self.block_position("compute 方块坐标")?)
            }
            _ => {
                self.expect(TokenKind::Comma, "compute entity 来源后需要 `,`")?;
                ComputeSource::Entity(self.score_holder()?)
            }
        };
        self.expect(TokenKind::Comma, "compute 来源后需要 `,`")?;
        let (kind_name, kind_span) = self.ident("compute 类型 float 或 integer")?;
        let Some(kind) = compute_kind(&kind_name) else {
            return Err(Diagnostic::new(
                format!("compute 类型只能是 float/浮点 或 integer/整数，实际为 `{kind_name}`"),
                kind_span,
            ));
        };
        let kind = if kind == "float" {
            ComputeKind::Float
        } else {
            ComputeKind::Integer
        };
        self.expect(TokenKind::Comma, "compute 类型后需要 `,`")?;
        let (provider, provider_span) = self.string("compute 需要 provider 资源位置字符串")?;
        let scale = if self.take(&TokenKind::Comma).is_some() {
            Some(self.signed_number_text("compute 缩放")?)
        } else {
            None
        };
        Ok((source, kind, provider, provider_span, scale))
    }
}
