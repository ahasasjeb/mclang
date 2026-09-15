//! 关键词、属性和枚举值的中英文规范化表。
//!
//! 解析器在构造 AST 时把中英文写法归一到同一个内部值，后续阶段不再关心语言。

use crate::ast::ItemRarity;

pub(super) fn word_matches(value: &str, english: &str) -> bool {
    value == english || keyword_alias(english) == Some(value)
}

pub(super) fn keyword_alias(english: &str) -> Option<&'static str> {
    match english {
        "namespace" => Some("命名空间"),
        "score" => Some("计分"),
        "query" => Some("查询"),
        "entity" => Some("实体"),
        "storage" => Some("存储"),
        "items" => Some("物品"),
        "item" => Some("物品"),
        "item_stack" => Some("物品堆"),
        "resource" => Some("资源"),
        "fn" => Some("函数"),
        "each" => Some("遍历"),
        "give" => Some("给予"),
        "in_dimension" => Some("在维度"),
        "spawn" => Some("召唤"),
        "self" => Some("自身"),
        "message" => Some("消息"),
        "sound" => Some("声音"),
        "run" => Some("原生命令"),
        "call" => Some("调用"),
        "schedule" => Some("调度"),
        "after" => Some("延后"),
        "append" => Some("追加"),
        "replace" => Some("替换"),
        "if" => Some("如果"),
        "else" => Some("否则"),
        "while" => Some("当"),
        "execute" => Some("原生执行"),
        "return" => Some("返回"),
        "let" => Some("令"),
        "predicate" => Some("谓词"),
        _ => None,
    }
}

pub(super) fn query_property(value: &str) -> Option<&'static str> {
    match value {
        "tag" | "标签" => Some("tag"),
        "without_tag" | "排除标签" => Some("without_tag"),
        "limit" | "上限" => Some("limit"),
        "within" | "范围" => Some("within"),
        "sort" | "排序" => Some("sort"),
        "item" | "物品" => Some("item"),
        _ => None,
    }
}

pub(super) fn item_property(value: &str) -> Option<&'static str> {
    match value {
        "id" | "类型" => Some("id"),
        "count" | "数量" => Some("count"),
        "custom_name" | "自定义名称" => Some("custom_name"),
        _ => None,
    }
}

pub(super) fn item_stack_property(value: &str) -> Option<&'static str> {
    match value {
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
        _ => None,
    }
}

pub(super) fn rarity_value(value: &str) -> Option<ItemRarity> {
    match value {
        "common" | "普通" => Some(ItemRarity::Common),
        "uncommon" | "罕见" => Some(ItemRarity::Uncommon),
        "rare" | "稀有" => Some(ItemRarity::Rare),
        "epic" | "史诗" => Some(ItemRarity::Epic),
        _ => None,
    }
}

pub(super) fn entity_sort(value: &str) -> Option<&'static str> {
    match value {
        "nearest" | "最近" => Some("nearest"),
        "furthest" | "最远" => Some("furthest"),
        "random" | "随机" => Some("random"),
        "arbitrary" | "任意" => Some("arbitrary"),
        _ => None,
    }
}

pub(super) fn self_method(value: &str) -> Option<&'static str> {
    match value {
        "add_tag" | "添加标签" => Some("add_tag"),
        "remove_tag" | "移除标签" => Some("remove_tag"),
        "set_invulnerable" | "设置无敌" => Some("set_invulnerable"),
        "save_items" | "保存物品" => Some("save_items"),
        "restore_items" | "恢复物品" => Some("restore_items"),
        "remove_preserving_items" | "保存并移除" => Some("remove_preserving_items"),
        "give_item" | "给予物品" => Some("give_item"),
        "clear_items" | "清空物品" => Some("clear_items"),
        "remove" | "移除" => Some("remove"),
        "consume" | "消耗" => Some("consume"),
        "return_to_owner" | "返还投掷者" => Some("return_to_owner"),
        _ => None,
    }
}

pub(super) fn message_target(value: &str) -> Option<&'static str> {
    match value {
        "all" | "全部" => Some("all"),
        "self" | "自身" => Some("self"),
        "nearest" | "最近" => Some("nearest"),
        _ => None,
    }
}

pub(super) fn boolean_word(value: &str) -> Option<&'static str> {
    match value {
        "true" | "真" => Some("true"),
        "false" | "假" => Some("false"),
        _ => None,
    }
}

pub(super) fn time_unit(value: &str) -> Option<&'static str> {
    match value {
        "t" | "刻" => Some("t"),
        "s" | "秒" => Some("s"),
        "d" | "天" => Some("d"),
        _ => None,
    }
}

pub(super) fn text_color(value: &str) -> Option<&'static str> {
    match value {
        "black" | "黑色" => Some("black"),
        "dark_blue" | "深蓝色" => Some("dark_blue"),
        "dark_green" | "深绿色" => Some("dark_green"),
        "dark_aqua" | "深青色" => Some("dark_aqua"),
        "dark_red" | "深红色" => Some("dark_red"),
        "dark_purple" | "深紫色" => Some("dark_purple"),
        "gold" | "金色" => Some("gold"),
        "gray" | "灰色" => Some("gray"),
        "dark_gray" | "深灰色" => Some("dark_gray"),
        "blue" | "蓝色" => Some("blue"),
        "green" | "绿色" => Some("green"),
        "aqua" | "青色" => Some("aqua"),
        "red" | "红色" => Some("red"),
        "light_purple" | "亮紫色" => Some("light_purple"),
        "yellow" | "黄色" => Some("yellow"),
        "white" | "白色" => Some("white"),
        _ => None,
    }
}

pub(super) fn sound_source(value: &str) -> Option<&'static str> {
    match value {
        "master" | "主音量" => Some("master"),
        "music" | "音乐" => Some("music"),
        "record" | "唱片" => Some("record"),
        "weather" | "天气" => Some("weather"),
        "block" | "方块" => Some("block"),
        "hostile" | "敌对" => Some("hostile"),
        "neutral" | "中立" => Some("neutral"),
        "player" | "玩家" => Some("player"),
        "ambient" | "环境" => Some("ambient"),
        "voice" | "语音" => Some("voice"),
        "ui" | "界面" => Some("ui"),
        _ => None,
    }
}
