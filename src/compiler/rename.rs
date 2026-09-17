//! 用户标识符的内部化：把非 ASCII 名字换成确定性的随机 ASCII 别名。
//!
//! 语义检查通过后、代码生成之前运行。用户写中文（或其他 Unicode）标识符，
//! 数据包里只出现 ASCII：
//!
//! - 别名由名字的稳定哈希驱动，同一份源码总是得到同一份产物；
//! - 每个别名在整程序内唯一，并避开所有保持原样的 ASCII 标识符；
//! - 名字的引用与声明走同一张映射表，改名后符号关系不变。
//!
//! 诊断仍在 `validate` 阶段用用户写的名字产生，因此错误信息不受影响。

use std::collections::{HashMap, HashSet};

use crate::ast::*;

use super::stable_hash;

/// 别名的字符数：8 个随机小写字母。
const ALIAS_LENGTH: usize = 8;

/// 把整程序中的非 ASCII 标识符替换为随机 ASCII 别名。
pub(super) fn rename_program(program: &mut Program) {
    let aliases = build_aliases(program);
    if aliases.is_empty() {
        return;
    }
    for_each_name_mut(program, &mut |name| {
        if let Some(alias) = aliases.get(name) {
            *name = alias.clone();
        }
    });
}

/// 为需要内部化的名字分配别名，返回 `原名 -> 别名`。
fn build_aliases(program: &mut Program) -> HashMap<String, String> {
    // 保持原样的 ASCII 标识符与内部固定名不允许被别名占用。
    let mut occupied = HashSet::new();
    for_each_name_mut(program, &mut |name| {
        if name.is_ascii() {
            occupied.insert(name.clone());
        }
    });
    occupied.insert("load".to_owned());
    occupied.insert("tick".to_owned());
    occupied.insert("__mcl".to_owned());

    let mut aliases = HashMap::new();
    for_each_name_mut(program, &mut |name| {
        if name.is_ascii() || aliases.contains_key(name.as_str()) {
            return;
        }
        let alias = random_alias(name, &mut occupied);
        aliases.insert(name.clone(), alias);
    });
    aliases
}

/// 从名字的稳定哈希生成随机小写字母别名；撞名时继续取下一批。
fn random_alias(name: &str, occupied: &mut HashSet<String>) -> String {
    let mut state = stable_hash(name);
    loop {
        let mut alias = String::with_capacity(ALIAS_LENGTH);
        for _ in 0..ALIAS_LENGTH {
            state = splitmix64(state);
            alias.push((b'a' + (state % 26) as u8) as char);
        }
        if occupied.insert(alias.clone()) {
            return alias;
        }
    }
}

/// splitmix64：从 64 位种子继续产生高质量的伪随机序列。
fn splitmix64(seed: u64) -> u64 {
    let seed = seed.wrapping_add(0x9e3779b97f4a7c15);
    let mut value = seed;
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d049bb133111eb);
    value ^ (value >> 31)
}

