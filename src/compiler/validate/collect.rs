use std::collections::{HashMap, HashSet};

use crate::ast::*;
use crate::diagnostic::Diagnostic;

use super::ResourceSymbols;
use super::components;
use super::functions::validate_function_declaration;
use super::items::{validate_entity_query, validate_item_stack};
use super::rules::{
    function_context, valid_name, valid_nbt_path, valid_resource_location, valid_resource_path,
    valid_user_name, validate_identifier, windows_reserved_name,
};
use crate::compiler::types::Signature;

pub(super) fn validate_namespace(program: &Program, diagnostics: &mut Vec<Diagnostic>) {
    if !valid_name(&program.namespace) || windows_reserved_name(&program.namespace) {
        diagnostics.push(Diagnostic::new(
            "命名空间只能包含小写 ASCII 字母、数字和下划线，不能以数字开头或使用 Windows 保留设备名",
            program.namespace_span.unwrap_or_default(),
        ));
    }
}

pub(super) fn collect_scores<'a>(
    program: &'a Program,
    diagnostics: &mut Vec<Diagnostic>,
) -> HashSet<&'a str> {
    let mut scores = HashSet::new();
    for score in &program.scores {
        validate_identifier("计分变量", &score.name, score.span, diagnostics);
        if !scores.insert(score.name.as_str()) {
            diagnostics.push(Diagnostic::new(
                format!("重复声明计分变量 `{}`", score.name),
                score.span,
            ));
        }
    }
    scores
}

/// 收集用户计分板目标，并检查名称与内部目标不冲突。
pub(super) fn collect_objectives<'a>(
    program: &'a Program,
    diagnostics: &mut Vec<Diagnostic>,
) -> HashSet<&'a str> {
    let mut objectives = HashSet::new();
    for objective in &program.objectives {
        validate_identifier(
            "计分板目标",
            &objective.name,
            objective.name_span,
            diagnostics,
        );
        if !objectives.insert(objective.name.as_str()) {
            diagnostics.push(Diagnostic::new(
                format!("重复声明计分板目标 `{}`", objective.name),
                objective.span,
            ));
        }
        if let Some(criteria) = &objective.criteria
            && !valid_score_criteria(criteria)
        {
            diagnostics.push(Diagnostic::new(
                format!("`{criteria}` 不是有效的计分板准则（dummy、trigger 或 `命名空间:统计`）"),
                objective.span,
            ));
        }
        if let Some(render_type) = &objective.render_type
            && !matches!(render_type.as_str(), "integer" | "hearts")
        {
            diagnostics.push(Diagnostic::new(
                format!("渲染类型只能是 integer 或 hearts，实际为 `{render_type}`"),
                objective.span,
            ));
        }
        if let Some(slot) = &objective.display_slot {
            crate::compiler::validate::registry::validate_enum(
                "display_slot",
                "显示槽",
                slot,
                objective.display_slot_span.unwrap_or(objective.span),
                diagnostics,
            );
        }
        if let Some(display_name) = &objective.display_name {
            validate_objective_component(display_name, diagnostics);
        }
        if let Some(NumberFormat::Fixed(component)) = &objective.number_format {
            validate_objective_component(component, diagnostics);
        }
        if let Some(NumberFormat::Styled(style)) = &objective.number_format
            && !serde_json::from_str::<serde_json::Value>(style).is_ok_and(|v| v.is_object())
        {
            diagnostics.push(Diagnostic::new("styled 需要 JSON 样式对象", objective.span));
        }
    }
    objectives
}

/// ObjectiveCriteria.byName accepts registered custom names or a registered
/// stat type and value pair encoded with dots instead of resource colons.
fn valid_score_criteria(criteria: &str) -> bool {
    if let Some((stat_type, value)) = criteria.split_once(':') {
        let Some(stat_type) = stat_type.strip_prefix("minecraft.") else {
            return false;
        };
        let Some(value) = value.strip_prefix("minecraft.") else {
            return false;
        };
        let registry = match stat_type {
            "mined" => "block",
            "crafted" | "used" | "broken" | "picked_up" | "dropped" => "item",
            "killed" | "killed_by" => "entity_type",
            "custom" => "custom_stat",
            _ => return false,
        };
        return crate::version::snapshot::snapshot()
            .registry_contains(registry, &format!("minecraft:{value}"))
            == Some(true);
    }
    if let Some(color) = criteria
        .strip_prefix("teamkill.")
        .or_else(|| criteria.strip_prefix("killedByTeam."))
    {
        return crate::version::snapshot::snapshot().enum_contains("team_color", color);
    }
    matches!(
        criteria,
        "dummy"
            | "trigger"
            | "deathCount"
            | "playerKillCount"
            | "totalKillCount"
            | "health"
            | "food"
            | "air"
            | "armor"
            | "xp"
            | "level"
    )
}

