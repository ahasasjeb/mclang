use super::*;

pub(super) fn validate_execute<'a>(
    clauses: &'a ExecuteClauses,
    body: &'a [Statement],
    span: Span,
    locals: &HashSet<&'a str>,
    ctx: ValidationContext<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let ExecuteClauses::Structured(clauses) = clauses else {
        // 字符串子句原样转发，权限等底层检查已由 run 语句族覆盖。
        validate_execute_body(body, locals, ctx, ctx.context, ctx.entity_type, diagnostics);
        return;
    };

    let mut context = ctx.context;
    let mut entity_type = ctx.entity_type;
    let mut seen: HashSet<&'static str> = HashSet::new();
    // 0 = 修饰符，1 = 条件，2 = store；子句只能按这个顺序推进。
    let mut stage = 0_u8;
    let mut has_stores = false;
    for clause in clauses {
        let clause_ctx = ValidationContext {
            context,
            entity_type,
            ..ctx
        };
        match &clause.kind {
            ExecuteClauseKind::As { query, query_span } => {
                if !mark_execute_modifier(&mut seen, "as", clause.span, stage, diagnostics) {
                    continue;
                }
                match query_reference(query, *query_span, clause_ctx, diagnostics) {
                    Some(declaration) => {
                        entity_type = Some(declaration.entity_type.as_str());
                        context = if declaration.entity_type == "minecraft:player" {
                            ExecutionContext::Player
                        } else {
                            ExecutionContext::Mob
                        };
                    }
                    None => {
                        entity_type = None;
                        context = ExecutionContext::Entity;
                    }
                }
            }
            ExecuteClauseKind::At { query, query_span } => {
                if !mark_execute_modifier(&mut seen, "at", clause.span, stage, diagnostics) {
                    continue;
                }
                validate_execute_query_selector(
                    query,
                    *query_span,
                    clause.span,
                    clause_ctx,
                    diagnostics,
                );
            }
            ExecuteClauseKind::Positioned(position) => {
                if mark_execute_modifier(&mut seen, "positioned", clause.span, stage, diagnostics) {
                    crate::compiler::validate::world::validate_position_value(
                        position,
                        diagnostics,
                    );
                }
            }
            ExecuteClauseKind::Rotated(_) => {
                mark_execute_modifier(&mut seen, "rotated", clause.span, stage, diagnostics);
            }
            ExecuteClauseKind::FacingPosition(position) => {
                if mark_execute_modifier(&mut seen, "facing", clause.span, stage, diagnostics) {
                    crate::compiler::validate::world::validate_position_value(
                        position,
                        diagnostics,
                    );
                }
            }
            ExecuteClauseKind::FacingEntity {
                query, query_span, ..
            } => {
                if mark_execute_modifier(&mut seen, "facing", clause.span, stage, diagnostics) {
                    validate_execute_query_selector(
                        query,
                        *query_span,
                        clause.span,
                        clause_ctx,
                        diagnostics,
                    );
                }
            }
            ExecuteClauseKind::Align { .. } => {
                mark_execute_modifier(&mut seen, "align", clause.span, stage, diagnostics);
            }
            ExecuteClauseKind::Anchored(_) => {
                mark_execute_modifier(&mut seen, "anchored", clause.span, stage, diagnostics);
            }
            ExecuteClauseKind::In {
                dimension,
                dimension_span,
            } => {
                if mark_execute_modifier(&mut seen, "in", clause.span, stage, diagnostics) {
                    validate_id("dimension", "维度", dimension, *dimension_span, diagnostics);
                }
            }
            ExecuteClauseKind::On(_) => {
                if mark_execute_modifier(&mut seen, "on", clause.span, stage, diagnostics)
                    && !context.is_entity()
                {
                    diagnostics.push(Diagnostic::new(
                        "execute on 需要当前存在执行实体：on 会读取 @s 的实体关系，请先写 as(查询) 或放进实体上下文",
                        clause.span,
                    ));
                }
                // 关系实体可能是玩家，也可能是非玩家，按最宽的实体上下文继续。
                context = ExecutionContext::Entity;
                entity_type = None;
            }
            ExecuteClauseKind::Summon {
                entity_type: summoned,
                entity_type_span,
            } => {
                if !mark_execute_modifier(&mut seen, "summon", clause.span, stage, diagnostics) {
                    continue;
                }
                validate_id(
                    "entity_type",
                    "实体类型",
                    summoned,
                    *entity_type_span,
                    diagnostics,
                );
                if valid_resource_location(summoned) && non_summonable_entity(summoned) {
                    diagnostics.push(Diagnostic::new(
                        format!("Minecraft 的 /summon 不支持实体类型 `{summoned}`"),
                        clause.span,
                    ));
                }
                context = ExecutionContext::Mob;
                entity_type = Some(summoned.as_str());
            }
            ExecuteClauseKind::If(condition) | ExecuteClauseKind::Unless(condition) => {
                if stage > 1 {
                    diagnostics.push(Diagnostic::new(
                        "execute 的条件必须写在 store 之前",
                        clause.span,
                    ));
                }
                stage = stage.max(1);
                validate_condition(condition, locals, clause_ctx, diagnostics);
            }
            ExecuteClauseKind::StoreResult(target) | ExecuteClauseKind::StoreSuccess(target) => {
                stage = 2;
                has_stores = true;
                validate_store_target(target, clause.span, clause_ctx, diagnostics);
            }
            ExecuteClauseKind::StoreData(data) => {
                stage = 2;
                has_stores = true;
                validate_store_data(data, clause.span, clause_ctx, diagnostics);
            }
        }
    }

    if has_stores && let Some(last) = body.last() {
        match &last.kind {
            StatementKind::If { .. }
            | StatementKind::While { .. }
            | StatementKind::Each { .. }
            | StatementKind::InDimension { .. }
            | StatementKind::Spawn { .. }
            | StatementKind::Execute { .. } => diagnostics.push(Diagnostic::new(
                "execute store 捕获块内最后一条命令的结果；if/while/each/spawn/in_dimension/execute \
                 作为最后一条语句时结果不会传递，请把要捕获的命令放到块末尾",
                last.span,
            )),
            StatementKind::Call {
                target: CallTarget::Function(name),
                ..
            } => {
                if let Some(signature) = ctx.symbols.functions.get(name.as_str())
                    && !signature.returns_score
                {
                    diagnostics.push(Diagnostic::new(
                        format!(
                            "execute store 捕获的是返回值，而函数 `{name}` 没有声明返回 score；\
                             请调用有返回值的函数，或把要捕获的命令放到块末尾"
                        ),
                        last.span,
                    ));
                }
            }
            _ => {}
        }
    }
    if has_stores && body.is_empty() {
        diagnostics.push(Diagnostic::new(
            "execute store 需要块内至少一条命令来产生结果",
            span,
        ));
    }

    validate_execute_body(body, locals, ctx, context, entity_type, diagnostics);
}

