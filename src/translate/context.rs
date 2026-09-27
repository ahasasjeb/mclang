//! 位置规则表：哪些词在什么位置属于语言词汇。
//!
//! 同一张别名表可能出现在不同语境里（`clear` 既是天气又是效果方法），而方法名、
//! 属性名又与用户标识符共用拼写，所以翻译必须看出现位置。这些表与
//! `docs/tools/translate.mjs` 的常量一一对应：先按位置选表，再查 `tables` 里
//! 由解析器函数构建的中英对照。
//!
//! 关键词与函数属性不在这里：它们是保留字，出现即语言词汇，只在 NBT 字面量、
//! `import` 名称等数据位置例外，由改写逻辑单独处理。

/// 接收者关键词（英文规范名）→ 方法表名。中文写法由关键词表提供。
pub(super) const RECEIVER_FAMILIES: &[(&str, &str)] = &[
    ("self", "self_method"),
    ("message", "message_target"),
    ("effect", "effect_method"),
    ("xp", "xp_method"),
    ("stopwatch", "stopwatch_method"),
    ("scoreboard", "scoreboard_method"),
    ("place", "place_method"),
    ("forceload", "forceload_method"),
    ("time", "time_method"),
    ("gamerule", "gamerule_method"),
    ("worldborder", "worldborder_method"),
    ("locate", "locate_kind"),
    ("weather", "weather_kind"),
    ("schedule", "schedule_method"),
    ("advancement", "advancement_method"),
    ("store", "store_method"),
];

/// 这些命令的参数按 `command_value`、`ui_value` 与文本颜色翻译。
pub(super) const COMMAND_RECEIVERS: &[&str] = &[
    "tag",
    "attribute",
    "ride",
    "rotate",
    "team",
    "waypoint",
    "datapack",
    "recipe",
    "loot",
    "random",
    "title",
    "bossbar",
    "dialog",
    "posteffect",
];

/// 作为独立调用（后面跟 `(`）出现的命令名。
pub(super) const COMMAND_CALLS: &[&str] = &[
    "kill",
    "enchant",
    "damage",
    "spreadplayers",
    "spectate",
    "swing",
    "trigger",
    "gamemode",
    "defaultgamemode",
    "difficulty",
    "spawnpoint",
    "setworldspawn",
    "list",
    "reload",
    "teleport",
    "has",
    "equals",
    "matches",
    "particle",
    "stopsound",
    "msg",
    "teammsg",
];

/// 表达式内建函数：`count(查询)`、`compute(...)`。关键词表优先于同名的声明属性
/// （`count` 在物品属性里是“数量”，在表达式里是“计数”）。
pub(super) const EXPRESSION_CALLS: &[&str] = &["count", "compute"];

/// 后面跟 `=` 或 `(` 时按属性表翻译的函数。
///
/// `objective_property` 必须排在 `advancement_property` 之前：`准则 = …` 只出现在
/// 计分板目标块里，含义是 `criteria`；进度块里的 `criterion` 后面跟的是准则名，
/// 不是 `=`（见 `parser/declarations/advancement.rs`）。
pub(super) const PROPERTY_FAMILIES: &[&str] = &[
    "objective_property",
    "query_property",
    "item_property",
    "item_stack_property",
    "function_tag_property",
    "slot_name",
    "resource_kind",
    "clone_dimension",
    "advancement_property",
    "criterion_property",
    "reward_property",
    "display_property",
];

/// 只在后面跟 `(` 时按函数名翻译的表：`facing(...)` 是 execute 子句，
/// `facing = "south"` 却是方块状态属性，不能混用同一张表。
pub(super) const CALL_FAMILIES: &[&str] = &["execute_clause"];

/// 调用实参里允许出现的枚举值表，键是规范化的“接收者.方法”或裸函数名。
pub(super) const CALL_VALUE_CONTEXTS: &[(&str, &[&str])] = &[
    ("sort", &["entity_sort"]),
    ("item", &["slot_name"]),
    ("message.all", &["text_color"]),
    ("message.self", &["text_color"]),
    ("message.nearest", &["text_color"]),
    ("sound.self", &["sound_source"]),
    ("stopsound", &["sound_source"]),
    ("xp.add", &["xp_kind"]),
    ("xp.set", &["xp_kind"]),
    ("xp.query", &["xp_kind"]),
    ("clone", &["clone_filter", "clone_mode", "strict_word"]),
    ("set_block", &["set_block_mode"]),
    ("fill", &["fill_mode"]),
    (
        "place.template",
        &["template_rotation", "template_mirror", "strict_word"],
    ),
    ("anchored", &["anchor_value"]),
    ("facing", &["anchor_value"]),
    ("on", &["entity_relation"]),
    ("store.data", &["store_method", "store_data_type"]),
    ("store.result", &["bossbar_field"]),
    ("store.success", &["bossbar_field"]),
];

/// `属性 = 值` 形式下的枚举值表。
pub(super) const PROPERTY_VALUE_CONTEXTS: &[(&str, &[&str])] = &[
    ("rarity", &["rarity_value"]),
    ("frame", &["advancement_frame"]),
    ("requirements", &["advancement_requirements"]),
];

/// 不经过上下文表、由改写逻辑直接使用的表。
const DIRECT_FAMILIES: &[&str] = &["command_value", "ui_value", "boolean_word", "time_unit"];

pub(super) fn call_value_families(callee: &str) -> &'static [&'static str] {
    CALL_VALUE_CONTEXTS
        .iter()
        .find(|(key, _)| *key == callee)
        .map(|(_, families)| *families)
        .unwrap_or(&[])
}

pub(super) fn property_value_families(property: &str) -> &'static [&'static str] {
    PROPERTY_VALUE_CONTEXTS
        .iter()
        .find(|(key, _)| *key == property)
        .map(|(_, families)| *families)
        .unwrap_or(&[])
}

/// 上下文规则引用到的全部表名，供词表构建自检：漏登记的表会让翻译静默失效。
pub(super) fn referenced_families() -> Vec<&'static str> {
    let mut names = Vec::new();
    names.extend(PROPERTY_FAMILIES.iter().copied());
    names.extend(CALL_FAMILIES.iter().copied());
    names.extend(RECEIVER_FAMILIES.iter().map(|(_, family)| *family));
    names.extend(DIRECT_FAMILIES.iter().copied());
    for (_, families) in CALL_VALUE_CONTEXTS.iter().chain(PROPERTY_VALUE_CONTEXTS) {
        names.extend(families.iter().copied());
    }
    names.sort_unstable();
    names.dedup();
    names
}

/// 接收者方法表反查出的规范接收者，用于把中文接收者还原成英文。
pub(super) fn canonical_receiver(family: &str) -> Option<&'static str> {
    RECEIVER_FAMILIES
        .iter()
        .find(|(_, receiver_family)| *receiver_family == family)
        .map(|(receiver, _)| *receiver)
}