/// 显示名与数字格式组件的轻量检查：递归检查颜色与悬停内容
/// （不需要符号表，因此可以在收集阶段调用）。
fn validate_objective_component(component: &TextComponent, diagnostics: &mut Vec<Diagnostic>) {
    if let Some(color) = &component.style.color
        && !components::valid_component_color(color)
    {
        diagnostics.push(Diagnostic::new(
            format!("`{color}` 不是有效的文本颜色（16 个颜色名或 `#rrggbb`）"),
            component.span,
        ));
    }
    if let Some(hover) = &component.style.hover {
        match hover {
            HoverEvent::Text(value) => validate_objective_component(value, diagnostics),
            HoverEvent::Entity {
                name: Some(name), ..
            } => {
                validate_objective_component(name, diagnostics);
            }
            HoverEvent::Item { .. } | HoverEvent::Entity { name: None, .. } => {}
        }
    }
    if let TextComponentKind::Translate { args, .. } = &component.kind {
        for arg in args {
            validate_objective_component(arg, diagnostics);
        }
    }
    if let TextComponentKind::Object(object) = &component.kind {
        let fallback = match object {
            ObjectContent::Atlas { fallback, .. } | ObjectContent::Player { fallback, .. } => {
                fallback
            }
        };
        if let Some(fallback) = fallback {
            validate_objective_component(fallback, diagnostics);
        }
    }
}

pub(super) fn collect_queries<'a>(
    program: &'a Program,
    diagnostics: &mut Vec<Diagnostic>,
) -> HashMap<&'a str, &'a EntityQueryDecl> {
    let mut names = HashSet::new();
    let mut queries = HashMap::new();
    for query in &program.queries {
        validate_identifier("实体查询", &query.name, query.span, diagnostics);
        if !names.insert(query.name.as_str()) {
            diagnostics.push(Diagnostic::new(
                format!("重复声明实体查询 `{}`", query.name),
                query.span,
            ));
        }
        queries.insert(query.name.as_str(), query);
        validate_entity_query(query, diagnostics);
    }
    queries
}

pub(super) fn collect_item_stacks<'a>(
    program: &'a Program,
    diagnostics: &mut Vec<Diagnostic>,
) -> HashMap<&'a str, &'a ItemStackDecl> {
    let mut item_stacks = HashMap::new();
    for item in &program.item_stacks {
        validate_identifier("物品定义", &item.name, item.span, diagnostics);
        if item_stacks.insert(item.name.as_str(), item).is_some() {
            diagnostics.push(Diagnostic::new(
                format!("重复声明物品定义 `{}`", item.name),
                item.span,
            ));
        }
        validate_item_stack(item, diagnostics);
    }
    item_stacks
}

pub(super) fn collect_storages<'a>(
    program: &'a Program,
    diagnostics: &mut Vec<Diagnostic>,
) -> HashSet<&'a str> {
    let mut storages = HashSet::new();
    for storage in &program.storages {
        validate_identifier("物品存储", &storage.name, storage.span, diagnostics);
        if !storages.insert(storage.name.as_str()) {
            diagnostics.push(Diagnostic::new(
                format!("重复声明物品存储 `{}`", storage.name),
                storage.span,
            ));
        }
        if !valid_resource_location(&storage.storage_id) {
            diagnostics.push(Diagnostic::new(
                format!("`{}` 不是有效的存储资源位置", storage.storage_id),
                storage.span,
            ));
        }
        if !valid_nbt_path(&storage.path) {
            diagnostics.push(Diagnostic::new(
                format!("`{}` 不是受支持的存储路径", storage.path),
                storage.span,
            ));
        }
    }
    storages
}