/// 遍历全部用户标识符位置，`visitor` 可以就地读写。
///
/// 只覆盖参与名称解析的字段；字符串字面量、资源位置与 NBT 键不是标识符。
/// 收集与改写共用这一份清单，避免两边遗漏不同步。
fn for_each_name_mut(program: &mut Program, visitor: &mut impl FnMut(&mut String)) {
    for score in &mut program.scores {
        visitor(&mut score.name);
    }
    for objective in &mut program.objectives {
        visitor(&mut objective.name);
        if let Some(display_name) = &mut objective.display_name {
            component_names(display_name, visitor);
        }
        if let Some(NumberFormat::Fixed(component)) = &mut objective.number_format {
            component_names(component, visitor);
        }
    }
    for query in &mut program.queries {
        visitor(&mut query.name);
    }
    for item in &mut program.item_stacks {
        visitor(&mut item.name);
    }
    for storage in &mut program.storages {
        visitor(&mut storage.name);
    }
    for slot in &mut program.data_slots {
        visitor(&mut slot.name);
    }
    for resource in &mut program.resources {
        visitor(&mut resource.name);
    }
    for advancement in &mut program.advancements {
        visitor(&mut advancement.name);
        if let Some(parent) = &mut advancement.parent {
            visitor(&mut parent.name);
        }
        for criterion in &mut advancement.criteria {
            visitor(&mut criterion.name);
        }
        if let Some(reward) = &mut advancement.reward {
            if let Some(function) = &mut reward.function {
                visitor(&mut function.name);
            }
            for loot in &mut reward.loot {
                visitor(&mut loot.name);
            }
            for recipe in &mut reward.recipes {
                visitor(&mut recipe.name);
            }
        }
        if let Some(display) = &mut advancement.display {
            visitor(&mut display.icon);
        }
    }
    for tag in &mut program.function_tags {
        visitor(&mut tag.name);
        for entry in &mut tag.values {
            match entry {
                FunctionTagEntry::Function(name, _) | FunctionTagEntry::Tag(name, _) => {
                    visitor(name);
                }
                FunctionTagEntry::External(_, _) => {}
            }
        }
    }
    for function in &mut program.functions {
        visitor(&mut function.name);
        for parameter in &mut function.parameters {
            visitor(&mut parameter.name);
        }
        statements_names(&mut function.body, visitor);
    }
}

fn statements_names(statements: &mut [Statement], visitor: &mut impl FnMut(&mut String)) {
    for statement in statements {
        statement_names(&mut statement.kind, visitor);
    }
}

fn statement_names(kind: &mut StatementKind, visitor: &mut impl FnMut(&mut String)) {
    match kind {
        StatementKind::Run(_) => {}
        StatementKind::Each { query, body } => {
            visitor(query);
            statements_names(body, visitor);
        }
        StatementKind::InDimension { body, .. } | StatementKind::Spawn { body, .. } => {
            statements_names(body, visitor);
        }
        StatementKind::Give { target, item, .. } => {
            if let GiveTarget::Query(name) = target {
                visitor(name);
            }
            if let GiveItem::Definition(name) = item {
                visitor(name);
            }
        }
        StatementKind::EffectGive { target, .. }
        | StatementKind::EffectClear { target, .. }
        | StatementKind::XpChange { target, .. }
        | StatementKind::ClearInventory { target, .. } => visitor(target),
        StatementKind::StopwatchAction { .. } => {}
        StatementKind::PlaySound { targets, .. } => {
            if let Some(targets) = targets {
                visitor(targets);
            }
        }
        StatementKind::Call { target, arguments } => {
            call_target_names(target, visitor);
            for argument in arguments {
                expression_names(argument, visitor);
            }
        }
        StatementKind::Let { name, value, .. } => {
            visitor(name);
            expression_names(value, visitor);
        }
        StatementKind::Schedule { target, .. } => call_target_names(target, visitor),
        StatementKind::ScheduleClear { function } => visitor(function),
        StatementKind::Assign { target, value, .. } => {
            visitor(target);
            expression_names(value, visitor);
        }
        StatementKind::ScoreSet { target, value } => {
            score_target_names(target, visitor);
            expression_names(value, visitor);
        }
        StatementKind::ScoreReset { target } | StatementKind::ScoreboardEnable { target } => {
            score_target_names(target, visitor);
        }
        StatementKind::ScoreboardOperation { result, source, .. } => {
            score_target_names(result, visitor);
            score_target_names(source, visitor);
        }
        StatementKind::ScoreboardDisplay { objective, .. } => {
            if let Some((name, _)) = objective {
                visitor(name);
            }
        }
        StatementKind::Teleport {
            targets,
            destination,
            ..
        } => {
            holder_names(targets, visitor);
            if let TeleportDestination::Entity { query, .. } = destination {
                visitor(query);
            }
        }
        StatementKind::NbtMerge { .. } => {}
        StatementKind::DataMerge { target, .. }
        | StatementKind::DataRemove { target, .. }
        | StatementKind::DataModify { target, .. } => nbt_source_names(target, visitor),
        StatementKind::ItemAction { target, action, .. } => {
            item_source_names(target, visitor);
            match action {
                ItemActionKind::With(item, _) => visitor(item),
                ItemActionKind::From { source, .. } => item_source_names(source, visitor),
                ItemActionKind::Modifier(_, _) => {}
            }
        }
        StatementKind::SelfAction(action) => self_action_names(action, visitor),
        StatementKind::Message { target, component } => {
            if let MessageTarget::Query { name, .. } = target {
                visitor(name);
            }
            component_names(component, visitor);
        }
        StatementKind::AdvancementAction {
            targets,
            advancement,
            criterion,
            ..
        } => {
            holder_names(targets, visitor);
            if let Some(advancement) = advancement {
                visitor(&mut advancement.name);
            }
            if let Some(criterion) = criterion {
                visitor(criterion);
            }
        }
        StatementKind::If {
            condition,
            then_body,
            else_body,
        } => {
            condition_names(condition, visitor);
            statements_names(then_body, visitor);
            statements_names(else_body, visitor);
        }
        StatementKind::While { condition, body } => {
            condition_names(condition, visitor);
            statements_names(body, visitor);
        }
        StatementKind::Execute { clauses, body } => {
            if let ExecuteClauses::Structured(clauses) = clauses {
                for clause in clauses {
                    clause_names(&mut clause.kind, visitor);
                }
            }
            statements_names(body, visitor);
        }
        StatementKind::Return(ReturnKind::Value(value)) => expression_names(value, visitor),
        StatementKind::Return(_) => {}
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
        | StatementKind::Locate { .. } => {}
    }
}

