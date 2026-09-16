//! 关键词、属性和枚举值的中英文规范化表。
//!
//! [`KEYWORDS`] 是语言关键词的唯一来源：解析器的 `word_matches`、保留字检查和
//! 诊断文本都从这里派生，新增关键词只改这一处。枚举值（排序、稀有度、颜色、
//! 声音分类等）按领域保持独立函数，规范形式统一为英文；中文别名只在解析边界
//! 出现，进入 AST 后所有阶段只处理英文规范值。

use crate::ast::{
    AdvancementFrame, AdvancementRequirements, Attribute, CloneMode, FillMode, ItemRarity,
    LocateKind, SetBlockMode, TemplateMirror, TemplateRotation, WeatherKind, XpKind,
};

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
        english: "objective",
        chinese: "目标",
    },
    Keyword {
        english: "scoreboard",
        chinese: "计分板",
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
        english: "data_slot",
        chinese: "数据槽",
    },
    Keyword {
        english: "item_data",
        chinese: "物品数据",
    },
    Keyword {
        english: "entity_data",
        chinese: "实体数据",
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
        english: "advancement",
        chinese: "进度",
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
        english: "fn_tag",
        chinese: "函数标签",
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
        english: "fail",
        chinese: "失败",
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
        english: "effect",
        chinese: "效果",
    },
    Keyword {
        english: "xp",
        chinese: "经验",
    },
    Keyword {
        english: "clear",
        chinese: "清除",
    },
    Keyword {
        english: "stopwatch",
        chinese: "秒表",
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
    Keyword {
        english: "set_block",
        chinese: "设置方块",
    },
    Keyword {
        english: "fill",
        chinese: "填充",
    },
    Keyword {
        english: "fill_biome",
        chinese: "填充生物群系",
    },
    Keyword {
        english: "clone",
        chinese: "复制",
    },
    Keyword {
        english: "place",
        chinese: "放置",
    },
    Keyword {
        english: "forceload",
        chinese: "强制加载",
    },
    Keyword {
        english: "time",
        chinese: "时间",
    },
    Keyword {
        english: "weather",
        chinese: "天气",
    },
    Keyword {
        english: "gamerule",
        chinese: "游戏规则",
    },
    Keyword {
        english: "worldborder",
        chinese: "世界边界",
    },
    Keyword {
        english: "locate",
        chinese: "定位",
    },
    Keyword {
        english: "teleport",
        chinese: "传送",
    },
    Keyword {
        english: "pos",
        chinese: "坐标",
    },
    Keyword {
        english: "column",
        chinese: "列坐标",
    },
    Keyword {
        english: "block_state",
        chinese: "方块状态",
    },
    Keyword {
        english: "nbt",
        chinese: "数据",
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

pub(crate) fn word_matches(value: &str, english: &str) -> bool {
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

/// `fn_tag` 声明体允许的属性与中文别名。
pub(super) fn function_tag_property(value: &str) -> Option<&'static str> {
    match value {
        "value" | "值" => Some("value"),
        "replace" | "替换" => Some("replace"),
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
        "custom_data" | "自定义数据" => Some("custom_data"),
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

/// `advancement.grant/revoke` 的方法名，返回操作与作用范围。
pub(super) fn advancement_method(value: &str) -> Option<(&'static str, &'static str)> {
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
pub(super) fn advancement_property(value: &str) -> Option<&'static str> {
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
pub(super) fn criterion_property(value: &str) -> Option<&'static str> {
    match value {
        "trigger" | "触发器" => Some("trigger"),
        "conditions" | "条件" => Some("conditions"),
        _ => None,
    }
}

/// `reward` 块内的属性。
pub(super) fn reward_property(value: &str) -> Option<&'static str> {
    match value {
        "function" | "函数" => Some("function"),
        "experience" | "经验" => Some("experience"),
        "loot" | "战利品" => Some("loot"),
        "recipe" | "配方" => Some("recipe"),
        _ => None,
    }
}

/// `display` 块内的属性。
pub(super) fn display_property(value: &str) -> Option<&'static str> {
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
pub(super) fn advancement_frame(value: &str) -> Option<AdvancementFrame> {
    match value {
        "task" | "任务" => Some(AdvancementFrame::Task),
        "goal" | "目标" => Some(AdvancementFrame::Goal),
        "challenge" | "挑战" => Some(AdvancementFrame::Challenge),
        _ => None,
    }
}

/// `requirements` 的两种完成策略。
pub(super) fn advancement_requirements(value: &str) -> Option<AdvancementRequirements> {
    match value {
        "all" | "全部" => Some(AdvancementRequirements::All),
        "any" | "任意" => Some(AdvancementRequirements::Any),
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
        "set_no_gravity" | "设置无视重力" => Some("set_no_gravity"),
        "save_items" | "保存物品" => Some("save_items"),
        "restore_items" | "恢复物品" => Some("restore_items"),
        "remove_preserving_items" | "保存并移除" => Some("remove_preserving_items"),
        "give_item" | "给予物品" => Some("give_item"),
        "clear_items" | "清空物品" => Some("clear_items"),
        "deposit" | "存入" => Some("deposit"),
        "withdraw" | "取出" => Some("withdraw"),
        "remove_data" | "移除数据" => Some("remove_data"),
        "remove" | "移除" => Some("remove"),
        _ => None,
    }
}

/// `scoreboard.set/reset/get` 的方法名；`get` 只在表达式里使用。
pub(super) fn scoreboard_method(value: &str) -> Option<&'static str> {
    match value {
        "set" | "设置" => Some("set"),
        "reset" | "重置" => Some("reset"),
        "get" | "取" => Some("get"),
        _ => None,
    }
}

/// 数据槽声明的来源构造器。
pub(super) fn data_slot_kind(value: &str) -> Option<&'static str> {
    match value {
        "item_data" | "物品数据" => Some("item_data"),
        "entity_data" | "实体数据" => Some("entity_data"),
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

pub(super) fn effect_method(value: &str) -> Option<&'static str> {
    match value {
        "give" | "给予" => Some("give"),
        "give_infinite" | "给予无限" => Some("give_infinite"),
        "clear" | "清除" => Some("clear"),
        _ => None,
    }
}

pub(super) fn xp_method(value: &str) -> Option<&'static str> {
    match value {
        "add" | "增加" => Some("add"),
        "set" | "设置" => Some("set"),
        "query" | "查询" => Some("query"),
        _ => None,
    }
}

pub(super) fn xp_kind(value: &str) -> Option<XpKind> {
    match value {
        "points" | "点数" => Some(XpKind::Points),
        "levels" | "等级" => Some(XpKind::Levels),
        _ => None,
    }
}

pub(super) fn stopwatch_method(value: &str) -> Option<&'static str> {
    match value {
        "create" | "创建" => Some("create"),
        "query" | "查询" => Some("query"),
        "restart" | "重启" => Some("restart"),
        "remove" | "移除" => Some("remove"),
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

/// `setblock` 的放置模式。
pub(super) fn set_block_mode(value: &str) -> Option<SetBlockMode> {
    match value {
        "destroy" | "摧毁" => Some(SetBlockMode::Destroy),
        "keep" | "保留" => Some(SetBlockMode::Keep),
        "replace" | "替换" => Some(SetBlockMode::Replace),
        "strict" | "严格" => Some(SetBlockMode::Strict),
        _ => None,
    }
}

/// `fill` 的填充模式。
pub(super) fn fill_mode(value: &str) -> Option<FillMode> {
    match value {
        "replace" | "替换" => Some(FillMode::Replace),
        "outline" | "轮廓" => Some(FillMode::Outline),
        "hollow" | "空心" => Some(FillMode::Hollow),
        "destroy" | "摧毁" => Some(FillMode::Destroy),
        "strict" | "严格" => Some(FillMode::Strict),
        "keep" | "保留" => Some(FillMode::Keep),
        _ => None,
    }
}

/// `clone` 的过滤方式；`filtered` 还需要紧跟一个方块谓词参数。
pub(super) fn clone_filter(value: &str) -> Option<&'static str> {
    match value {
        "replace" | "替换" => Some("replace"),
        "masked" | "遮罩" => Some("masked"),
        "filtered" | "过滤" => Some("filtered"),
        _ => None,
    }
}

/// `clone` 的复制模式。
pub(super) fn clone_mode(value: &str) -> Option<CloneMode> {
    match value {
        "normal" | "普通" => Some(CloneMode::Normal),
        "force" | "强制" => Some(CloneMode::Force),
        "move" | "移动" => Some(CloneMode::Move),
        _ => None,
    }
}

/// `place.template` 的旋转值；`180` 既是数字字面量也是原版枚举名。
pub(super) fn template_rotation(value: &str) -> Option<TemplateRotation> {
    match value {
        "none" | "无" => Some(TemplateRotation::None),
        "clockwise_90" | "顺时针90" => Some(TemplateRotation::Clockwise90),
        "180" => Some(TemplateRotation::Clockwise180),
        "counterclockwise_90" | "逆时针90" => Some(TemplateRotation::Counterclockwise90),
        _ => None,
    }
}

/// `place.template` 的镜像值。
pub(super) fn template_mirror(value: &str) -> Option<TemplateMirror> {
    match value {
        "none" | "无" => Some(TemplateMirror::None),
        "left_right" | "左右" => Some(TemplateMirror::LeftRight),
        "front_back" | "前后" => Some(TemplateMirror::FrontBack),
        _ => None,
    }
}

/// `clone` 的跨维度选项。
pub(super) fn clone_dimension(value: &str) -> Option<&'static str> {
    match value {
        "from_dimension" | "起始维度" => Some("from"),
        "to_dimension" | "目标维度" => Some("to"),
        _ => None,
    }
}

