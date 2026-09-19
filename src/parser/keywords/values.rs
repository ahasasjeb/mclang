use crate::ast::{AdvancementFrame, AdvancementRequirements, Attribute, ItemRarity};

use super::table::{ATTRIBUTES, KEYWORDS};

pub(crate) fn attribute_word(value: &str) -> Option<Attribute> {
    let english = ATTRIBUTES
        .iter()
        .find(|keyword| keyword.english == value || keyword.chinese == value)?
        .english;
    match english {
        "load" => Some(Attribute::Load),
        "tick" => Some(Attribute::Tick),
        "entity" => Some(Attribute::Entity),
        "player" => Some(Attribute::Player),
        "non_player" => Some(Attribute::NonPlayer),
        _ => unreachable!("ATTRIBUTES 表与 attribute_word 不同步"),
    }
}

pub(crate) fn keyword_alias(english: &str) -> Option<&'static str> {
    KEYWORDS
        .iter()
        .find(|keyword| keyword.english == english)
        .map(|keyword| keyword.chinese)
}

pub(crate) fn word_matches(value: &str, english: &str) -> bool {
    value == english || keyword_alias(english).is_some_and(|alias| alias == value)
}

/// 不能用作标识符的保留字：全部关键词（中英文）和布尔字面量。
///
/// 属性名（`load`、`tick`、`player` 等）不算保留字：`@` 前缀已经把它们和标识符
/// 区分开，`@load fn load()` 这类自然命名应当保持可用。
pub(crate) fn reserved_word(value: &str) -> bool {
    KEYWORDS
        .iter()
        .any(|keyword| keyword.english == value || keyword.chinese == value)
        || matches!(value, "true" | "false" | "真" | "假")
}

pub(crate) fn query_property(value: &str) -> Option<&'static str> {
    match value {
        "tag" | "标签" => Some("tag"),
        "without_tag" | "排除标签" => Some("without_tag"),
        "type" | "实体类型" => Some("type"),
        "without_type" | "排除类型" => Some("without_type"),
        "limit" | "上限" => Some("limit"),
        "within" | "范围" => Some("within"),
        "sort" | "排序" => Some("sort"),
        "name" | "名称" => Some("name"),
        "without_name" | "排除名称" => Some("without_name"),
        "scores" | "分数" => Some("scores"),
        "nbt" | "数据" => Some("nbt"),
        "without_nbt" | "排除数据" => Some("without_nbt"),
        "box" | "坐标盒" => Some("box"),
        "distance" | "距离" => Some("distance"),
        "level" | "等级" => Some("level"),
        "gamemode" | "游戏模式" => Some("gamemode"),
        "team" | "队伍" => Some("team"),
        "without_team" | "排除队伍" => Some("without_team"),
        "rotate" | "旋转" => Some("rotate"),
        "predicate" | "谓词" => Some("predicate"),
        "advancements" | "进度过滤" => Some("advancements"),
        "item" | "物品" => Some("item"),
        _ => None,
    }
}

/// 游戏模式取值（选择器 `gamemode=` 与 `gamemode` 命令共用）。
pub(crate) fn gamemode_value(value: &str) -> Option<&'static str> {
    match value {
        "survival" | "生存" => Some("survival"),
        "creative" | "创造" => Some("creative"),
        "adventure" | "冒险" => Some("adventure"),
        "spectator" | "旁观" => Some("spectator"),
        _ => None,
    }
}

pub(crate) fn item_property(value: &str) -> Option<&'static str> {
    match value {
        "id" | "类型" => Some("id"),
        "count" | "数量" => Some("count"),
        "custom_name" | "自定义名称" => Some("custom_name"),
        _ => None,
    }
}

/// `fn_tag` 声明体允许的属性与中文别名。
pub(crate) fn function_tag_property(value: &str) -> Option<&'static str> {
    match value {
        "value" | "值" => Some("value"),
        "replace" | "替换" => Some("replace"),
        _ => None,
    }
}

pub(crate) fn item_stack_property(value: &str) -> Option<&'static str> {
    match value {
        "components" | "组件" => Some("components"),
        "count" | "数量" => Some("count"),
        "custom_name" | "自定义名称" => Some("custom_name"),
        "item_name" | "物品名称" => Some("item_name"),
        "lore" | "描述" => Some("lore"),
        "enchantment" | "附魔" => Some("enchantment"),
        "stored_enchantment" | "存储附魔" => Some("stored_enchantment"),
        "damage" | "损伤" => Some("damage"),
        "max_damage" | "最大损伤" => Some("max_damage"),
        "max_stack_size" | "最大堆叠" => Some("max_stack_size"),
        "rarity" | "稀有度" => Some("rarity"),
        "item_model" | "物品模型" => Some("item_model"),
        "dyed_color" | "染色" => Some("dyed_color"),
        "enchantment_glint_override" | "附魔光效" => Some("enchantment_glint_override"),
        "unbreakable" | "无法破坏" => Some("unbreakable"),
        "custom_data" | "自定义数据" => Some("custom_data"),
        _ => None,
    }
}

