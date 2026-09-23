use super::*;

pub(super) fn validate_statement<'a>(
    statement: &'a Statement,
    locals: &mut HashSet<&'a str>,
    ctx: ValidationContext<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match &statement.kind {
        StatementKind::MacroCall { target, arguments } => {
            crate::compiler::validate::macros::validate_macro_call(
                target,
                arguments,
                statement.span,
                ctx,
                diagnostics,
            )
        }
        StatementKind::CoreCommand(command) => {
            crate::compiler::validate::core_commands::validate_core_command(
                command,
                statement.span,
                ctx,
                diagnostics,
            )
        }
        StatementKind::EntityCommand(command) => {
            crate::compiler::validate::entity_commands::validate_entity_command(
                command,
                statement.span,
                ctx,
                diagnostics,
            )
        }
        StatementKind::Run(command) => {
            validate_raw_command(command, statement.span, diagnostics);
        }
        StatementKind::Give {
            target,
            item,
            count,
            count_span,
        } => validate_give_statement(
            target,
            item,
            *count,
            *count_span,
            statement.span,
            ctx,
            diagnostics,
        ),
        StatementKind::SelfAction(action) => {
            validate_self_action(action, statement.span, ctx, diagnostics);
        }
        StatementKind::EffectGive {
            target,
            effect,
            duration,
            amplifier,
            ..
        } => validate_effect_give(
            target,
            effect,
            *duration,
            *amplifier,
            statement.span,
            ctx,
            diagnostics,
        ),
        StatementKind::EffectClear { target, effect } => {
            validate_effect_clear(target, effect.as_deref(), statement.span, ctx, diagnostics);
        }
        StatementKind::XpChange {
            target,
            operation,
            amount,
            ..
        } => validate_xp_change(
            target,
            *operation,
            *amount,
            statement.span,
            ctx,
            diagnostics,
        ),
        StatementKind::StopwatchAction { id, .. } => {
            validate_stopwatch_id(id, statement.span, diagnostics);
        }
        StatementKind::ClearInventory {
            target,
            item,
            max_count,
        } => validate_clear_inventory(
            target,
            item.as_ref(),
            *max_count,
            statement.span,
            ctx,
            diagnostics,
        ),
        StatementKind::Message { target, component } => {
            validate_message(target, component, statement.span, ctx, diagnostics);
        }
        StatementKind::UiCommand(command) => {
            validate_ui_command(command, statement.span, ctx, diagnostics);
        }
        StatementKind::PlaySound {
            sound,
            source,
            targets,
            position,
            volume,
            pitch,
            min_volume,
        } => {
            validate_play_sound(
                sound,
                source,
                targets.as_deref(),
                position.as_ref(),
                volume.as_deref(),
                pitch.as_deref(),
                min_volume.as_deref(),
                statement.span,
                ctx,
                diagnostics,
            );
        }
        StatementKind::Each { query, body } => {
            validate_each(query, body, locals, statement.span, ctx, diagnostics);
        }
        StatementKind::InDimension { dimension, body } => {
            validate_in_dimension(dimension, body, locals, statement.span, ctx, diagnostics);
        }
        StatementKind::Spawn {
            entity_type,
            position,
            body,
        } => {
            validate_spawn(
                entity_type,
                position.as_ref(),
                body,
                locals,
                statement.span,
                ctx,
                diagnostics,
            );
        }
        StatementKind::Return(kind) => {
            validate_return(kind, locals, statement.span, ctx, diagnostics);
        }
        StatementKind::Call { target, arguments } => match target {
            CallTarget::External(id) => {
                crate::compiler::validate::macros::validate_external_function(
                    id,
                    statement.span,
                    diagnostics,
                );
                if !arguments.is_empty() {
                    diagnostics.push(Diagnostic::new(
                        "外部函数不接受计分 ABI 参数；请使用 nbt 宏参数",
                        statement.span,
                    ));
                }
            }
            CallTarget::Function(function) => {
                validate_call(
                    function,
                    arguments,
                    locals,
                    statement.span,
                    ctx,
                    diagnostics,
                );
            }
            CallTarget::Tag(tag) => {
                validate_tag_call(tag, arguments, locals, statement.span, ctx, diagnostics);
            }
        },
        StatementKind::Schedule { target, .. } => match target {
            CallTarget::External(id) => {
                crate::compiler::validate::macros::validate_external_function(
                    id,
                    statement.span,
                    diagnostics,
                )
            }
            CallTarget::Function(function) => {
                validate_schedule(function, statement.span, ctx, diagnostics);
            }
            CallTarget::Tag(tag) => {
                validate_tag_schedule(tag, statement.span, ctx, diagnostics);
            }
        },
        StatementKind::ScheduleClear { target } => match target {
            CallTarget::Function(function) => {
                validate_schedule(function, statement.span, ctx, diagnostics)
            }
            CallTarget::External(id) if !id.starts_with('#') => {
                crate::compiler::validate::macros::validate_external_function(
                    id,
                    statement.span,
                    diagnostics,
                )
            }
            _ => diagnostics.push(Diagnostic::new(
                "schedule.clear 只接受函数资源位置，原版不能清除 #标签 调度",
                statement.span,
            )),
        },
        StatementKind::Assign {
            target,
            operation,
            value,
        } => {
            validate_assign(
                target,
                *operation,
                value,
                locals,
                statement.span,
                ctx,
                diagnostics,
            );
        }
        StatementKind::ScoreSet { target, value } => {
            validate_score_target(target, statement.span, ctx, diagnostics);
            validate_expr(value, locals, ctx, diagnostics);
        }
        StatementKind::ScoreReset { target } => {
            validate_score_target(target, statement.span, ctx, diagnostics);
        }
        StatementKind::ScoreboardEnable { target } => {
            validate_score_target(target, statement.span, ctx, diagnostics);
            if let Some(declaration) = ctx
                .symbols
                .objective_declarations
                .get(target.objective.as_str())
                && declaration.criteria.as_deref() != Some("trigger")
            {
                diagnostics.push(Diagnostic::new(
                    format!(
                        "scoreboard.enable 只对 criteria = \"trigger\" 的目标有意义，`{}` 的准则是 `{}`",
                        target.objective,
                        declaration.criteria.as_deref().unwrap_or("dummy")
                    ),
                    statement.span,
                ));
            }
        }
        StatementKind::ScoreboardOperation {
            result,
            operation: _,
            source,
        } => {
            validate_score_target(result, statement.span, ctx, diagnostics);
            validate_score_target(source, statement.span, ctx, diagnostics);
            for target in [result, source] {
                if matches!(target.holder, Holder::Origin) {
                    diagnostics.push(Diagnostic::new(
                        "scoreboard.operation 的持有者不能是投掷者；请用 self/自身 或实体查询",
                        statement.span,
                    ));
                }
            }
        }
        StatementKind::ScoreboardDisplay {
            slot,
            slot_span,
            objective,
        } => {
            validate_enum("display_slot", "显示槽", slot, *slot_span, diagnostics);
            if let Some((name, name_span)) = objective
                && !ctx.symbols.objectives.contains(name.as_str())
            {
                diagnostics.push(Diagnostic::new(
                    format!("找不到计分板目标 `{name}`；先声明 `objective {name};`"),
                    *name_span,
                ));
            }
        }
        StatementKind::ScoreboardCommand(command) => {
            crate::compiler::validate::scoreboard::validate_scoreboard_command(
                command,
                statement.span,
                ctx,
                diagnostics,
            );
        }
        StatementKind::Teleport {
            targets,
            destination,
            rotation,
        } => {
            validate_teleport(
                targets,
                destination,
                rotation.as_ref(),
                statement.span,
                ctx,
                diagnostics,
            );
        }
        StatementKind::DataMerge { target, nbt } => {
            crate::compiler::validate::components::validate_writable_data_nbt_target(
                target,
                statement.span,
                ctx,
                diagnostics,
            );
            if !matches!(nbt.kind, NbtValueKind::Compound(_)) {
                diagnostics.push(Diagnostic::new(
                    "data.merge 需要复合标签 `nbt { ... }`（中文 `数据 { ... }`）",
                    statement.span,
                ));
            }
        }
        StatementKind::DataRemove {
            target,
            path,
            path_span,
        } => {
            crate::compiler::validate::components::validate_writable_data_nbt_target(
                target,
                statement.span,
                ctx,
                diagnostics,
            );
            if !crate::compiler::validate::components::valid_nbt_component_path(path) {
                diagnostics.push(Diagnostic::new(
                    format!("`{path}` 不是有效的 NBT 路径"),
                    *path_span,
                ));
            }
        }
        StatementKind::DataModify {
            target,
            path,
            path_span,
            operation,
        } => {
            crate::compiler::validate::components::validate_writable_data_nbt_target(
                target,
                statement.span,
                ctx,
                diagnostics,
            );
            if !crate::compiler::validate::components::valid_nbt_component_path(path) {
                diagnostics.push(Diagnostic::new(
                    format!("`{path}` 不是有效的 NBT 路径"),
                    *path_span,
                ));
            }
            crate::compiler::validate::expressions::validate_data_source(
                &operation.source,
                statement.span,
                ctx,
                diagnostics,
            );
            if let DataSource::String {
                start: Some(start),
                end: Some(end),
                ..
            } = &operation.source
                && start > end
            {
                diagnostics.push(Diagnostic::new(
                    format!("string 来源的起始下标 {start} 不能大于结束下标 {end}"),
                    statement.span,
                ));
            }
        }
        StatementKind::ItemAction {
            method: _,
            target,
            slots,
            slots_span,
            action,
        } => {
            crate::compiler::validate::expressions::validate_item_condition_source(
                target,
                statement.span,
                ctx,
                diagnostics,
            );
            crate::compiler::validate::expressions::validate_slot_source(
                slots,
                *slots_span,
                diagnostics,
            );
            if let ItemConditionSource::Entity(Holder::Query(name, name_span)) = target
                && let Some(query) = ctx.symbols.queries.get(name.as_str())
                && query.limit != Some(1)
            {
                diagnostics.push(Diagnostic::new(
                    format!("item 目标需要 limit(1) 的单个实体查询 `{name}`"),
                    *name_span,
                ));
            }
            match action {
                ItemActionKind::With(item, span) => {
                    match ctx.symbols.item_stacks.get(item.as_str()) {
                        None => diagnostics.push(Diagnostic::new(
                            format!(
                                "找不到物品定义 `{item}`；先声明 `item {item} = item_stack(...);`"
                            ),
                            *span,
                        )),
                        Some(definition) if definition.count > 99 => {
                            diagnostics.push(Diagnostic::new(
                                format!(
                                    "item with 使用的物品 `{item}` 数量不能超过 99，实际为 {}",
                                    definition.count
                                ),
                                *span,
                            ))
                        }
                        Some(_) => {}
                    }
                }
                ItemActionKind::From {
                    source,
                    slots,
                    slots_span,
                    modifier,
                } => {
                    crate::compiler::validate::expressions::validate_item_condition_source(
                        source,
                        statement.span,
                        ctx,
                        diagnostics,
                    );
                    crate::compiler::validate::expressions::validate_slot_source(
                        slots,
                        *slots_span,
                        diagnostics,
                    );
                    if let Some(modifier) = modifier
                        && !crate::compiler::validate::rules::valid_resource_location(modifier)
                    {
                        diagnostics.push(Diagnostic::new(
                            format!("`{modifier}` 不是有效的物品修饰器资源位置"),
                            statement.span,
                        ));
                    }
                    if let ItemConditionSource::Entity(Holder::Query(name, name_span)) = source
                        && let Some(query) = ctx.symbols.queries.get(name.as_str())
                        && query.limit != Some(1)
                    {
                        diagnostics.push(Diagnostic::new(
                            format!("item 来源需要 limit(1) 的单个实体查询 `{name}`"),
                            *name_span,
                        ));
                    }
                }
                ItemActionKind::Modifier(modifier, span) => {
                    if !crate::compiler::validate::rules::valid_resource_location(modifier) {
                        diagnostics.push(Diagnostic::new(
                            format!("`{modifier}` 不是有效的物品修饰器资源位置"),
                            *span,
                        ));
                    }
                }
            }
        }
        StatementKind::NbtMerge { nbt } => {
            validate_nbt_merge(nbt, statement.span, ctx, diagnostics);
        }
        StatementKind::AdvancementAction {
            targets,
            advancement,
            criterion,
            criterion_span,
            ..
        } => {
            validate_advancement_action(
                targets,
                advancement.as_ref(),
                criterion.as_deref(),
                *criterion_span,
                statement.span,
                ctx,
                diagnostics,
            );
        }
        StatementKind::Let { name, value, .. } => {
            validate_let(name, value, locals, ctx, diagnostics);
        }
        StatementKind::If {
            condition,
            then_body,
            else_body,
        } => {
            validate_if(condition, then_body, else_body, locals, ctx, diagnostics);
        }
        StatementKind::While { condition, body } => {
            validate_while(condition, body, locals, ctx, diagnostics);
        }
        StatementKind::For {
            variable,
            start,
            end,
            body,
            ..
        } => {
            validate_for(variable, start, end, body, locals, ctx, diagnostics);
        }
        StatementKind::Break | StatementKind::Continue => {
            if ctx.return_rules.loop_depth == 0 {
                let keyword = if matches!(&statement.kind, StatementKind::Break) {
                    "break"
                } else {
                    "continue"
                };
                diagnostics.push(Diagnostic::new(
                    format!(
                        "`{keyword}` 只能出现在 for/while 循环体内；each/spawn 的每个实体会单独执行，\
                         不能用它跳出"
                    ),
                    statement.span,
                ));
            }
        }
        StatementKind::Execute { clauses, body } => {
            validate_execute(clauses, body, statement.span, locals, ctx, diagnostics);
        }
        StatementKind::SetBlock { .. }
        | StatementKind::Fill { .. }
        | StatementKind::FillBiome { .. }
        | StatementKind::Clone { .. }
        | StatementKind::PlaceFeature { .. }
        | StatementKind::PlaceJigsaw { .. }
        | StatementKind::PlaceStructure { .. }
        | StatementKind::PlaceTemplate { .. }
        | StatementKind::ForceLoad(_)
        | StatementKind::TimeAction { .. }
        | StatementKind::Weather { .. }
        | StatementKind::GameRuleSet { .. }
        | StatementKind::WorldBorder(_)
        | StatementKind::Locate { .. } => {
            crate::compiler::validate::world::validate_world_statement(statement, diagnostics);
        }
    }
}
