use super::*;

pub(super) fn validate_return<'a>(
    kind: &'a ReturnKind,
    locals: &HashSet<&'a str>,
    span: Span,
    ctx: ValidationContext<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !ctx.return_rules.allowed_here {
        diagnostics.push(Diagnostic::new(
            "return 只能直接出现在函数代码块中，不能放在 if、while、each、spawn、in_dimension 或 execute 块内",
            span,
        ));
    }
    match (ctx.return_rules.returns_score, kind) {
        (true, ReturnKind::Value(value)) => validate_expr(value, locals, ctx, diagnostics),
        (_, ReturnKind::Run(command)) => validate_raw_command(command, span, diagnostics),
        (true, ReturnKind::Fail) => {}
        (true, ReturnKind::Void) => diagnostics.push(Diagnostic::new(
            "返回 score 的函数需要 `return <表达式>;`、`return run \"命令\";` 或 `return fail;`",
            span,
        )),
        (false, ReturnKind::Value(_)) => diagnostics.push(Diagnostic::new(
            "无返回值函数只能使用 `return;`、`return fail;` 或 `return run \"命令\";`",
            span,
        )),
        (false, ReturnKind::Void | ReturnKind::Fail) => {}
    }
}
pub(super) fn validate_call<'a>(
    function: &str,
    arguments: &'a [Expr],
    locals: &HashSet<&'a str>,
    span: Span,
    ctx: ValidationContext<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let Some(signature) = ctx.symbols.functions.get(function) {
        if signature.is_macro {
            diagnostics.push(Diagnostic::new(
                format!("宏函数 `{function}` 需要 nbt 参数或 with 来源，不能使用计分实参调用"),
                span,
            ));
        }
        if arguments.len() != signature.parameters {
            diagnostics.push(Diagnostic::new(
                format!(
                    "函数 `{function}` 需要 {} 个参数，实际提供 {} 个",
                    signature.parameters,
                    arguments.len()
                ),
                span,
            ));
        }
        validate_call_context(function, *signature, span, ctx, diagnostics);
    } else {
        diagnostics.push(Diagnostic::new(format!("找不到函数 `{function}`；如果它来自其他模块，请确认对方声明了 `export`，并在本模块 `import` 它"), span));
    }
    for argument in arguments {
        validate_expr(argument, locals, ctx, diagnostics);
    }
}

pub(super) fn validate_schedule(
    function: &str,
    span: Span,
    ctx: ValidationContext<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match ctx.symbols.functions.get(function) {
        None => diagnostics.push(Diagnostic::new(format!("找不到函数 `{function}`；如果它来自其他模块，请确认对方声明了 `export`，并在本模块 `import` 它"), span)),
        Some(signature) if signature.required_context != ExecutionContext::None => diagnostics
            .push(Diagnostic::new(
                format!("不能调度需要执行上下文的函数 `{function}`，调度不会保留实体或玩家"),
                span,
            )),
        Some(signature) if signature.parameters == 0 => {}
        Some(signature) => diagnostics.push(Diagnostic::new(
            format!(
                "不能调度需要 {} 个参数的函数 `{function}`",
                signature.parameters
            ),
            span,
        )),
    }
}

pub(super) fn validate_assign<'a>(
    target: &str,
    operation: AssignOp,
    value: &'a Expr,
    locals: &HashSet<&'a str>,
    span: Span,
    ctx: ValidationContext<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !ctx.symbols.scores.contains(target)
        && !ctx.symbols.parameters.contains(target)
        && !locals.contains(target)
    {
        diagnostics.push(Diagnostic::new(format!("找不到计分变量 `{target}`"), span));
    }
    validate_expr(value, locals, ctx, diagnostics);
    if matches!(operation, AssignOp::Divide | AssignOp::Modulo) && constant_value(value) == Some(0)
    {
        diagnostics.push(Diagnostic::new("不能除以零", value.span));
    }
}

pub(super) fn validate_let<'a>(
    name: &'a str,
    value: &'a Expr,
    locals: &mut HashSet<&'a str>,
    ctx: ValidationContext<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    validate_expr(value, locals, ctx, diagnostics);
    locals.insert(name);
}

pub(super) fn validate_if<'a>(
    condition: &Condition,
    then_body: &'a [Statement],
    else_body: &'a [Statement],
    locals: &HashSet<&'a str>,
    ctx: ValidationContext<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    validate_condition(condition, locals, ctx, diagnostics);
    let mut then_locals = locals.clone();
    validate_statements(
        then_body,
        &mut then_locals,
        ctx.symbols,
        ctx.context,
        ctx.entity_type,
        ctx.return_rules.nested(),
        diagnostics,
    );
    let mut else_locals = locals.clone();
    validate_statements(
        else_body,
        &mut else_locals,
        ctx.symbols,
        ctx.context,
        ctx.entity_type,
        ctx.return_rules.nested(),
        diagnostics,
    );
}

pub(super) fn validate_while<'a>(
    condition: &Condition,
    body: &'a [Statement],
    locals: &HashSet<&'a str>,
    ctx: ValidationContext<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    validate_condition(condition, locals, ctx, diagnostics);
    let mut body_locals = locals.clone();
    validate_statements(
        body,
        &mut body_locals,
        ctx.symbols,
        ctx.context,
        ctx.entity_type,
        ctx.return_rules.in_loop(),
        diagnostics,
    );
}

pub(super) fn validate_for<'a>(
    variable: &'a str,
    start: &'a Expr,
    end: &'a Expr,
    body: &'a [Statement],
    locals: &HashSet<&'a str>,
    ctx: ValidationContext<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    validate_expr(start, locals, ctx, diagnostics);
    validate_expr(end, locals, ctx, diagnostics);
    let mut body_locals = locals.clone();
    body_locals.insert(variable);
    validate_statements(
        body,
        &mut body_locals,
        ctx.symbols,
        ctx.context,
        ctx.entity_type,
        ctx.return_rules.in_loop(),
        diagnostics,
    );
}