fn self_action_names(action: &mut SelfAction, visitor: &mut impl FnMut(&mut String)) {
    match action {
        SelfAction::AddTag(_)
        | SelfAction::RemoveTag(_)
        | SelfAction::SetInvulnerable(_)
        | SelfAction::SetNoGravity(_)
        | SelfAction::ClearItems
        | SelfAction::Remove => {}
        SelfAction::SaveItems(storage)
        | SelfAction::RestoreItems(storage)
        | SelfAction::RemovePreservingItems(storage) => visitor(storage),
        SelfAction::RemovePreservingSlot { slot, query, .. }
        | SelfAction::DataStore { slot, query, .. }
        | SelfAction::DataLoad { slot, query, .. } => {
            visitor(slot);
            visitor(query);
        }
        SelfAction::GiveItem { item, .. } => visitor(item),
        SelfAction::DataClear { slot, .. } => visitor(slot),
    }
}

fn clause_names(kind: &mut ExecuteClauseKind, visitor: &mut impl FnMut(&mut String)) {
    match kind {
        ExecuteClauseKind::As { query, .. } | ExecuteClauseKind::At { query, .. } => visitor(query),
        ExecuteClauseKind::FacingEntity { query, .. } => visitor(query),
        ExecuteClauseKind::If(condition) | ExecuteClauseKind::Unless(condition) => {
            condition_names(condition, visitor);
        }
        ExecuteClauseKind::StoreResult(target) | ExecuteClauseKind::StoreSuccess(target) => {
            if let ExecuteStoreTarget::Score(target) = target {
                score_target_names(target, visitor);
            }
        }
        ExecuteClauseKind::StoreData(data) => nbt_source_names(&mut data.source, visitor),
        ExecuteClauseKind::Positioned(_)
        | ExecuteClauseKind::Rotated(_)
        | ExecuteClauseKind::FacingPosition(_)
        | ExecuteClauseKind::Align { .. }
        | ExecuteClauseKind::Anchored(_)
        | ExecuteClauseKind::In { .. }
        | ExecuteClauseKind::On(_)
        | ExecuteClauseKind::Summon { .. } => {}
    }
}

