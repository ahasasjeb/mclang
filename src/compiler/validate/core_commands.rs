use super::{
    entity_commands::{entity_target, resource_id},
    registry::validate_id,
    statements::ValidationContext,
    world::{validate_block_position, validate_position_value},
};
use crate::{ast::*, diagnostic::Diagnostic};

pub(super) fn validate_core_command(
    command: &CoreCommand,
    span: Span,
    ctx: ValidationContext<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match command {
        CoreCommand::Reload => {}
        CoreCommand::Recipe { target, recipe, .. } => {
            entity_target(target, false, true, span, ctx, diagnostics);
            if let Some(recipe) = recipe {
                validate_resource_ref(recipe, "recipe", ctx.symbols.recipes, diagnostics);
            }
        }
        CoreCommand::Random {
            min, max, sequence, ..
        } => {
            let width = i64::from(*max) - i64::from(*min);
            if !(1..i64::from(i32::MAX)).contains(&width) {
                diagnostics.push(Diagnostic::new(
                    "random 要求下界小于上界，且上下界之差小于 2147483647",
                    span,
                ));
            }
            if let Some(sequence) = sequence {
                resource_id(sequence, span, diagnostics);
            }
        }
        CoreCommand::RandomReset { sequence, .. } => {
            if sequence != "*" {
                resource_id(sequence, span, diagnostics);
            }
        }
        CoreCommand::Datapack(operation) => {
            let mut names = Vec::new();
            match operation {
                DatapackOperation::Enable { name, order } => {
                    names.push(name);
                    if let Some(PackOrder::Before(other) | PackOrder::After(other)) = order {
                        names.push(other);
                    }
                }
                DatapackOperation::Disable(name) => names.push(name),
                DatapackOperation::List(_) => {}
            }
            for name in names {
                if name.is_empty() || name.contains(['\n', '\r', '\0']) {
                    diagnostics.push(Diagnostic::new("数据包名称不能为空或包含换行/NUL", span));
                }
            }
        }
        CoreCommand::Loot { target, source } => {
            match target {
                LootTarget::Give(target) => {
                    entity_target(target, false, true, span, ctx, diagnostics)
                }
                LootTarget::Insert(pos) => validate_block_position(pos, diagnostics),
                LootTarget::Spawn(pos) => validate_position_value(pos, diagnostics),
                LootTarget::Replace {
                    target,
                    slot,
                    count,
                } => {
                    match target {
                        ItemConditionSource::Entity(holder) => {
                            entity_target(holder, false, false, span, ctx, diagnostics)
                        }
                        ItemConditionSource::Block(pos) => {
                            validate_block_position(pos, diagnostics)
                        }
                    }
                    if !crate::version::snapshot::snapshot()
                        .slots()
                        .accepts_single(slot)
                    {
                        diagnostics.push(Diagnostic::new(format!("loot.replace 起始槽位 `{slot}` 必须是单槽位，不能使用范围或 slot_source"), span));
                    }
                    if count.is_some_and(|v| v > i32::MAX as u32) {
                        diagnostics
                            .push(Diagnostic::new("战利品槽位数量不能超过 2147483647", span));
                    }
                }
            }
            match source.as_ref() {
                LootSource::Table(table) => {
                    validate_resource_ref(table, "loot_table", ctx.symbols.loot_tables, diagnostics)
                }
                LootSource::Kill(target) => {
                    entity_target(target, true, false, span, ctx, diagnostics)
                }
                LootSource::Fish {
                    table,
                    position,
                    tool,
                } => {
                    validate_resource_ref(
                        table,
                        "loot_table",
                        ctx.symbols.loot_tables,
                        diagnostics,
                    );
                    validate_block_position(position, diagnostics);
                    validate_tool(tool, span, ctx, diagnostics);
                }
                LootSource::Mine { position, tool } => {
                    validate_block_position(position, diagnostics);
                    validate_tool(tool, span, ctx, diagnostics);
                }
            }
        }
    }
}

fn validate_tool(
    tool: &Option<LootTool>,
    span: Span,
    ctx: ValidationContext<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match tool {
        Some(LootTool::Item(name)) if !ctx.symbols.item_stacks.contains_key(name.as_str()) => {
            diagnostics.push(Diagnostic::new(format!("找不到物品定义 `{name}`"), span))
        }
        Some(LootTool::Hand(_)) if !ctx.context.is_entity() => diagnostics.push(Diagnostic::new(
            "从 mainhand/offhand 获取工具需要实体执行上下文",
            span,
        )),
        _ => {}
    }
}

fn validate_resource_ref(
    reference: &AdvancementReference,
    kind: &str,
    declarations: &std::collections::HashSet<&str>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if reference.external {
        validate_id(kind, kind, &reference.name, reference.span, diagnostics);
    } else if !declarations.contains(reference.name.as_str()) {
        diagnostics.push(Diagnostic::new(
            format!("找不到 {kind} 资源 `{}`", reference.name),
            reference.span,
        ));
    }
}
