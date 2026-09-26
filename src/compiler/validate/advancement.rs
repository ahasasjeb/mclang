//! 进度声明：触发器、准则、奖励与展示字段的检查。
//!
//! 触发器名来自 26.3 `CriteriaTriggers` 注册表；`criteria` 条件按触发器字段
//! 快照校验，并递归检查战利品条件的类型注册与常用组合节点结构。
//! 其余引用（父进度、奖励函数、战利品表、配方、图标）在本命名空间内要求
//! 声明存在，字符串形式的外部引用只校验资源位置。

use std::collections::{HashMap, HashSet};

use crate::ast::{AdvancementDecl, AdvancementReference, Program};
use crate::compiler::types::ExecutionContext;
use crate::diagnostic::Diagnostic;

use super::ResourceSymbols;
use super::Signature;
use super::rules::{valid_resource_location, valid_resource_path, validate_identifier};

/// 26.3 `CriteriaTriggers` 注册的全部触发器名。
///
/// 取自 `net/minecraft/advancements/triggers/CriteriaTriggers.java` 的注册调用；
/// 1.1 版本数据生成器落地后改为读取快照。
const TRIGGERS: &[&str] = &[
    "allay_drop_item_on_block",
    "any_block_use",
    "avoid_vibration",
    "bee_nest_destroyed",
    "bred_animals",
    "brewed_potion",
    "changed_dimension",
    "channeled_lightning",
    "consume_item",
    "construct_beacon",
    "crafter_recipe_crafted",
    "cured_zombie_villager",
    "default_block_use",
    "effects_changed",
    "enchanted_item",
    "enter_block",
    "entity_hurt_player",
    "entity_killed_player",
    "fall_after_explosion",
    "fall_from_height",
    "filled_bucket",
    "fishing_rod_hooked",
    "hero_of_the_village",
    "impossible",
    "inventory_changed",
    "item_durability_changed",
    "item_used_on_block",
    "kill_mob_near_sculk_catalyst",
    "killed_by_arrow",
    "levitation",
    "lightning_strike",
    "location",
    "nether_travel",
    "placed_block",
    "player_generates_container_loot",
    "player_hurt_entity",
    "player_interacted_with_entity",
    "player_killed_entity",
    "player_sheared_equipment",
    "recipe_crafted",
    "recipe_unlocked",
    "ride_entity_in_lava",
    "shot_crossbow",
    "slide_down_block",
    "slept_in_bed",
    "spear_mobs",
    "started_riding",
    "summoned_entity",
    "tame_animal",
    "target_hit",
    "thrown_item_picked_up_by_entity",
    "thrown_item_picked_up_by_player",
    "tick",
    "used_ender_eye",
    "used_totem",
    "using_item",
    "villager_trade",
    "voluntary_exile",
];

/// 去掉可选的 `minecraft:` 前缀后，触发器名是否在 26.3 注册表中。
pub(super) fn normalize_trigger(value: &str) -> Option<&str> {
    let name = value.strip_prefix("minecraft:").unwrap_or(value);
    TRIGGERS.contains(&name).then_some(name)
}

/// 纹理资源位置：允许完整资源位置，也允许省略命名空间的路径形式。
fn valid_texture_location(value: &str) -> bool {
    if value.contains(':') {
        valid_resource_location(value)
    } else {
        valid_resource_path(value)
    }
}

/// 校验全部进度声明，并返回以名称为键的映射。
pub(super) fn collect_advancements<'a>(
    program: &'a Program,
    resources: &ResourceSymbols<'a>,
    item_stacks: &HashMap<&'a str, &'a crate::ast::ItemStackDecl>,
    signatures: &HashMap<&'a str, Signature>,
    diagnostics: &mut Vec<Diagnostic>,
) -> HashMap<&'a str, &'a AdvancementDecl> {
    let mut advancements = HashMap::new();
    for advancement in &program.advancements {
        validate_identifier(
            "进度",
            &advancement.name,
            advancement.name_span,
            diagnostics,
        );
        if advancements
            .insert(advancement.name.as_str(), advancement)
            .is_some()
        {
            diagnostics.push(Diagnostic::new(
                format!("重复声明进度 `{}`", advancement.name),
                advancement.span,
            ));
        }
        if resources.advancements.contains(advancement.name.as_str()) {
            diagnostics.push(Diagnostic::new(
                format!(
                    "进度 `{}` 与 resource advancement 声明重名",
                    advancement.name
                ),
                advancement.name_span,
            ));
        }
    }
    for advancement in &program.advancements {
        validate_advancement(
            advancement,
            &advancements,
            resources,
            item_stacks,
            signatures,
            diagnostics,
        );
    }
    detect_parent_cycles(&advancements, diagnostics);
    advancements
}

