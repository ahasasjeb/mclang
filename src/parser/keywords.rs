//! 关键词、属性和枚举值的中英文规范化表。
//!
//! [`KEYWORDS`] 是语言关键词的唯一来源：解析器的 `word_matches`、保留字检查和
//! 诊断文本都从这里派生，新增关键词只改这一处。枚举值（排序、稀有度、颜色、
//! 声音分类等）按领域保持独立函数，规范形式统一为英文；中文别名只在解析边界
//! 出现，进入 AST 后所有阶段只处理英文规范值。

use crate::ast::{Attribute, ItemRarity};

/// 语言关键词的规范英文写法与中文别名。
pub(crate) struct Keyword {
    pub english: &'static str,
    pub chinese: &'static str,
}

/// 全部语言关键词。中英文写法都必须全局唯一，测试 `keywords_are_unique` 保证这一点。
pub(crate) const KEYWORDS: &[Keyword] = &[
    Keyword {
        english: "namespace",
        chinese: "命名空间",
    },
    Keyword {
        english: "score",
        chinese: "计分",
    },
    Keyword {
        english: "query",
        chinese: "查询",
    },
    Keyword {
        english: "entity",
        chinese: "实体",
    },
    Keyword {
        english: "item",
        chinese: "物品",
    },
    Keyword {
        english: "item_stack",
        chinese: "物品堆",
    },
    Keyword {
        english: "item_list",
        chinese: "物品列表",
    },
    Keyword {
        english: "storage",
        chinese: "存储",
    },
    Keyword {
        english: "resource",
        chinese: "资源",
    },
    Keyword {
        english: "predicate",
        chinese: "谓词",
    },
    Keyword {
        english: "fn",
        chinese: "函数",
    },
    Keyword {
        english: "let",
        chinese: "令",
    },
    Keyword {
        english: "return",
        chinese: "返回",
    },
    Keyword {
        english: "if",
        chinese: "如果",
    },
    Keyword {
        english: "else",
        chinese: "否则",
    },
    Keyword {
        english: "while",
        chinese: "当",
    },
    Keyword {
        english: "each",
        chinese: "遍历",
    },
    Keyword {
        english: "call",
        chinese: "调用",
    },
    Keyword {
        english: "schedule",
        chinese: "调度",
    },
    Keyword {
        english: "after",
        chinese: "延后",
    },
    Keyword {
        english: "append",
        chinese: "追加",
    },
    Keyword {
        english: "replace",
        chinese: "替换",
    },
    Keyword {
        english: "give",
        chinese: "给予",
    },
    Keyword {
        english: "origin",
        chinese: "投掷者",
    },
    Keyword {
        english: "in_dimension",
        chinese: "在维度",
    },
    Keyword {
        english: "spawn",
        chinese: "召唤",
    },
    Keyword {
        english: "self",
        chinese: "自身",
    },
    Keyword {
        english: "message",
        chinese: "消息",
    },
    Keyword {
        english: "sound",
        chinese: "声音",
    },
    Keyword {
        english: "run",
        chinese: "原生命令",
    },
    Keyword {
        english: "execute",
        chinese: "原生执行",
    },
    Keyword {
        english: "contents",
        chinese: "内容",
    },
];

/// 函数属性的规范英文写法与中文别名，`@` 之后使用。
pub(crate) const ATTRIBUTES: &[Keyword] = &[
    Keyword {
        english: "load",
        chinese: "加载",
    },
    Keyword {
        english: "tick",
        chinese: "每刻",
    },
    Keyword {
        english: "entity",
        chinese: "实体",
    },
    Keyword {
        english: "player",
        chinese: "玩家",
    },
    Keyword {
        english: "non_player",
        chinese: "非玩家",
    },
];

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

pub(super) fn word_matches(value: &str, english: &str) -> bool {
    value == english || keyword_alias(english).is_some_and(|alias| alias == value)
}

/// 不能用作标识符的保留字：全部规范关键词和布尔字面量。
///
/// 属性名（`load`、`tick`、`player` 等）不算保留字：`@` 前缀已经把它们和标识符
/// 区分开，`@load fn load()` 这类自然命名应当保持可用。
pub(crate) fn reserved_word(value: &str) -> bool {
    KEYWORDS.iter().any(|keyword| keyword.english == value) || matches!(value, "true" | "false")
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

/// 类型化物品查询目前只支持 `contents` 槽；`内容` 是它的中文写法。
pub(super) fn slot_name(value: &str) -> Option<&'static str> {
    match value {
        "contents" | "内容" => Some("contents"),
        _ => None,
    }
}

/// `resource` 声明允许把常用资源类型写成中文。
pub(super) fn resource_kind(value: &str) -> Option<&'static str> {
    match value {
        "predicate" | "谓词" => Some("predicate"),
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

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn keywords_are_unique() {
        let mut english = HashSet::new();
        let mut chinese = HashSet::new();
        for keyword in KEYWORDS {
            assert!(
                english.insert(keyword.english),
                "重复的英文关键词 `{}`",
                keyword.english
            );
            assert!(
                chinese.insert(keyword.chinese),
                "重复的中文关键词 `{}`",
                keyword.chinese
            );
            assert_ne!(keyword.english, keyword.chinese);
        }
        let mut attribute_english = HashSet::new();
        let mut attribute_chinese = HashSet::new();
        for attribute in ATTRIBUTES {
            assert!(
                attribute_english.insert(attribute.english),
                "重复的英文属性 `@{}`",
                attribute.english
            );
            assert!(
                attribute_chinese.insert(attribute.chinese),
                "重复的中文属性 `@{}`",
                attribute.chinese
            );
        }
    }

    #[test]
    fn aliases_resolve_in_both_directions() {
        for keyword in KEYWORDS {
            assert!(word_matches(keyword.english, keyword.english));
            assert!(word_matches(keyword.chinese, keyword.english));
            for other in KEYWORDS
                .iter()
                .filter(|other| other.english != keyword.english)
            {
                assert!(
                    !word_matches(keyword.chinese, other.english),
                    "`{}` 同时匹配 `{}` 和 `{}`",
                    keyword.chinese,
                    keyword.english,
                    other.english
                );
            }
        }
        for attribute in ATTRIBUTES {
            assert!(attribute_word(attribute.english).is_some());
            assert!(attribute_word(attribute.chinese).is_some());
        }
    }
}