/// 校验 JSON 资源声明，并返回可作为谓词条件引用的资源名集合。
/// 收集数据槽声明：名称唯一，键必须是受支持的 NBT 路径分段。
pub(super) fn collect_data_slots<'a>(
    program: &'a Program,
    diagnostics: &mut Vec<Diagnostic>,
) -> HashMap<&'a str, &'a DataSlotDecl> {
    let mut data_slots = HashMap::new();
    for slot in &program.data_slots {
        validate_identifier("数据槽", &slot.name, slot.name_span, diagnostics);
        if data_slots.insert(slot.name.as_str(), slot).is_some() {
            diagnostics.push(Diagnostic::new(
                format!("重复声明数据槽 `{}`", slot.name),
                slot.span,
            ));
        }
        if !components::valid_nbt_component_path(&slot.key) {
            diagnostics.push(Diagnostic::new(
                format!(
                    "`{}` 不是有效的数据槽路径；路径由点分键组成，可带 `[下标]` 与引号键",
                    slot.key
                ),
                slot.key_span,
            ));
        }
    }
    data_slots
}

/// 校验 JSON 资源声明，并返回按类型归类的资源名集合。
pub(super) fn validate_resources<'a>(
    program: &'a Program,
    diagnostics: &mut Vec<Diagnostic>,
) -> ResourceSymbols<'a> {
    let mut resources = HashSet::new();
    let mut symbols = ResourceSymbols::default();
    for resource in &program.resources {
        if !crate::version::snapshot::snapshot().resource_kind_supported(&resource.kind) {
            diagnostics.push(Diagnostic::new(
                format!("Minecraft 26.3 不支持 JSON 资源类型 `{}`", resource.kind),
                resource.span,
            ));
        }
        if !valid_resource_path(&resource.name) && !valid_user_name(&resource.name) {
            diagnostics.push(Diagnostic::new(
                format!("资源名称 `{}` 不是有效的资源路径", resource.name),
                resource.span,
            ));
        }
        // The kind may itself include a subdirectory, so different declarations
        // can still resolve to the same output file.
        if !resources.insert(format!("{}/{}", resource.kind, resource.name)) {
            diagnostics.push(Diagnostic::new(
                format!("重复声明资源 `{}/{}`", resource.kind, resource.name),
                resource.span,
            ));
        }
        if resource.kind == "tags/function"
            && program
                .function_tags
                .iter()
                .any(|tag| tag.name == resource.name)
        {
            diagnostics.push(Diagnostic::new(
                format!(
                    "函数标签 `{}` 同时由 fn_tag 和 resource 声明",
                    resource.name
                ),
                resource.span,
            ));
        }
        match serde_json::from_str::<serde_json::Value>(&resource.json) {
            Ok(value) => super::resource_schema::validate_resource(
                &resource.kind,
                &resource.name,
                &value,
                resource.span,
                diagnostics,
            ),
            Err(error) => {
                diagnostics.push(Diagnostic::new(
                    format!(
                        "资源 `{}/{}` 的 JSON 无效（JSON 第 {} 行第 {} 列）：{}",
                        resource.kind,
                        resource.name,
                        error.line(),
                        error.column(),
                        error
                    ),
                    resource.span,
                ));
            }
        }
        let bucket = match resource.kind.as_str() {
            "predicate" => &mut symbols.predicates,
            "loot_table" => &mut symbols.loot_tables,
            "recipe" => &mut symbols.recipes,
            "advancement" => &mut symbols.advancements,
            _ => continue,
        };
        bucket.insert(resource.name.as_str());
    }
    symbols
}

/// 校验函数声明本身（名称、签名、属性和返回约定），并建立签名表。
pub(super) fn collect_signatures<'a>(
    program: &'a Program,
    scores: &HashSet<&'a str>,
    diagnostics: &mut Vec<Diagnostic>,
) -> HashMap<&'a str, Signature> {
    let mut functions = HashSet::new();
    let mut signatures = HashMap::new();
    for function in &program.functions {
        validate_identifier("函数", &function.name, function.span, diagnostics);
        if !functions.insert(function.name.as_str()) {
            diagnostics.push(Diagnostic::new(
                format!("重复声明函数 `{}`", function.name),
                function.span,
            ));
        }
        signatures.insert(
            function.name.as_str(),
            Signature {
                is_macro: function.macro_signature.is_some(),
                parameters: function
                    .macro_signature
                    .as_ref()
                    .map_or(function.parameters.len(), |m| m.parameters.len()),
                returns_score: function.returns_score,
                required_context: function_context(function),
            },
        );
        validate_function_declaration(function, scores, diagnostics);
    }
    signatures
}