fn validate_advancement(
    advancement: &AdvancementDecl,
    advancements: &HashMap<&str, &AdvancementDecl>,
    resources: &ResourceSymbols<'_>,
    item_stacks: &HashMap<&str, &crate::ast::ItemStackDecl>,
    signatures: &HashMap<&str, Signature>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut criteria = HashSet::new();
    for criterion in &advancement.criteria {
        validate_identifier("准则", &criterion.name, criterion.name_span, diagnostics);
        if !criteria.insert(criterion.name.as_str()) {
            diagnostics.push(Diagnostic::new(
                format!(
                    "进度 `{}` 重复声明准则 `{}`",
                    advancement.name, criterion.name
                ),
                criterion.name_span,
            ));
        }
        if normalize_trigger(&criterion.trigger).is_none() {
            diagnostics.push(Diagnostic::new(
                format!(
                    "`{}` 不是 26.3 注册的进度触发器；常见值有 placed_block、enter_block、item_used_on_block",
                    criterion.trigger
                ),
                criterion.trigger_span,
            ));
        }
        if let Some(json) = &criterion.conditions {
            let span = criterion.conditions_span.unwrap_or(criterion.span);
            match json {
                crate::ast::AdvancementConditions::Parsed(conditions) => {
                    if let Some(trigger) = normalize_trigger(&criterion.trigger) {
                        validate_conditions(trigger, conditions, span, diagnostics);
                    }
                }
                crate::ast::AdvancementConditions::Invalid {
                    line,
                    column,
                    message,
                } => {
                    diagnostics.push(Diagnostic::new(
                        format!(
                            "准则 `{}` 的 conditions JSON 无效（JSON 第 {} 行第 {} 列）：{}",
                            criterion.name, line, column, message
                        ),
                        span,
                    ));
                }
            }
        }
    }

    if let Some(parent) = &advancement.parent {
        let exists = advancements.contains_key(parent.name.as_str())
            || resources.advancements.contains(parent.name.as_str());
        validate_reference(parent, "父进度", exists, diagnostics);
    }

    if let Some(reward) = &advancement.reward {
        if let Some(function) = &reward.function {
            let signature = signatures.get(function.name.as_str());
            validate_reference(function, "奖励函数", signature.is_some(), diagnostics);
            if let Some(signature) = signature {
                if signature.parameters > 0 {
                    diagnostics.push(Diagnostic::new(
                        format!(
                            "奖励函数 `{}` 不能带参数，进度奖励不会绑定实参",
                            function.name
                        ),
                        function.span,
                    ));
                }
                if !ExecutionContext::Player.satisfies(signature.required_context) {
                    diagnostics.push(Diagnostic::new(
                        format!(
                            "奖励函数 `{}` 要求非玩家实体上下文，但进度奖励以玩家身份运行",
                            function.name
                        ),
                        function.span,
                    ));
                }
            }
        }
        for loot in &reward.loot {
            validate_reference(
                loot,
                "战利品表",
                resources.loot_tables.contains(loot.name.as_str()),
                diagnostics,
            );
        }
        for recipe in &reward.recipes {
            validate_reference(
                recipe,
                "配方",
                resources.recipes.contains(recipe.name.as_str()),
                diagnostics,
            );
        }
    }

    if let Some(display) = &advancement.display {
        if !item_stacks.contains_key(display.icon.as_str()) {
            diagnostics.push(Diagnostic::new(
                format!("找不到图标物品定义 `{}`", display.icon),
                display.icon_span,
            ));
        }
        if let Some(background) = &display.background
            && !valid_texture_location(background)
        {
            diagnostics.push(Diagnostic::new(
                format!("`{background}` 不是有效的纹理资源位置"),
                display.background_span.unwrap_or(display.span),
            ));
        }
        let is_root = advancement.parent.is_none();
        match (is_root, display.background.is_some()) {
            (true, false) => diagnostics.push(Diagnostic::new(
                "根进度的 display 必须声明 background",
                display.span,
            )),
            (false, true) => diagnostics.push(Diagnostic::new(
                "只有根进度可以声明 background",
                display.background_span.unwrap_or(display.span),
            )),
            _ => {}
        }
    }
}

