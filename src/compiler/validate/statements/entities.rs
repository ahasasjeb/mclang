use super::*;

pub(super) fn validate_give_statement(
    target: &GiveTarget,
    item: &GiveItem,
    count: Option<u32>,
    count_span: Option<Span>,
    span: Span,
    ctx: ValidationContext<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match target {
        GiveTarget::Query(name) => match ctx.symbols.queries.get(name.as_str()) {
            None => diagnostics.push(Diagnostic::new(format!("找不到实体查询 `{name}`"), span)),
            Some(query) if query.entity_type != "minecraft:player" => {
                diagnostics.push(Diagnostic::new(
                    format!("give 目标查询 `{name}` 必须匹配 minecraft:player"),
                    span,
                ));
            }
            Some(_) => {}
        },
        GiveTarget::Origin => {
            if !ctx.context.is_entity() {
                diagnostics.push(Diagnostic::new(
                    "give 的 origin/投掷者目标需要实体执行上下文",
                    span,
                ));
            }
        }
    }
    match item {
        GiveItem::Definition(name) => match ctx.symbols.item_stacks.get(name.as_str()) {
            None => diagnostics.push(Diagnostic::new(format!("找不到物品定义 `{name}`"), span)),
            Some(declaration) => {
                validate_give_count(declaration, count, span, count_span, diagnostics);
            }
        },
        GiveItem::SelfItem => {
            if !ctx.context.is_entity() {
                diagnostics.push(Diagnostic::new(
                    "self.item 需要实体执行上下文；请放入 each/spawn 块，或给函数添加 @entity",
                    span,
                ));
            }
            if count.is_some() {
                diagnostics.push(Diagnostic::new(
                    "原样给予 self.item 时不能指定数量",
                    count_span.unwrap_or(span),
                ));
            }
        }
    }
}

pub(super) fn validate_self_action(
    action: &SelfAction,
    span: Span,
    ctx: ValidationContext<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let method = match action {
        SelfAction::AddTag(_) => "add_tag",
        SelfAction::RemoveTag(_) => "remove_tag",
        SelfAction::SetInvulnerable(_) => "set_invulnerable",
        SelfAction::SetNoGravity(_) => "set_no_gravity",
        SelfAction::SaveItems(_) => "save_items",
        SelfAction::RestoreItems(_) => "restore_items",
        SelfAction::RemovePreservingItems(_) => "remove_preserving_items",
        SelfAction::RemovePreservingSlot { .. } => "remove_preserving_items",
        SelfAction::GiveItem { .. } => "give_item",
        SelfAction::ClearItems => "clear_items",
        SelfAction::DataStore { .. } => "deposit",
        SelfAction::DataLoad { .. } => "withdraw",
        SelfAction::DataClear { .. } => "remove_data",
        SelfAction::Remove => "remove",
    };
    let (required, message) = match action {
        SelfAction::GiveItem { .. } => (
            ExecutionContext::Player,
            format!(
                "self.{method} 需要玩家执行上下文；请放入玩家 query 的 each 块，或给函数添加 @player"
            ),
        ),
        SelfAction::SetInvulnerable(_)
        | SelfAction::SetNoGravity(_)
        | SelfAction::SaveItems(_)
        | SelfAction::RestoreItems(_)
        | SelfAction::RemovePreservingItems(_)
        | SelfAction::RemovePreservingSlot { .. }
        | SelfAction::ClearItems
        | SelfAction::DataStore { .. }
        | SelfAction::DataLoad { .. }
        | SelfAction::DataClear { .. } => (
            ExecutionContext::Mob,
            format!(
                "self.{method} 通过 data 命令修改实体 NBT，Minecraft 不允许修改玩家数据；只能在确定不是玩家的实体上下文中使用（非玩家查询的 each、非玩家 spawn，或 @non_player 函数）"
            ),
        ),
        SelfAction::AddTag(_) | SelfAction::RemoveTag(_) | SelfAction::Remove => (
            ExecutionContext::Entity,
            format!("self.{method} 需要实体执行上下文；请放入 each/spawn 块，或给函数添加 @entity"),
        ),
    };
    if !ctx.context.satisfies(required) {
        diagnostics.push(Diagnostic::new(message, span));
    }
    match action {
        SelfAction::AddTag(tag) | SelfAction::RemoveTag(tag) => {
            if !valid_entity_tag(tag) {
                diagnostics.push(Diagnostic::new(format!("`{tag}` 不是有效的实体标签"), span));
            }
        }
        SelfAction::SaveItems(storage)
        | SelfAction::RestoreItems(storage)
        | SelfAction::RemovePreservingItems(storage) => {
            if !ctx.symbols.storages.contains(storage.as_str()) {
                diagnostics.push(Diagnostic::new(format!("找不到物品存储 `{storage}`"), span));
            }
        }
        SelfAction::DataStore {
            slot,
            slot_span,
            query,
            query_span,
        }
        | SelfAction::DataLoad {
            slot,
            slot_span,
            query,
            query_span,
        }
        | SelfAction::RemovePreservingSlot {
            slot,
            slot_span,
            query,
            query_span,
        } => {
            validate_data_slot(slot, *slot_span, ctx, diagnostics);
            validate_data_slot_query(slot, *slot_span, query, *query_span, ctx, diagnostics);
        }
        SelfAction::DataClear { slot, slot_span } => {
            validate_data_slot(slot, *slot_span, ctx, diagnostics);
        }
        SelfAction::GiveItem {
            item,
            count,
            count_span,
        } => match ctx.symbols.item_stacks.get(item.as_str()) {
            None => diagnostics.push(Diagnostic::new(format!("找不到物品定义 `{item}`"), span)),
            Some(declaration) => {
                validate_give_count(declaration, *count, span, *count_span, diagnostics);
            }
        },
        SelfAction::SetInvulnerable(_)
        | SelfAction::SetNoGravity(_)
        | SelfAction::ClearItems
        | SelfAction::Remove => {}
    }
}

