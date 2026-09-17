//! 进度声明：触发器、准则、奖励与展示字段的检查。
//!
//! 触发器名来自 26.3 `CriteriaTriggers` 注册表；`criteria` 里的条件保持
//! 原始 JSON（触发器的条件结构属于原版战利品条件树，后续批次再建模）。
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
            match serde_json::from_str::<serde_json::Value>(json) {
                Ok(conditions) => {
                    if let Some(trigger) = normalize_trigger(&criterion.trigger) {
                        validate_conditions(trigger, &conditions, span, diagnostics);
                    }
                }
                Err(error) => {
                    diagnostics.push(Diagnostic::new(
                        format!(
                            "准则 `{}` 的 conditions JSON 无效（JSON 第 {} 行第 {} 列）：{}",
                            criterion.name,
                            error.line(),
                            error.column(),
                            error
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
/// `data/version/26.3-rc-2/advancement_triggers.json`）；战利品条件字段
/// 需要谓词资源字符串或带 `type` 的内联条件对象——26.3 把旧版本的判别键
/// `condition` 改成了 `type`，写错时原版只会给出「Failed to parse」。
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
        if kind != "loot_condition" {
            continue;
        }
        match value {
            serde_json::Value::String(reference) => {
                if !valid_resource_location(reference) {
                    diagnostics.push(Diagnostic::new(
                        format!("`{field}` 的谓词引用 `{reference}` 不是有效的资源位置"),
                        span,
                    ));
                }
            }
            serde_json::Value::Object(condition) => {
                if !condition
                    .get("type")
                    .is_some_and(serde_json::Value::is_string)
                {
                    let mut message = format!(
                        "`{field}` 的内联战利品条件缺少 `type`；26.3 的判别键是 `type` 而不是 `condition`，例如 {{\"type\":\"minecraft:entity_properties\",\"entity\":\"this\",\"predicate\":{{\"minecraft:entity_type\":\"minecraft:zombie\"}}}}"
                    );
                    if condition.contains_key("condition") {
                        message = format!(
                            "`{field}` 使用了 `condition` 作判别键，26.3 已改为 `type`；例如 {{\"type\":\"minecraft:entity_properties\",\"entity\":\"this\",\"predicate\":{{\"minecraft:entity_type\":\"minecraft:zombie\"}}}}"
                        );
                    }
                    diagnostics.push(Diagnostic::new(message, span));
                }
            }
            _ => diagnostics.push(Diagnostic::new(
                format!("`{field}` 需要谓词资源字符串或带 `type` 的内联条件对象"),
                span,
            )),
        }
    }
}

/// 检测本命名空间 `parent` 链里的环；环会让原版进度树无法加载。
fn detect_parent_cycles(
    advancements: &HashMap<&str, &AdvancementDecl>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for (name, advancement) in advancements {
        let mut visited = HashSet::new();
        let mut current = *advancement;
        while let Some(parent) = &current.parent {
            if parent.external {
                break;
            }
            let parent_name = parent.name.as_str();
            if parent_name == *name || !visited.insert(parent_name) {
                diagnostics.push(Diagnostic::new(
                    format!("进度 `{name}` 的 parent 链形成循环"),
                    advancement.span,
                ));
                break;
            }
            let Some(next) = advancements.get(parent_name).copied() else {
                break;
            };
            current = next;
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