/// 校验已知触发器的 `conditions` 结构。
///
/// 字段表来自 26.3 源码里各触发器的 `TriggerInstance` 记录（见
/// `data/version/26.3/advancement_triggers.json`）；战利品条件字段可以是单个
/// holder 或 holder 列表。26.3 将旧版本的判别键 `condition` 改为 `type`。
fn validate_conditions(
    trigger: &str,
    conditions: &serde_json::Value,
    span: crate::ast::Span,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(fields) = crate::version::snapshot::snapshot().trigger_fields(trigger) else {
        return;
    };
    let Some(object) = conditions.as_object() else {
        diagnostics.push(Diagnostic::new(
            format!("触发器 `{trigger}` 的 conditions 必须是 JSON 对象"),
            span,
        ));
        return;
    };
    for (field, value) in object {
        let Some(kind) = fields.get(field) else {
            let mut message = format!("触发器 `{trigger}` 没有条件字段 `{field}`");
            if let Some(candidate) =
                crate::version::snapshot::snapshot().suggest_trigger_field(trigger, field)
            {
                message.push_str(&format!("，是否想写 `{candidate}`？"));
            } else {
                let known: Vec<&str> = fields.keys().map(String::as_str).collect();
                message.push_str(&format!("；可用字段：{}", known.join("、")));
            }
            diagnostics.push(Diagnostic::new(message, span));
            continue;
        };
        match kind.as_str() {
            "loot_condition" => validate_loot_condition(field, value, span, diagnostics),
            "loot_condition_list" => validate_loot_condition_list(field, value, span, diagnostics),
            // `listOf()` 字段只接受数组；元素本身的具体字段留给各自的 schema。
            "item_predicate_list" | "entity_predicate_list" if !value.is_array() => {
                diagnostics.push(Diagnostic::new(
                    format!(
                        "触发器 `{trigger}` 的 `{field}` 需要数组；26.3 的 codec 是 `listOf()`，单个对象或字符串不是合法写法"
                    ),
                    span,
                ));
            }
            _ => {}
        }
    }
}

fn validate_loot_condition_list(
    label: &str,
    value: &serde_json::Value,
    span: crate::ast::Span,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(conditions) = value.as_array() else {
        diagnostics.push(Diagnostic::new(
            format!("`{label}` 需要战利品条件数组"),
            span,
        ));
        return;
    };
    for (index, condition) in conditions.iter().enumerate() {
        let item_label = format!("{label}[{}]", index + 1);
        validate_loot_condition(&item_label, condition, span, diagnostics);
    }
}

fn validate_loot_condition(
    label: &str,
    value: &serde_json::Value,
    span: crate::ast::Span,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match value {
        serde_json::Value::String(reference) => {
            if !valid_resource_location(reference) {
                diagnostics.push(Diagnostic::new(
                    format!("`{label}` 的谓词引用 `{reference}` 不是有效的资源位置"),
                    span,
                ));
            }
        }
        serde_json::Value::Object(condition) => {
            validate_loot_condition_object(label, condition, span, diagnostics);
        }
        _ => diagnostics.push(Diagnostic::new(
            format!("`{label}` 需要谓词资源字符串或带 `type` 的内联条件对象"),
            span,
        )),
    }
}

fn validate_loot_condition_object(
    label: &str,
    condition: &serde_json::Map<String, serde_json::Value>,
    span: crate::ast::Span,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(raw_type) = condition.get("type").and_then(serde_json::Value::as_str) else {
        let message = if condition.contains_key("condition") {
            format!("`{label}` 使用了 `condition` 作判别键，26.3 已改为 `type`")
        } else {
            format!("`{label}` 的内联战利品条件缺少字符串 `type`；26.3 的判别键是 `type`")
        };
        diagnostics.push(Diagnostic::new(message, span));
        return;
    };

    let condition_type = canonical_resource_location(raw_type);
    if !valid_resource_location(&condition_type) {
        diagnostics.push(Diagnostic::new(
            format!("`{label}` 的战利品条件 type `{raw_type}` 不是有效的资源位置"),
            span,
        ));
        return;
    }
    if crate::version::snapshot::snapshot()
        .registry_contains_exact("loot_condition_type", &condition_type)
        == Some(false)
    {
        let mut message = format!(
            "`{label}` 的战利品条件 type `{condition_type}` 未在 Minecraft 26.3 的 loot condition 注册表中"
        );
        if let Some(candidate) = crate::version::snapshot::snapshot()
            .suggest_registry_id("loot_condition_type", &condition_type)
        {
            message.push_str(&format!("；是否想写 `{candidate}`？"));
        }
        diagnostics.push(Diagnostic::new(message, span));
        return;
    }

    match condition_type.as_str() {
        "minecraft:inverted" => match condition.get("term") {
            Some(term) => {
                validate_loot_condition_reference(&format!("{label}.term"), term, span, diagnostics)
            }
            None => diagnostics.push(Diagnostic::new(
                format!("`{label}` 的 inverted 条件缺少 `term`"),
                span,
            )),
        },
        "minecraft:all_of" | "minecraft:any_of" => {
            validate_loot_condition_terms(label, condition.get("terms"), span, diagnostics);
        }
        _ => {}
    }
}

