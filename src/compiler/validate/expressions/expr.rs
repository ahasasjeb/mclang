use super::*;

pub(in crate::compiler::validate) fn validate_expr(
    expression: &Expr,
    locals: &HashSet<&str>,
    ctx: ValidationContext<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match &expression.kind {
        ExprKind::CoreCommand(command) => {
            crate::compiler::validate::core_commands::validate_core_command(
                command,
                expression.span,
                ctx,
                diagnostics,
            )
        }
        ExprKind::EntityCommand(command) => {
            crate::compiler::validate::entity_commands::validate_entity_command(
                command,
                expression.span,
                ctx,
                diagnostics,
            )
        }
        ExprKind::Integer(_) => {}
        ExprKind::Score(name) => {
            if !ctx.symbols.scores.contains(name.as_str())
                && !ctx.symbols.parameters.contains(name.as_str())
                && !locals.contains(name.as_str())
            {
                diagnostics.push(Diagnostic::new(
                    format!("找不到计分变量 `{name}`"),
                    expression.span,
                ));
            }
        }
        ExprKind::Call {
            function,
            arguments,
        } => {
            match ctx.symbols.functions.get(function.as_str()) {
                None => diagnostics.push(Diagnostic::new(
                    format!("找不到函数 `{function}`；如果它来自其他模块，请确认对方声明了 `export`，并在本模块 `import` 它"),
                    expression.span,
                )),
                Some(signature) => {
                    if !signature.returns_score {
                        diagnostics.push(Diagnostic::new(
                            format!("无返回值函数 `{function}` 不能用于表达式"),
                            expression.span,
                        ));
                    }
                    validate_call_context(function, *signature, expression.span, ctx, diagnostics);
                    if arguments.len() != signature.parameters {
                        diagnostics.push(Diagnostic::new(
                            format!(
                                "函数 `{function}` 需要 {} 个参数，实际提供 {} 个",
                                signature.parameters,
                                arguments.len()
                            ),
                            expression.span,
                        ));
                    }
                }
            }
            for argument in arguments {
                validate_expr(argument, locals, ctx, diagnostics);
            }
        }
        ExprKind::XpQuery { target, .. } => {
            if let Some(query) = super::super::statements::require_player_query(
                target,
                expression.span,
                ctx,
                diagnostics,
            ) && query.limit != Some(1)
            {
                diagnostics.push(Diagnostic::new(
                    format!("xp.query 需要 limit(1) 的单个玩家查询 `{target}`"),
                    expression.span,
                ));
            }
        }
        ExprKind::ScoreQuery { target } => {
            super::super::statements::validate_score_target(
                target,
                expression.span,
                ctx,
                diagnostics,
            );
            if let Holder::Query(name, span) = &target.holder
                && let Some(query) = ctx.symbols.queries.get(name.as_str())
                && query.limit != Some(1)
            {
                diagnostics.push(Diagnostic::new(
                    format!("scoreboard.get 需要 limit(1) 的单个实体查询 `{name}`"),
                    *span,
                ));
            }
        }
        ExprKind::StopwatchQuery { id, .. } => {
            super::super::statements::validate_stopwatch_id(id, expression.span, diagnostics);
        }
        ExprKind::TimeQuery { clock } => {
            if let Some(clock) = clock {
                super::super::registry::validate_id(
                    "world_clock",
                    "世界时钟",
                    clock,
                    expression.span,
                    diagnostics,
                );
            }
        }
        ExprKind::GameTimeQuery | ExprKind::WorldBorderSize => {}
        ExprKind::GameRuleQuery { name } => {
            if !super::super::world::game_rule_exists(name) {
                diagnostics.push(Diagnostic::new(
                    format!("未知游戏规则 `{name}`；规则名来自 26.3 的 GameRules 注册表"),
                    expression.span,
                ));
            }
        }
        ExprKind::Count { query, query_span } => {
            if !ctx.symbols.queries.contains_key(query.as_str()) {
                diagnostics.push(Diagnostic::new(
                    format!("找不到实体查询 `{query}`"),
                    *query_span,
                ));
            }
        }
        ExprKind::Random { min, max } => {
            if !(1..i64::from(i32::MAX)).contains(&(i64::from(*max) - i64::from(*min))) {
                diagnostics.push(Diagnostic::new(
                    format!("random 的范围 {min}..{max} 必须至少包含两个整数，且上下界之差小于 2147483647"),
                    expression.span,
                ));
            }
        }
        ExprKind::DataGet {
            source,
            path,
            path_span,
        } => {
            super::super::components::validate_nbt_source(
                source,
                expression.span,
                ctx,
                diagnostics,
            );
            if !super::super::components::valid_nbt_component_path(path) {
                diagnostics.push(Diagnostic::new(
                    format!("`{path}` 不是有效的 NBT 路径"),
                    *path_span,
                ));
            }
        }
        ExprKind::Compute {
            source,
            kind,
            provider,
            provider_span,
            scale,
        } => {
            validate_compute(
                source,
                *kind,
                provider,
                *provider_span,
                scale.as_deref(),
                expression.span,
                ctx,
                diagnostics,
            );
        }
        ExprKind::Negate(value) => validate_expr(value, locals, ctx, diagnostics),
        ExprKind::Binary {
            left,
            operation,
            right,
        } => {
            validate_expr(left, locals, ctx, diagnostics);
            validate_expr(right, locals, ctx, diagnostics);
            if matches!(operation, BinaryOp::Divide | BinaryOp::Modulo)
                && constant_value(right) == Some(0)
            {
                diagnostics.push(Diagnostic::new("不能除以零", right.span));
            }
        }
    }
}
