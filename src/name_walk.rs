//! 名字位置的统一遍历。
//!
//! 编译器里有两类按名字走遍整棵 AST 的工作：
//!
//! - [`crate::compiler::rename`]：把非 ASCII 名字换成随机 ASCII 别名；
//! - [`crate::modules`]：模块解析时把名字限定到所属模块。
//!
//! 两边必须覆盖完全相同的姓名位置，因此遍历只有这一份实现。回调同时收到
//! 名字的**位置**（声明还是引用）与**类别**（函数、计分变量、查询……），
//! 以及所在函数的参数/局部变量表，调用方据此决定改写方式。字符串字面量、
//! 资源位置与 NBT 键不是标识符，遍历不会触及。

use std::collections::HashSet;

use crate::ast::*;

/// 名字所属的符号类别。
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum NameRole {
    Function,
    /// 函数标签（`#标签`）。
    Tag,
    Score,
    Objective,
    Query,
    Item,
    Storage,
    DataSlot,
    /// JSON 资源（谓词、战利品表、配方、进度资源等）。
    Resource,
    /// 结构化进度声明。
    Advancement,
    /// 进度内的准则名；只在所属进度内可见。
    Criterion,
    Parameter,
    Local,
}

/// 名字出现在声明处还是引用处。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NameSite {
    Declaration,
    Reference,
}

/// 遍历回调的上下文：当前函数的参数与局部变量。
pub struct NameContext<'a> {
    /// 当前函数的参数与局部变量名。
    pub locals: &'a HashSet<String>,
}

impl NameContext<'_> {
    /// 名字是否是当前函数的参数或局部变量（全局计分变量会返回 `false`）。
    pub fn is_local(&self, name: &str) -> bool {
        self.locals.contains(name)
    }
}