fn validate_loot_condition_terms(
    label: &str,
    terms: Option<&serde_json::Value>,
    span: crate::ast::Span,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match terms {
        Some(serde_json::Value::Array(terms)) => {
            for (index, term) in terms.iter().enumerate() {
                validate_loot_condition_reference(
                    &format!("{label}.terms[{}]", index + 1),
                    term,
                    span,
                    diagnostics,
                );
            }
        }
        Some(serde_json::Value::String(tag)) if tag.starts_with('#') => {
            if !valid_resource_location(&canonical_resource_location(&tag[1..])) {
                diagnostics.push(Diagnostic::new(
                    format!("`{label}` 的组合条件 terms 标签引用 `{tag}` 无效"),
                    span,
                ));
            }
        }
        Some(_) => diagnostics.push(Diagnostic::new(
            format!("`{label}` 的组合条件 `terms` 必须是条件数组或 #标签引用"),
            span,
        )),
        None => diagnostics.push(Diagnostic::new(
            format!("`{label}` 的组合条件缺少 `terms`"),
            span,
        )),
    }
}

fn validate_loot_condition_reference(
    label: &str,
    value: &serde_json::Value,
    span: crate::ast::Span,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match value {
        serde_json::Value::String(reference) => {
            let canonical = canonical_resource_location(reference);
            if !valid_resource_location(&canonical) {
                diagnostics.push(Diagnostic::new(
                    format!("`{label}` 的谓词引用 `{reference}` 不是有效的资源位置"),
                    span,
                ));
            }
        }
        serde_json::Value::Object(condition) => {
            validate_loot_condition_object(label, condition, span, diagnostics);
        }
        _ => diagnostics.push(Diagnostic::new(
            format!("`{label}` 需要资源引用字符串或带 `type` 的内联条件对象"),
            span,
        )),
    }
}

fn canonical_resource_location(value: &str) -> String {
    if value.contains(':') {
        value.to_owned()
    } else {
        format!("minecraft:{value}")
    }
}

/// 检测本命名空间 `parent` 链里的环；环会让原版进度树无法加载。
/// 映射表本身无序，按声明顺序排序后再遍历，保证诊断顺序稳定。
fn detect_parent_cycles(
    advancements: &HashMap<&str, &AdvancementDecl>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut declarations = advancements
        .iter()
        .map(|(name, advancement)| (*name, *advancement))
        .collect::<Vec<_>>();
    declarations.sort_by_key(|(_, advancement)| (advancement.span.source, advancement.span.start));
    let mut reaches_cycle = HashMap::with_capacity(declarations.len());
    let mut path = Vec::new();
    let mut visited = HashSet::new();
    for &(name, _) in &declarations {
        if reaches_cycle.contains_key(name) {
            continue;
        }

        path.clear();
        visited.clear();
        let mut current_name = name;
        let reaches_cycle_from_start = loop {
            if let Some(&reaches_cycle) = reaches_cycle.get(current_name) {
                break reaches_cycle;
            }
            if !visited.insert(current_name) {
                break true;
            }
            path.push(current_name);

            let Some(current) = advancements.get(current_name) else {
                break false;
            };
            let Some(parent) = &current.parent else {
                break false;
            };
            if parent.external {
                break false;
            }
            current_name = parent.name.as_str();
            if !advancements.contains_key(current_name) {
                break false;
            }
        };
        for name in &path {
            reaches_cycle.insert(*name, reaches_cycle_from_start);
        }
    }

    for (name, advancement) in declarations {
        if reaches_cycle.get(name) == Some(&true) {
            diagnostics.push(Diagnostic::new(
                format!("进度 `{name}` 的 parent 链形成循环"),
                advancement.span,
            ));
        }
    }
}

/// 校验一条资源引用：本命名空间引用要求存在，外部字符串只检查资源位置。
fn validate_reference(
    reference: &AdvancementReference,
    label: &str,
    exists: bool,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if reference.external {
        if !valid_resource_location(&reference.name) {
            diagnostics.push(Diagnostic::new(
                format!("`{}` 不是有效的{label}资源位置", reference.name),
                reference.span,
            ));
        }
    } else if !exists {
        diagnostics.push(Diagnostic::new(
            format!("找不到{label} `{}`", reference.name),
            reference.span,
        ));
    }
}