/// 类型化物品查询目前只支持 `contents` 槽；`内容` 是它的中文写法。
pub(crate) fn slot_name(value: &str) -> Option<&'static str> {
    match value {
        "contents" | "内容" => Some("contents"),
        _ => None,
    }
}

/// `resource` 声明允许把常用资源类型写成中文。
pub(crate) fn resource_kind(value: &str) -> Option<&'static str> {
    match value {
        "recipe" | "配方" => Some("recipe"),
        "predicate" | "谓词" => Some("predicate"),
        _ => None,
    }
}

/// `advancement.grant/revoke` 的方法名，返回操作与作用范围。
pub(crate) fn advancement_method(value: &str) -> Option<(&'static str, &'static str)> {
    match value {
        "grant" | "授予" => Some(("grant", "only")),
        "grant_through" | "授予至" => Some(("grant", "through")),
        "grant_from" | "授予从" => Some(("grant", "from")),
        "grant_until" | "授予直到" => Some(("grant", "until")),
        "grant_everything" | "授予全部" => Some(("grant", "everything")),
        "revoke" | "撤销" => Some(("revoke", "only")),
        "revoke_through" | "撤销至" => Some(("revoke", "through")),
        "revoke_from" | "撤销从" => Some(("revoke", "from")),
        "revoke_until" | "撤销直到" => Some(("revoke", "until")),
        "revoke_everything" | "撤销全部" => Some(("revoke", "everything")),
        _ => None,
    }
}

/// `advancement` 声明块的顶层属性。
pub(crate) fn advancement_property(value: &str) -> Option<&'static str> {
    match value {
        "parent" | "父进度" => Some("parent"),
        "criterion" | "准则" => Some("criterion"),
        "requirements" | "要求" => Some("requirements"),
        "reward" | "奖励" => Some("reward"),
        "display" | "展示" => Some("display"),
        _ => None,
    }
}

/// `criterion` 块内的属性。
pub(crate) fn criterion_property(value: &str) -> Option<&'static str> {
    match value {
        "trigger" | "触发器" => Some("trigger"),
        "conditions" | "条件" => Some("conditions"),
        _ => None,
    }
}

/// `reward` 块内的属性。
pub(crate) fn reward_property(value: &str) -> Option<&'static str> {
    match value {
        "function" | "函数" => Some("function"),
        "experience" | "经验" => Some("experience"),
        "loot" | "战利品" => Some("loot"),
        "recipe" | "配方" => Some("recipe"),
        _ => None,
    }
}

/// `display` 块内的属性。
pub(crate) fn display_property(value: &str) -> Option<&'static str> {
    match value {
        "icon" | "图标" => Some("icon"),
        "title" | "标题" => Some("title"),
        "description" | "描述" => Some("description"),
        "frame" | "框架" => Some("frame"),
        "background" | "背景" => Some("background"),
        "show_toast" | "显示提示" => Some("show_toast"),
        "announce_to_chat" | "聊天公告" => Some("announce_to_chat"),
        "hidden" | "隐藏" => Some("hidden"),
        _ => None,
    }
}

/// `display.frame` 的三种进度框样式。
pub(crate) fn advancement_frame(value: &str) -> Option<AdvancementFrame> {
    match value {
        "task" | "任务" => Some(AdvancementFrame::Task),
        "goal" | "目标" => Some(AdvancementFrame::Goal),
        "challenge" | "挑战" => Some(AdvancementFrame::Challenge),
        _ => None,
    }
}

/// `requirements` 的两种完成策略。
pub(crate) fn advancement_requirements(value: &str) -> Option<AdvancementRequirements> {
    match value {
        "all" | "全部" => Some(AdvancementRequirements::All),
        "any" | "任意" => Some(AdvancementRequirements::Any),
        _ => None,
    }
}

pub(crate) fn rarity_value(value: &str) -> Option<ItemRarity> {
    match value {
        "common" | "普通" => Some(ItemRarity::Common),
        "uncommon" | "罕见" => Some(ItemRarity::Uncommon),
        "rare" | "稀有" => Some(ItemRarity::Rare),
        "epic" | "史诗" => Some(ItemRarity::Epic),
        _ => None,
    }
}

pub(crate) fn entity_sort(value: &str) -> Option<&'static str> {
    match value {
        "nearest" | "最近" => Some("nearest"),
        "furthest" | "最远" => Some("furthest"),
        "random" | "随机" => Some("random"),
        "arbitrary" | "任意" => Some("arbitrary"),
        _ => None,
    }
}
