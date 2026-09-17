//! 表达式与布尔条件的校验。
//!
//! 这里只负责名称解析、函数调用形状和静态除零；语句结构留在
//! [`super::statements`]，执行上下文通过调用方的 [`ValidationContext`] 传入。

use std::collections::HashSet;

use crate::ast::{
    BinaryOp, CallTarget, ComputeKind, ComputeSource, Condition, DataSource, Expr, ExprKind,
    Holder, ItemConditionSource, Span,
};
use crate::compiler::constant::constant_value;
use crate::compiler::types::{ExecutionContext, Signature};
use crate::diagnostic::Diagnostic;

use super::statements::ValidationContext;

pub(super) fn validate_call_context(
    function: &str,
    signature: Signature,
    span: Span,
    ctx: ValidationContext<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if ctx.context.satisfies(signature.required_context) {
        return;
    }
    let Some((attribute, kind)) = execution_context_label(signature.required_context) else {
        return;
    };
    diagnostics.push(Diagnostic::new(
        format!("{attribute} 函数 `{function}` 需要{kind}执行上下文"),
        span,
    ));
}

/// 执行上下文对应的函数属性与中文描述，供调用点组织诊断文本。
pub(super) fn execution_context_label(
    context: ExecutionContext,
) -> Option<(&'static str, &'static str)> {
    match context {
        ExecutionContext::Player => Some(("@player", "玩家")),
        ExecutionContext::Mob => Some(("@non_player", "非玩家实体")),
        ExecutionContext::Entity => Some(("@entity", "实体")),
        ExecutionContext::None => None,
    }
}

pub(super) fn validate_condition(
    condition: &Condition,
    locals: &HashSet<&str>,
    ctx: ValidationContext<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match condition {
        Condition::Predicate { name, span } => {
            if !ctx.symbols.predicates.contains(name.as_str()) {
                diagnostics.push(Diagnostic::new(
                    format!("找不到 predicate 资源 `{name}`"),
                    *span,
                ));
            }
        }
        Condition::Compare { left, right, .. } => {
            validate_expr(left, locals, ctx, diagnostics);
            validate_expr(right, locals, ctx, diagnostics);
        }
        Condition::Block { pos, block, .. } => {
            super::world::validate_block_position(pos, diagnostics);
            super::world::validate_block_state(block, true, diagnostics);
        }
        Condition::Blocks {
            start,
            end,
            destination,
            ..
        } => {
            super::world::validate_block_position(start, diagnostics);
            super::world::validate_block_position(end, diagnostics);
            super::world::validate_block_position(destination, diagnostics);
        }
        Condition::Biome {
            pos,
            biome,
            biome_span,
            ..
        } => {
            super::world::validate_block_position(pos, diagnostics);
            super::registry::validate_id_or_tag(
                "biome",
                "生物群系",
                biome,
                *biome_span,
                diagnostics,
            );
        }
        Condition::Loaded { pos, .. } => {
            super::world::validate_block_position(pos, diagnostics);
        }
        Condition::Dimension {
            dimension,
            dimension_span,
            ..
        } => {
            super::registry::validate_id(
                "dimension",
                "维度",
                dimension,
                *dimension_span,
                diagnostics,
            );
        }
        Condition::Entity {
            query, query_span, ..
        } => {
            if !ctx.symbols.queries.contains_key(query.as_str()) {
                diagnostics.push(Diagnostic::new(
                    format!("找不到实体查询 `{query}`"),
                    *query_span,
                ));
            }
        }
        Condition::Data {
            source,
            path,
            path_span,
            ..
        } => {
            super::components::validate_nbt_source(
                source,
                condition_span(condition),
                ctx,
                diagnostics,
            );
            if !super::components::valid_nbt_component_path(path) {
                diagnostics.push(Diagnostic::new(
                    format!("`{path}` 不是有效的 NBT 路径"),
                    *path_span,
                ));
            }
        }
        Condition::Items {
            source,
            slots,
            slots_span,
            item,
            item_span,
            ..
        } => {
            validate_item_condition_source(source, condition_span(condition), ctx, diagnostics);
            validate_slot_source(slots, *slots_span, diagnostics);
            if !valid_item_predicate(item) {
                diagnostics.push(Diagnostic::new(
                    format!("`{item}` 不是有效的物品谓词（物品 id、`#标签` 或带组件过滤器的 id）"),
                    *item_span,
                ));
            }
        }
        Condition::Slots {
            source,
            slots,
            slots_span,
            ..
        } => {
            validate_item_condition_source(source, condition_span(condition), ctx, diagnostics);
            validate_slot_source(slots, *slots_span, diagnostics);
        }
        Condition::Function { target, span } => match target {
            CallTarget::Function(name) => match ctx.symbols.functions.get(name.as_str()) {
                None => diagnostics.push(Diagnostic::new(format!("找不到函数 `{name}`；如果它来自其他模块，请确认对方声明了 `export`，并在本模块 `import` 它"), *span)),
                Some(signature) => {
                    validate_call_context(name, *signature, *span, ctx, diagnostics);
                }
            },
            CallTarget::Tag(tag) => {
                if !ctx.symbols.function_tags.contains_key(tag.as_str()) {
                    diagnostics.push(Diagnostic::new(format!("找不到函数标签 `#{tag}`"), *span));
                }
            }
        },
        Condition::Stopwatch { id, span } => {
            super::statements::validate_stopwatch_id(id, *span, diagnostics);
        }
        Condition::Not(condition) => validate_condition(condition, locals, ctx, diagnostics),
        Condition::And(left, right) | Condition::Or(left, right) => {
            validate_condition(left, locals, ctx, diagnostics);
            validate_condition(right, locals, ctx, diagnostics);
        }
    }
}

