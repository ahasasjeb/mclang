use super::*;

pub(super) fn validate_tag_call<'a>(
    tag: &str,
    arguments: &'a [Expr],
    locals: &HashSet<&'a str>,
    span: Span,
    ctx: ValidationContext<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match ctx.symbols.function_tags.get(tag) {
        None => diagnostics.push(Diagnostic::new(format!("找不到函数标签 `{tag}`"), span)),
        Some(_) if !arguments.is_empty() => diagnostics.push(Diagnostic::new(
            "函数标签调用不接受参数，标签不能传递实参",
            span,
        )),
        Some(_) => {
            for function in reachable_functions(tag, ctx.symbols.function_tags) {
                let Some(signature) = ctx.symbols.functions.get(function) else {
                    continue;
                };
                if signature.parameters > 0 {
                    diagnostics.push(Diagnostic::new(
                        format!(
                            "函数标签 `{tag}` 中的函数 `{function}` 需要 {} 个参数，标签调用无法传递",
                            signature.parameters
                        ),
                        span,
                    ));
                }
                if !ctx.context.satisfies(signature.required_context)
                    && let Some((attribute, kind)) =
                        execution_context_label(signature.required_context)
                {
                    diagnostics.push(Diagnostic::new(
                        format!(
                            "函数标签 `{tag}` 中的 {attribute} 函数 `{function}` 需要{kind}执行上下文"
                        ),
                        span,
                    ));
                }
            }
        }
    }
    for argument in arguments {
        validate_expr(argument, locals, ctx, diagnostics);
    }
}

pub(super) fn validate_tag_schedule(
    tag: &str,
    span: Span,
    ctx: ValidationContext<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !ctx.symbols.function_tags.contains_key(tag) {
        diagnostics.push(Diagnostic::new(format!("找不到函数标签 `{tag}`"), span));
        return;
    }
    for function in reachable_functions(tag, ctx.symbols.function_tags) {
        let Some(signature) = ctx.symbols.functions.get(function) else {
            continue;
        };
        if signature.required_context != ExecutionContext::None {
            diagnostics.push(Diagnostic::new(
                format!(
                    "不能调度函数标签 `{tag}` 中需要执行上下文的函数 `{function}`，调度不会保留实体或玩家"
                ),
                span,
            ));
        }
        if signature.parameters > 0 {
            diagnostics.push(Diagnostic::new(
                format!(
                    "不能调度函数标签 `{tag}` 中需要 {} 个参数的函数 `{function}`",
                    signature.parameters
                ),
                span,
            ));
        }
    }
}