/// 数据槽引用必须已声明。
pub(super) fn validate_data_slot(
    name: &str,
    span: Span,
    ctx: ValidationContext<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !ctx.symbols.data_slots.contains_key(name) {
        diagnostics.push(Diagnostic::new(format!("找不到数据槽 `{name}`"), span));
    }
}

/// `deposit`/`withdraw` 的另一侧查询：必须唯一，且实体类型与数据槽种类相容。
pub(super) fn validate_data_slot_query(
    slot: &str,
    slot_span: Span,
    query: &str,
    query_span: Span,
    ctx: ValidationContext<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(declaration) = ctx.symbols.data_slots.get(slot) else {
        return;
    };
    let Some(target) = ctx.symbols.queries.get(query) else {
        diagnostics.push(Diagnostic::new(
            format!("找不到实体查询 `{query}`"),
            query_span,
        ));
        return;
    };
    if target.limit != Some(1) {
        diagnostics.push(Diagnostic::new(
            format!("数据槽操作需要 limit(1) 的单个实体查询 `{query}`"),
            query_span,
        ));
    }
    match declaration.kind {
        DataSlotKind::ItemData if target.entity_type != "minecraft:item" => {
            diagnostics.push(Diagnostic::new(
                format!(
                    "物品数据槽 `{slot}` 只能配合 minecraft:item 查询使用；`{query}` 匹配 {}",
                    target.entity_type
                ),
                query_span.merge(slot_span),
            ));
        }
        DataSlotKind::EntityData if target.entity_type == "minecraft:player" => {
            diagnostics.push(Diagnostic::new(
                format!("实体数据槽 `{slot}` 不能指向玩家：Minecraft 拒绝修改玩家 NBT"),
                query_span.merge(slot_span),
            ));
        }
        _ => {}
    }
}

/// 计分目标的持有者与目标名都必须已声明，且持有者类型与当前上下文相容。
pub(in crate::compiler::validate) fn validate_score_target(
    target: &ScoreTarget,
    span: Span,
    ctx: ValidationContext<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !ctx.symbols.objectives.contains(target.objective.as_str()) {
        diagnostics.push(Diagnostic::new(
            format!("找不到计分板目标 `{}`", target.objective),
            target.objective_span,
        ));
    }
    validate_holder(&target.holder, span, ctx, diagnostics);
}

/// 持有者引用：`self`/`origin` 需要实体上下文，查询必须已声明。
pub(in crate::compiler::validate) fn validate_holder(
    holder: &Holder,
    span: Span,
    ctx: ValidationContext<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match holder {
        Holder::SelfEntity => {
            if !ctx.context.is_entity() {
                diagnostics.push(Diagnostic::new(
                    "持有者 self/自身 需要实体执行上下文；请放入 each/spawn 块，或给函数添加 @entity",
                    span,
                ));
            }
        }
        Holder::Origin => {
            if !ctx.context.is_entity() {
                diagnostics.push(Diagnostic::new(
                    "持有者 origin/投掷者 需要实体执行上下文",
                    span,
                ));
            }
        }
        Holder::Query(name, query_span) => {
            if !ctx.symbols.queries.contains_key(name.as_str()) {
                diagnostics.push(Diagnostic::new(
                    format!("找不到实体查询 `{name}`"),
                    *query_span,
                ));
            }
        }
    }
}

/// `teleport(持有者, 落点)`：落点是坐标或 `limit(1)` 的单个实体查询。
pub(super) fn validate_teleport(
    targets: &Holder,
    destination: &TeleportDestination,
    rotation: Option<&RotationValue>,
    span: Span,
    ctx: ValidationContext<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    validate_holder(targets, span, ctx, diagnostics);
    if let Some(rotation) = rotation
        && matches!(destination, TeleportDestination::Entity { .. })
    {
        diagnostics.push(Diagnostic::new(
            "teleport 跟随实体时不能同时指定朝向：实体的朝向会一并跟随",
            rotation.span,
        ));
    }
    match destination {
        TeleportDestination::Position(position) => {
            crate::compiler::validate::world::validate_position_value(position, diagnostics);
        }
        TeleportDestination::Entity { query, query_span } => {
            if !ctx.symbols.queries.contains_key(query.as_str()) {
                diagnostics.push(Diagnostic::new(
                    format!("找不到实体查询 `{query}`"),
                    *query_span,
                ));
                return;
            }
            if let Some(declaration) = ctx.symbols.queries.get(query.as_str())
                && declaration.limit != Some(1)
            {
                diagnostics.push(Diagnostic::new(
                    format!("teleport 的落点需要 limit(1) 的单个实体查询 `{query}`"),
                    *query_span,
                ));
            }
        }
    }
}