/// 结构化 execute 的块体校验：上下文由子句推导，`return` 仍然被拒绝。
pub(super) fn validate_execute_body<'a>(
    body: &'a [Statement],
    locals: &HashSet<&'a str>,
    ctx: ValidationContext<'_, '_>,
    context: ExecutionContext,
    entity_type: Option<&'a str>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut body_locals = locals.clone();
    validate_statements(
        body,
        &mut body_locals,
        ctx.symbols,
        context,
        entity_type,
        ctx.return_rules.nested(),
        diagnostics,
    );
}

/// `at`/`facing entity` 只转发实体选择器，无法应用查询自带的物品过滤。
pub(super) fn validate_execute_query_selector(
    query: &str,
    query_span: Span,
    clause_span: Span,
    ctx: ValidationContext<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let Some(declaration) = query_reference(query, query_span, ctx, diagnostics)
        && declaration.item.is_some()
    {
        diagnostics.push(Diagnostic::new(
            format!(
                "查询 `{query}` 带物品过滤，但 at/facing entity 只转发实体选择器，无法追加物品条件；\
                 请改用 as 子句或去掉过滤"
            ),
            clause_span,
        ));
    }
}

/// 记录一次修饰符出现：重复或出现在条件之后时报错并返回假。
pub(super) fn mark_execute_modifier(
    seen: &mut HashSet<&'static str>,
    name: &'static str,
    span: Span,
    stage: u8,
    diagnostics: &mut Vec<Diagnostic>,
) -> bool {
    if stage > 0 {
        diagnostics.push(Diagnostic::new(
            "execute 的修饰符必须写在 if/unless 条件之前",
            span,
        ));
        return false;
    }
    if !seen.insert(name) {
        diagnostics.push(Diagnostic::new(
            format!("execute 的 {name} 子句只能出现一次"),
            span,
        ));
        return false;
    }
    true
}

