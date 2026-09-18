use super::*;

/// 解析实体查询引用；未声明时报告并返回 `None`。
pub(super) fn query_reference<'b>(
    name: &str,
    span: Span,
    ctx: ValidationContext<'_, 'b>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<&'b EntityQueryDecl> {
    match ctx.symbols.queries.get(name).copied() {
        Some(query) => Some(query),
        None => {
            diagnostics.push(Diagnostic::new(format!("找不到实体查询 `{name}`"), span));
            None
        }
    }
}

/// 解析并要求查询匹配玩家；命令目标是玩家而查询不匹配时报告。
pub(in crate::compiler::validate) fn require_player_query<'b>(
    name: &str,
    span: Span,
    ctx: ValidationContext<'_, 'b>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<&'b EntityQueryDecl> {
    let query = query_reference(name, span, ctx, diagnostics)?;
    if query.entity_type != "minecraft:player" {
        diagnostics.push(Diagnostic::new(
            format!("查询 `{name}` 必须匹配 minecraft:player"),
            span,
        ));
    }
    Some(query)
}

/// 秒表 id 的公共校验，供语句与表达式共用。
pub(in crate::compiler::validate) fn validate_stopwatch_id(
    id: &str,
    span: Span,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !valid_resource_location(id) {
        diagnostics.push(Diagnostic::new(
            format!("`{id}` 不是有效的秒表资源位置"),
            span,
        ));
    }
}