/// `strict` 标志：应用在 `set_block`、`fill`、`clone`、`place.template` 上。
pub(super) fn strict_word(value: &str) -> bool {
    matches!(value, "strict" | "严格")
}

pub(super) fn place_method(value: &str) -> Option<&'static str> {
    match value {
        "feature" | "地物" => Some("feature"),
        "jigsaw" | "拼图" => Some("jigsaw"),
        "structure" | "结构" => Some("structure"),
        "template" | "模板" => Some("template"),
        _ => None,
    }
}

pub(super) fn forceload_method(value: &str) -> Option<&'static str> {
    match value {
        "add" | "添加" => Some("add"),
        "remove" | "移除" => Some("remove"),
        "remove_all" | "全部移除" => Some("remove_all"),
        "query" | "查询" => Some("query"),
        _ => None,
    }
}

pub(super) fn time_method(value: &str) -> Option<&'static str> {
    match value {
        "set" | "设置" => Some("set"),
        "add" | "增加" => Some("add"),
        "pause" | "暂停" => Some("pause"),
        "resume" | "恢复" => Some("resume"),
        "rate" | "速率" => Some("rate"),
        "query" | "查询" => Some("query"),
        "query_gametime" | "查询游戏时间" => Some("query_gametime"),
        _ => None,
    }
}