fn condition_names(condition: &mut Condition, visitor: &mut impl FnMut(&mut String)) {
    match condition {
        Condition::Predicate { name, .. } => visitor(name),
        Condition::Compare { left, right, .. } => {
            expression_names(left, visitor);
            expression_names(right, visitor);
        }
        Condition::Entity { query, .. } => visitor(query),
        Condition::Data { source, .. } => nbt_source_names(source, visitor),
        Condition::Items { source, .. } | Condition::Slots { source, .. } => {
            item_source_names(source, visitor);
        }
        Condition::Function { target, .. } => call_target_names(target, visitor),
        Condition::Not(inner) => condition_names(inner, visitor),
        Condition::And(left, right) | Condition::Or(left, right) => {
            condition_names(left, visitor);
            condition_names(right, visitor);
        }
        Condition::Block { .. }
        | Condition::Blocks { .. }
        | Condition::Biome { .. }
        | Condition::Loaded { .. }
        | Condition::Dimension { .. }
        | Condition::Stopwatch { .. } => {}
    }
}

fn expression_names(expression: &mut Expr, visitor: &mut impl FnMut(&mut String)) {
    match &mut expression.kind {
        ExprKind::Integer(_) => {}
        ExprKind::Score(name) => visitor(name),
        ExprKind::Call {
            function,
            arguments,
        } => {
            visitor(function);
            for argument in arguments {
                expression_names(argument, visitor);
            }
        }
        ExprKind::XpQuery { target, .. } => visitor(target),
        ExprKind::ScoreQuery { target } => score_target_names(target, visitor),
        ExprKind::Count { query, .. } => visitor(query),
        ExprKind::DataGet { source, .. } => nbt_source_names(source, visitor),
        ExprKind::Compute { source, .. } => {
            if let ComputeSource::Entity(holder) = source {
                holder_names(holder, visitor);
            }
        }
        ExprKind::Negate(value) => expression_names(value, visitor),
        ExprKind::Binary { left, right, .. } => {
            expression_names(left, visitor);
            expression_names(right, visitor);
        }
        ExprKind::StopwatchQuery { .. }
        | ExprKind::TimeQuery { .. }
        | ExprKind::GameTimeQuery
        | ExprKind::GameRuleQuery { .. }
        | ExprKind::WorldBorderSize
        | ExprKind::Random { .. } => {}
    }
}

fn component_names(component: &mut TextComponent, visitor: &mut impl FnMut(&mut String)) {
    match &mut component.kind {
        TextComponentKind::Text(_) | TextComponentKind::Keybind(_) => {}
        TextComponentKind::Translate { args, .. } => {
            for arg in args {
                component_names(arg, visitor);
            }
        }
        TextComponentKind::Score {
            holder, objective, ..
        } => {
            holder_names(holder, visitor);
            if let ObjectiveRef::Declared(name) = objective {
                visitor(name);
            }
        }
        TextComponentKind::Selector(SelectorValue::Query(name, _)) => visitor(name),
        TextComponentKind::Selector(SelectorValue::Raw(_, _)) => {}
        TextComponentKind::Nbt {
            source, separator, ..
        } => {
            nbt_source_names(source, visitor);
            if let Some(separator) = separator {
                component_names(separator, visitor);
            }
        }
    }
    if let Some(hover) = &mut component.style.hover {
        component_names(hover, visitor);
    }
}

fn holder_names(holder: &mut Holder, visitor: &mut impl FnMut(&mut String)) {
    if let Holder::Query(name, _) = holder {
        visitor(name);
    }
}

fn call_target_names(target: &mut CallTarget, visitor: &mut impl FnMut(&mut String)) {
    match target {
        CallTarget::Function(name) | CallTarget::Tag(name) => visitor(name),
    }
}

fn score_target_names(target: &mut ScoreTarget, visitor: &mut impl FnMut(&mut String)) {
    holder_names(&mut target.holder, visitor);
    visitor(&mut target.objective);
}

fn nbt_source_names(source: &mut NbtComponentSource, visitor: &mut impl FnMut(&mut String)) {
    if let NbtComponentSource::Entity(holder) = source {
        holder_names(holder, visitor);
    }
}

fn item_source_names(source: &mut ItemConditionSource, visitor: &mut impl FnMut(&mut String)) {
    if let ItemConditionSource::Entity(holder) = source {
        holder_names(holder, visitor);
    }
}