/// `store.result/success` 的目标：用户计分板或 Boss 栏。
pub(super) fn validate_store_target(
    target: &ExecuteStoreTarget,
    span: Span,
    ctx: ValidationContext<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match target {
        // origin 持有者的写入由代码生成拆成「临时项捕获 + on origin 复制」，
        // 这里只校验持有者与目标本身。
        ExecuteStoreTarget::Score(score) => validate_score_target(score, span, ctx, diagnostics),
        ExecuteStoreTarget::BossBar { id, id_span, .. } => {
            if !valid_resource_location(id) {
                diagnostics.push(Diagnostic::new(
                    format!("`{id}` 不是有效的 Boss 栏资源位置"),
                    *id_span,
                ));
            }
        }
    }
}

/// `store.data` 的 NBT 目标：路径、来源与实体写保护。
pub(super) fn validate_store_data(
    data: &ExecuteStoreData,
    span: Span,
    ctx: ValidationContext<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !crate::compiler::validate::components::valid_nbt_component_path(&data.path) {
        diagnostics.push(Diagnostic::new(
            format!("`{}` 不是有效的 NBT 路径", data.path),
            data.path_span,
        ));
    }
    match &data.source {
        NbtComponentSource::Entity(holder) => {
            match holder {
                Holder::SelfEntity => {
                    if ctx.context != ExecutionContext::Mob {
                        diagnostics.push(Diagnostic::new(
                            "store.data 通过 data 命令写入实体 NBT，Minecraft 不允许修改玩家数据；只能在确定不是玩家的实体上下文中使用（非玩家查询的 each、非玩家 spawn，或 @non_player 函数）",
                            span,
                        ));
                        return;
                    }
                }
                Holder::Query(name, query_span) => {
                    if let Some(query) = ctx.symbols.queries.get(name.as_str()) {
                        if query.limit != Some(1) {
                            diagnostics.push(Diagnostic::new(
                                format!(
                                    "store.data 的实体来源需要 limit(1) 的单个实体查询 `{name}`"
                                ),
                                *query_span,
                            ));
                        } else if query.entity_type == "minecraft:player" {
                            diagnostics.push(Diagnostic::new(
                                format!(
                                    "store.data 不能写入玩家：查询 `{name}` 匹配 minecraft:player"
                                ),
                                *query_span,
                            ));
                            return;
                        }
                    }
                }
                // 投掷者的类型无法静态确认：运行期是玩家时 Minecraft 静默拒绝写入，
                // 与非玩家的成功路径共用同一份代码，不在编译期强制拒绝。
                Holder::Origin => {}
            }
            validate_holder(holder, span, ctx, diagnostics);
        }
        NbtComponentSource::Block(position) => {
            crate::compiler::validate::world::validate_block_position(position, diagnostics);
        }
        NbtComponentSource::Storage(storage, storage_span) => {
            if !valid_resource_location(storage) {
                diagnostics.push(Diagnostic::new(
                    format!("`{storage}` 不是有效的存储资源位置"),
                    *storage_span,
                ));
            }
        }
    }
}

/// 函数权限模型（1.6）：`run` 字符串里的根命令不得越过数据包函数固定的
/// GAMEMASTER 等级（2）。未知根命令留给后续的命令树校验（8.4）。
pub(super) fn validate_raw_command(command: &str, span: Span, diagnostics: &mut Vec<Diagnostic>) {
    let Some(first) = command.split_whitespace().next() else {
        return;
    };
    let name = first.strip_prefix('/').unwrap_or(first);
    let snapshot = crate::version::snapshot::snapshot();
    let Some(level) = snapshot.root_command_level(name) else {
        return;
    };
    let limit = crate::compiler::FUNCTION_PERMISSION_LEVEL;
    if level > limit {
        diagnostics.push(Diagnostic::new(
            format!(
                "`{name}` 需要权限等级 {level}（{}），超过数据包函数上限 {limit}（{}）",
                permission_label(level),
                permission_label(limit),
            ),
            span,
        ));
    }
}

/// 权限等级的中文标签。
pub(super) fn permission_label(level: u8) -> &'static str {
    match level {
        0 => "所有人",
        1 => "MODERATOR",
        2 => "GAMEMASTER",
        3 => "ADMIN",
        4 => "OWNER",
        _ => "未知",
    }
}