pub(super) fn weather_kind(value: &str) -> Option<WeatherKind> {
    match value {
        "clear" | "晴朗" => Some(WeatherKind::Clear),
        "rain" | "下雨" => Some(WeatherKind::Rain),
        "thunder" | "雷暴" => Some(WeatherKind::Thunder),
        _ => None,
    }
}

pub(super) fn gamerule_method(value: &str) -> Option<&'static str> {
    match value {
        "set" | "设置" => Some("set"),
        "query" | "查询" => Some("query"),
        _ => None,
    }
}

pub(super) fn worldborder_method(value: &str) -> Option<&'static str> {
    match value {
        "add" | "增加" => Some("add"),
        "set" | "设置" => Some("set"),
        "center" | "中心" => Some("center"),
        "damage_amount" | "伤害量" => Some("damage_amount"),
        "damage_buffer" | "伤害缓冲" => Some("damage_buffer"),
        "get" | "获取" => Some("get"),
        "warning_distance" | "警告距离" => Some("warning_distance"),
        "warning_time" | "警告时间" => Some("warning_time"),
        _ => None,
    }
}

pub(super) fn locate_kind(value: &str) -> Option<LocateKind> {
    match value {
        "structure" | "结构" => Some(LocateKind::Structure),
        "biome" | "生物群系" => Some(LocateKind::Biome),
        "poi" | "兴趣点" => Some(LocateKind::Poi),
        _ => None,
    }
}