impl Program {
    /// 按源码顺序遍历全部用户标识符位置，`visitor` 可以就地读写。
    pub fn for_each_name_mut(
        &mut self,
        visitor: &mut impl FnMut(&NameContext<'_>, NameSite, NameRole, &mut String),
    ) {
        let empty = HashSet::new();
        let top = NameContext { locals: &empty };

        for score in &mut self.scores {
            visitor(
                &top,
                NameSite::Declaration,
                NameRole::Score,
                &mut score.name,
            );
        }
        for objective in &mut self.objectives {
            visitor(
                &top,
                NameSite::Declaration,
                NameRole::Objective,
                &mut objective.name,
            );
            if let Some(display_name) = &mut objective.display_name {
                component_names(display_name, visitor, &top);
            }
            if let Some(NumberFormat::Fixed(component)) = &mut objective.number_format {
                component_names(component, visitor, &top);
            }
        }
        for query in &mut self.queries {
            visitor(
                &top,
                NameSite::Declaration,
                NameRole::Query,
                &mut query.name,
            );
        }
        for item in &mut self.item_stacks {
            visitor(&top, NameSite::Declaration, NameRole::Item, &mut item.name);
        }
        for storage in &mut self.storages {
            visitor(
                &top,
                NameSite::Declaration,
                NameRole::Storage,
                &mut storage.name,
            );
        }
        for slot in &mut self.data_slots {
            visitor(
                &top,
                NameSite::Declaration,
                NameRole::DataSlot,
                &mut slot.name,
            );
        }
        for resource in &mut self.resources {
            visitor(
                &top,
                NameSite::Declaration,
                NameRole::Resource,
                &mut resource.name,
            );
        }
        for advancement in &mut self.advancements {
            visitor(
                &top,
                NameSite::Declaration,
                NameRole::Advancement,
                &mut advancement.name,
            );
            if let Some(parent) = &mut advancement.parent {
                reference_names(parent, NameRole::Advancement, visitor, &top);
            }
            for criterion in &mut advancement.criteria {
                visitor(
                    &top,
                    NameSite::Declaration,
                    NameRole::Criterion,
                    &mut criterion.name,
                );
            }
            if let Some(reward) = &mut advancement.reward {
                if let Some(function) = &mut reward.function {
                    reference_names(function, NameRole::Function, visitor, &top);
                }
                for loot in &mut reward.loot {
                    reference_names(loot, NameRole::Resource, visitor, &top);
                }
                for recipe in &mut reward.recipes {
                    reference_names(recipe, NameRole::Resource, visitor, &top);
                }
            }
            if let Some(display) = &mut advancement.display {
                visitor(&top, NameSite::Reference, NameRole::Item, &mut display.icon);
            }
        }
        for tag in &mut self.function_tags {
            visitor(&top, NameSite::Declaration, NameRole::Tag, &mut tag.name);
            for entry in &mut tag.values {
                match entry {
                    FunctionTagEntry::Function(name, _) => {
                        visitor(&top, NameSite::Reference, NameRole::Function, name);
                    }
                    FunctionTagEntry::Tag(name, _) => {
                        visitor(&top, NameSite::Reference, NameRole::Tag, name);
                    }
                    FunctionTagEntry::External(_, _) => {}
                }
            }
        }
        for function in &mut self.functions {
            let Function {
                name,
                parameters,
                body,
                ..
            } = function;
            visitor(&top, NameSite::Declaration, NameRole::Function, name);
            for parameter in parameters.iter_mut() {
                visitor(
                    &top,
                    NameSite::Declaration,
                    NameRole::Parameter,
                    &mut parameter.name,
                );
            }
            let mut locals = collect_local_names_owned(body);
            // 参数也是函数局部名：模块解析不能把参数引用改写成导入的限定名。
            for parameter in parameters.iter() {
                locals.insert(parameter.name.clone());
            }
            let context = NameContext { locals: &locals };
            statement_names(body, visitor, &context);
        }
    }
}

/// 资源引用：`external` 为真时是完整的外部资源位置，不参与名称解析。
fn reference_names(
    reference: &mut AdvancementReference,
    role: NameRole,
    visitor: &mut impl FnMut(&NameContext<'_>, NameSite, NameRole, &mut String),
    context: &NameContext<'_>,
) {
    if !reference.external {
        visitor(context, NameSite::Reference, role, &mut reference.name);
    }
}

/// 收集函数体内的全部 `let` 局部变量名（含嵌套块）。
pub fn collect_local_names<'a>(statements: &'a [Statement], locals: &mut Vec<&'a str>) {
    for statement in statements {
        match &statement.kind {
            StatementKind::Let { name, .. } => locals.push(name),
            StatementKind::For { variable, body, .. } => {
                locals.push(variable);
                collect_local_names(body, locals);
            }
            StatementKind::If {
                then_body,
                else_body,
                ..
            } => {
                collect_local_names(then_body, locals);
                collect_local_names(else_body, locals);
            }
            StatementKind::Execute { body, .. }
            | StatementKind::Each { body, .. }
            | StatementKind::InDimension { body, .. }
            | StatementKind::Spawn { body, .. }
            | StatementKind::While { body, .. } => collect_local_names(body, locals),
            _ => {}
        }
    }
}

/// [`collect_local_names`] 的独立版本：返回拥有所有权的名字集合。
pub fn collect_local_names_owned(statements: &[Statement]) -> HashSet<String> {
    let mut names = Vec::new();
    collect_local_names(statements, &mut names);
    names.into_iter().map(str::to_owned).collect()
}

fn statement_names(
    statements: &mut [Statement],
    visitor: &mut impl FnMut(&NameContext<'_>, NameSite, NameRole, &mut String),
    context: &NameContext<'_>,
) {
    for statement in statements {
        statement_kind_names(&mut statement.kind, visitor, context);
    }
}

fn statement_kind_names(
    kind: &mut StatementKind,
    visitor: &mut impl FnMut(&NameContext<'_>, NameSite, NameRole, &mut String),
    context: &NameContext<'_>,
) {
    match kind {
        StatementKind::Run(_) => {}
        StatementKind::Each { query, body } => {
            visitor(context, NameSite::Reference, NameRole::Query, query);
            statement_names(body, visitor, context);
        }
        StatementKind::InDimension { body, .. } | StatementKind::Spawn { body, .. } => {
            statement_names(body, visitor, context);
        }
        StatementKind::Give { target, item, .. } => {
            if let GiveTarget::Query(name) = target {
                visitor(context, NameSite::Reference, NameRole::Query, name);
            }
            if let GiveItem::Definition(name) = item {
                visitor(context, NameSite::Reference, NameRole::Item, name);
            }
        }
        StatementKind::EffectGive { target, .. }
        | StatementKind::EffectClear { target, .. }
        | StatementKind::XpChange { target, .. }
        | StatementKind::ClearInventory { target, .. } => {
            visitor(context, NameSite::Reference, NameRole::Query, target);
        }
        StatementKind::StopwatchAction { .. } => {}
        StatementKind::PlaySound { targets, .. } => {
            if let Some(targets) = targets {
                visitor(context, NameSite::Reference, NameRole::Query, targets);
            }
        }
        StatementKind::Call { target, arguments } => {
            call_target_names(target, visitor, context);
            for argument in arguments {
                expression_names(argument, visitor, context);
            }
        }
        StatementKind::Let { name, value, .. } => {
            visitor(context, NameSite::Declaration, NameRole::Local, name);
            expression_names(value, visitor, context);
        }
        StatementKind::Schedule { target, .. } => call_target_names(target, visitor, context),
        StatementKind::ScheduleClear { function } => {
            visitor(context, NameSite::Reference, NameRole::Function, function);
        }
        StatementKind::Assign { target, value, .. } => {
            visitor(context, NameSite::Reference, NameRole::Score, target);
            expression_names(value, visitor, context);
        }
        StatementKind::ScoreSet { target, value } => {
            score_target_names(target, visitor, context);
            expression_names(value, visitor, context);
        }
        StatementKind::ScoreReset { target } | StatementKind::ScoreboardEnable { target } => {
            score_target_names(target, visitor, context);
        }
        StatementKind::ScoreboardOperation { result, source, .. } => {
            score_target_names(result, visitor, context);
            score_target_names(source, visitor, context);
        }
        StatementKind::ScoreboardDisplay { objective, .. } => {
            if let Some((name, _)) = objective {
                visitor(context, NameSite::Reference, NameRole::Objective, name);
            }
        }
        StatementKind::Teleport {
            targets,
            destination,
            ..
        } => {
            holder_names(targets, visitor, context);
            if let TeleportDestination::Entity { query, .. } = destination {
                visitor(context, NameSite::Reference, NameRole::Query, query);
            }
        }
        StatementKind::NbtMerge { .. } => {}
        StatementKind::DataMerge { target, .. }
        | StatementKind::DataRemove { target, .. }
        | StatementKind::DataModify { target, .. } => nbt_source_names(target, visitor, context),
        StatementKind::ItemAction { target, action, .. } => {
            item_source_names(target, visitor, context);
            match action {
                ItemActionKind::With(item, _) => {
                    visitor(context, NameSite::Reference, NameRole::Item, item);
                }
                ItemActionKind::From { source, .. } => {
                    item_source_names(source, visitor, context);
                }
                ItemActionKind::Modifier(_, _) => {}
            }
        }
        StatementKind::SelfAction(action) => self_action_names(action, visitor, context),
        StatementKind::Message { target, component } => {
            if let MessageTarget::Query { name, .. } = target {
                visitor(context, NameSite::Reference, NameRole::Query, name);
            }
            component_names(component, visitor, context);
        }
        StatementKind::AdvancementAction {
            targets,
            advancement,
            criterion,
            ..
        } => {
            holder_names(targets, visitor, context);
            if let Some(advancement) = advancement {
                reference_names(advancement, NameRole::Advancement, visitor, context);
            }
            if let Some(criterion) = criterion {
                visitor(context, NameSite::Reference, NameRole::Criterion, criterion);
            }
        }
        StatementKind::If {
            condition,
            then_body,
            else_body,
        } => {
            condition_names(condition, visitor, context);
            statement_names(then_body, visitor, context);
            statement_names(else_body, visitor, context);
        }
        StatementKind::While { condition, body } => {
            condition_names(condition, visitor, context);
            statement_names(body, visitor, context);
        }
        StatementKind::For {
            variable,
            start,
            end,
            body,
            ..
        } => {
            visitor(context, NameSite::Declaration, NameRole::Local, variable);
            expression_names(start, visitor, context);
            expression_names(end, visitor, context);
            statement_names(body, visitor, context);
        }
        StatementKind::Break | StatementKind::Continue => {}
        StatementKind::Execute { clauses, body } => {
            if let ExecuteClauses::Structured(clauses) = clauses {
                for clause in clauses {
                    clause_names(&mut clause.kind, visitor, context);
                }
            }
            statement_names(body, visitor, context);
        }
        StatementKind::Return(ReturnKind::Value(value)) => {
            expression_names(value, visitor, context)
        }
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

fn self_action_names(
    action: &mut SelfAction,
    visitor: &mut impl FnMut(&NameContext<'_>, NameSite, NameRole, &mut String),
    context: &NameContext<'_>,
) {
    match action {
        SelfAction::AddTag(_)
        | SelfAction::RemoveTag(_)
        | SelfAction::SetInvulnerable(_)
        | SelfAction::SetNoGravity(_)
        | SelfAction::ClearItems
        | SelfAction::Remove => {}
        SelfAction::SaveItems(storage)
        | SelfAction::RestoreItems(storage)
        | SelfAction::RemovePreservingItems(storage) => {
            visitor(context, NameSite::Reference, NameRole::Storage, storage);
        }
        SelfAction::RemovePreservingSlot { slot, query, .. }
        | SelfAction::DataStore { slot, query, .. }
        | SelfAction::DataLoad { slot, query, .. } => {
            visitor(context, NameSite::Reference, NameRole::DataSlot, slot);
            visitor(context, NameSite::Reference, NameRole::Query, query);
        }
        SelfAction::GiveItem { item, .. } => {
            visitor(context, NameSite::Reference, NameRole::Item, item);
        }
        SelfAction::DataClear { slot, .. } => {
            visitor(context, NameSite::Reference, NameRole::DataSlot, slot);
        }
    }
}

fn clause_names(
    kind: &mut ExecuteClauseKind,
    visitor: &mut impl FnMut(&NameContext<'_>, NameSite, NameRole, &mut String),
    context: &NameContext<'_>,
) {
    match kind {
        ExecuteClauseKind::As { query, .. } | ExecuteClauseKind::At { query, .. } => {
            visitor(context, NameSite::Reference, NameRole::Query, query);
        }
        ExecuteClauseKind::FacingEntity { query, .. } => {
            visitor(context, NameSite::Reference, NameRole::Query, query);
        }
        ExecuteClauseKind::If(condition) | ExecuteClauseKind::Unless(condition) => {
            condition_names(condition, visitor, context);
        }
        ExecuteClauseKind::StoreResult(target) | ExecuteClauseKind::StoreSuccess(target) => {
            if let ExecuteStoreTarget::Score(target) = target {
                score_target_names(target, visitor, context);
            }
        }
        ExecuteClauseKind::StoreData(data) => nbt_source_names(&mut data.source, visitor, context),
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

fn condition_names(
    condition: &mut Condition,
    visitor: &mut impl FnMut(&NameContext<'_>, NameSite, NameRole, &mut String),
    context: &NameContext<'_>,
) {
    match condition {
        Condition::Predicate { name, .. } => {
            visitor(context, NameSite::Reference, NameRole::Resource, name);
        }
        Condition::Compare { left, right, .. } => {
            expression_names(left, visitor, context);
            expression_names(right, visitor, context);
        }
        Condition::Entity { query, .. } => {
            visitor(context, NameSite::Reference, NameRole::Query, query);
        }
        Condition::Data { source, .. } => nbt_source_names(source, visitor, context),
        Condition::Items { source, .. } | Condition::Slots { source, .. } => {
            item_source_names(source, visitor, context);
        }
        Condition::Function { target, .. } => call_target_names(target, visitor, context),
        Condition::Not(inner) => condition_names(inner, visitor, context),
        Condition::And(left, right) | Condition::Or(left, right) => {
            condition_names(left, visitor, context);
            condition_names(right, visitor, context);
        }
        Condition::Block { .. }
        | Condition::Blocks { .. }
        | Condition::Biome { .. }
        | Condition::Loaded { .. }
        | Condition::Dimension { .. }
        | Condition::Stopwatch { .. } => {}
    }
}

fn expression_names(
    expression: &mut Expr,
    visitor: &mut impl FnMut(&NameContext<'_>, NameSite, NameRole, &mut String),
    context: &NameContext<'_>,
) {
    match &mut expression.kind {
        ExprKind::Integer(_) => {}
        ExprKind::Score(name) => visitor(context, NameSite::Reference, NameRole::Score, name),
        ExprKind::Call {
            function,
            arguments,
        } => {
            visitor(context, NameSite::Reference, NameRole::Function, function);
            for argument in arguments {
                expression_names(argument, visitor, context);
            }
        }
        ExprKind::XpQuery { target, .. } => {
            visitor(context, NameSite::Reference, NameRole::Query, target);
        }
        ExprKind::ScoreQuery { target } => score_target_names(target, visitor, context),
        ExprKind::Count { query, .. } => {
            visitor(context, NameSite::Reference, NameRole::Query, query);
        }
        ExprKind::DataGet { source, .. } => nbt_source_names(source, visitor, context),
        ExprKind::Compute { source, .. } => {
            if let ComputeSource::Entity(holder) = source {
                holder_names(holder, visitor, context);
            }
        }
        ExprKind::Negate(value) => expression_names(value, visitor, context),
        ExprKind::Binary { left, right, .. } => {
            expression_names(left, visitor, context);
            expression_names(right, visitor, context);
        }
        ExprKind::StopwatchQuery { .. }
        | ExprKind::TimeQuery { .. }
        | ExprKind::GameTimeQuery
        | ExprKind::GameRuleQuery { .. }
        | ExprKind::WorldBorderSize
        | ExprKind::Random { .. } => {}
    }
}

fn component_names(
    component: &mut TextComponent,
    visitor: &mut impl FnMut(&NameContext<'_>, NameSite, NameRole, &mut String),
    context: &NameContext<'_>,
) {
    match &mut component.kind {
        TextComponentKind::Text(_) | TextComponentKind::Keybind(_) => {}
        TextComponentKind::Translate { args, .. } => {
            for arg in args {
                component_names(arg, visitor, context);
            }
        }
        TextComponentKind::Score {
            holder, objective, ..
        } => {
            holder_names(holder, visitor, context);
            if let ObjectiveRef::Declared(name) = objective {
                visitor(context, NameSite::Reference, NameRole::Objective, name);
            }
        }
        TextComponentKind::Selector(SelectorValue::Query(name, _)) => {
            visitor(context, NameSite::Reference, NameRole::Query, name);
        }
        TextComponentKind::Selector(SelectorValue::Raw(_, _)) => {}
        TextComponentKind::Nbt {
            source, separator, ..
        } => {
            nbt_source_names(source, visitor, context);
            if let Some(separator) = separator {
                component_names(separator, visitor, context);
            }
        }
    }
    if let Some(hover) = &mut component.style.hover {
        component_names(hover, visitor, context);
    }
}

fn holder_names(
    holder: &mut Holder,
    visitor: &mut impl FnMut(&NameContext<'_>, NameSite, NameRole, &mut String),
    context: &NameContext<'_>,
) {
    if let Holder::Query(name, _) = holder {
        visitor(context, NameSite::Reference, NameRole::Query, name);
    }
}

fn call_target_names(
    target: &mut CallTarget,
    visitor: &mut impl FnMut(&NameContext<'_>, NameSite, NameRole, &mut String),
    context: &NameContext<'_>,
) {
    match target {
        CallTarget::Function(name) => {
            visitor(context, NameSite::Reference, NameRole::Function, name);
        }
        CallTarget::Tag(name) => {
            visitor(context, NameSite::Reference, NameRole::Tag, name);
        }
    }
}

fn score_target_names(
    target: &mut ScoreTarget,
    visitor: &mut impl FnMut(&NameContext<'_>, NameSite, NameRole, &mut String),
    context: &NameContext<'_>,
) {
    holder_names(&mut target.holder, visitor, context);
    visitor(
        context,
        NameSite::Reference,
        NameRole::Objective,
        &mut target.objective,
    );
}

fn nbt_source_names(
    source: &mut NbtComponentSource,
    visitor: &mut impl FnMut(&NameContext<'_>, NameSite, NameRole, &mut String),
    context: &NameContext<'_>,
) {
    if let NbtComponentSource::Entity(holder) = source {
        holder_names(holder, visitor, context);
    }
}

fn item_source_names(
    source: &mut ItemConditionSource,
    visitor: &mut impl FnMut(&NameContext<'_>, NameSite, NameRole, &mut String),
    context: &NameContext<'_>,
) {
    if let ItemConditionSource::Entity(holder) = source {
        holder_names(holder, visitor, context);
    }
}