/// 条件整体的源位置：用于没有独立 span 的子节点诊断。
fn condition_span(condition: &Condition) -> Span {
    match condition {
        Condition::Predicate { span, .. }
        | Condition::Block { span, .. }
        | Condition::Blocks { span, .. }
        | Condition::Biome { span, .. }
        | Condition::Loaded { span, .. }
        | Condition::Dimension { span, .. }
        | Condition::Entity { span, .. }
        | Condition::Data { span, .. }
        | Condition::Items { span, .. }
        | Condition::Slots { span, .. }
        | Condition::Function { span, .. }
        | Condition::Stopwatch { span, .. } => *span,
        Condition::Not(inner) => condition_span(inner),
        Condition::And(left, _) | Condition::Or(left, _) => condition_span(left),
        Condition::Compare { left, .. } => left.span,
    }
}

pub(super) fn validate_item_condition_source(
    source: &ItemConditionSource,
    span: Span,
    ctx: ValidationContext<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match source {
        ItemConditionSource::Entity(holder) => {
            super::statements::validate_holder(holder, span, ctx, diagnostics);
        }
        ItemConditionSource::Block(position) => {
            super::world::validate_block_position(position, diagnostics);
        }
    }
}

/// 槽位来源：槽位名（含 `prefix.N` 与通配）或 `slot_source` 资源位置。
pub(super) fn validate_slot_source(slots: &str, span: Span, diagnostics: &mut Vec<Diagnostic>) {
    let snapshot = crate::version::snapshot::snapshot();
    if !snapshot.slots().accepts(slots) && !super::rules::valid_resource_location(slots) {
        diagnostics.push(Diagnostic::new(
            format!("`{slots}` 不是有效的槽位来源（槽位名或 slot_source 资源位置）"),
            span,
        ));
    }
}

/// 物品谓词：物品 id、`#标签`，可带 `[组件过滤器]`。
fn valid_item_predicate(text: &str) -> bool {
    if text.is_empty() || text.contains(char::is_whitespace) {
        return false;
    }
    let base = text.strip_prefix('#').unwrap_or(text);
    let location = base.split('[').next().unwrap_or(base);
    super::rules::valid_resource_location(location)
}

/// `compute` 的公共校验（表达式与 `data.modify` 的 compute 来源共用）。
#[allow(clippy::too_many_arguments)]
pub(super) fn validate_compute(
    source: &ComputeSource,
    kind: ComputeKind,
    provider: &str,
    provider_span: Span,
    scale: Option<&str>,
    span: Span,
    ctx: ValidationContext<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match source {
        ComputeSource::Default => {}
        ComputeSource::Block(position) => {
            super::world::validate_block_position(position, diagnostics);
        }
        ComputeSource::Entity(holder) => {
            if matches!(holder, Holder::Origin) {
                diagnostics.push(Diagnostic::new(
                    "compute 的 entity 来源不能是投掷者；请用 self/自身 或实体查询",
                    span,
                ));
            } else {
                super::statements::validate_holder(holder, span, ctx, diagnostics);
            }
        }
    }
    let registry = match kind {
        ComputeKind::Float => "context_float_provider",
        ComputeKind::Integer => "context_int_provider",
    };
    let label = match kind {
        ComputeKind::Float => "浮点 provider",
        ComputeKind::Integer => "整数 provider",
    };
    super::registry::validate_id(registry, label, provider, provider_span, diagnostics);
    if let Some(scale) = scale
        && !scale
            .parse::<f64>()
            .is_ok_and(|value| value.is_finite() && value != 0.0)
    {
        diagnostics.push(Diagnostic::new("compute 的缩放必须是非零数字", span));
    }
}

/// 数据来源的命令文本校验。
pub(super) fn validate_data_source(
    source: &DataSource,
    span: Span,
    ctx: ValidationContext<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match source {
        DataSource::From {
            target,
            path,
            path_span,
        }
        | DataSource::String {
            target,
            path,
            path_span,
            ..
        } => {
            super::components::validate_nbt_source(target, span, ctx, diagnostics);
            if !super::components::valid_nbt_component_path(path) {
                diagnostics.push(Diagnostic::new(
                    format!("`{path}` 不是有效的 NBT 路径"),
                    *path_span,
                ));
            }
        }
        DataSource::Value(_) => {}
        DataSource::Compute {
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
                span,
                ctx,
                diagnostics,
            );
        }
    }
}

pub(super) fn validate_expr(
    expression: &Expr,
    locals: &HashSet<&str>,
    ctx: ValidationContext<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match &expression.kind {
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
            if let Some(query) =
                super::statements::require_player_query(target, expression.span, ctx, diagnostics)
                && query.limit != Some(1)
            {
                diagnostics.push(Diagnostic::new(
                    format!("xp.query 需要 limit(1) 的单个玩家查询 `{target}`"),
                    expression.span,
                ));
            }
        }
        ExprKind::ScoreQuery { target } => {
            super::statements::validate_score_target(target, expression.span, ctx, diagnostics);
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
            super::statements::validate_stopwatch_id(id, expression.span, diagnostics);
        }
        ExprKind::TimeQuery { clock } => {
            if let Some(clock) = clock {
                super::registry::validate_id(
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
            if !super::world::game_rule_exists(name) {
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
            if min > max {
                diagnostics.push(Diagnostic::new(
                    format!("random 的最小值 {min} 不能大于最大值 {max}"),
                    expression.span,
                ));
            }
        }
        ExprKind::DataGet {
            source,
            path,
            path_span,
        } => {
            super::components::validate_nbt_source(source, expression.span, ctx, diagnostics);
            if !super::components::valid_nbt_component_path(path) {
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
