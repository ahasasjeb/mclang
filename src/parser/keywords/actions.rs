use crate::ast::XpKind;

pub(crate) fn self_method(value: &str) -> Option<&'static str> {
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
pub(crate) fn scoreboard_method(value: &str) -> Option<&'static str> {
    match value {
        "set" | "设置" => Some("set"),
        "reset" | "重置" => Some("reset"),
        "get" | "取" => Some("get"),
        "enable" | "启用" => Some("enable"),
        "operation" | "运算" => Some("operation"),
        "display" | "显示" => Some("display"),
        _ => None,
    }
}

/// `scoreboard players operation` 的运算名。
pub(crate) fn score_operation(value: &str) -> Option<&'static str> {
    match value {
        "set" | "赋值" => Some("set"),
        "add" | "加上" => Some("add"),
        "subtract" | "减去" => Some("subtract"),
        "multiply" | "乘以" => Some("multiply"),
        "divide" | "除以" => Some("divide"),
        "modulo" | "取余" => Some("modulo"),
        "min" | "最小值" => Some("min"),
        "max" | "最大值" => Some("max"),
        "swap" | "交换" => Some("swap"),
        _ => None,
    }
}

/// `objective` 声明块里的属性名。
pub(crate) fn objective_property(value: &str) -> Option<&'static str> {
    match value {
        "criteria" | "准则" => Some("criteria"),
        "display_name" | "显示名" => Some("display_name"),
        "render_type" | "渲染类型" => Some("render_type"),
        "number_format" | "数字格式" => Some("number_format"),
        "display_slot" | "显示槽" => Some("display_slot"),
        _ => None,
    }
}

/// 目标渲染类型。
pub(crate) fn render_type(value: &str) -> Option<&'static str> {
    match value {
        "integer" | "整数" => Some("integer"),
        "hearts" | "爱心" => Some("hearts"),
        _ => None,
    }
}

/// `item` 命令的方法名。
pub(crate) fn item_method(value: &str) -> Option<&'static str> {
    match value {
        "replace" | "替换" => Some("replace"),
        "fill" | "填充" => Some("fill"),
        "override" | "覆盖" => Some("override"),
        "modify" | "修改" => Some("modify"),
        _ => None,
    }
}

/// 数字格式构造器：`blank`、`fixed(...)`、`styled`。
pub(crate) fn number_format_kind(value: &str) -> Option<&'static str> {
    match value {
        "blank" | "空白" => Some("blank"),
        "fixed" | "固定" => Some("fixed"),
        "styled" | "样式" => Some("styled"),
        _ => None,
    }
}

/// 数据槽声明的来源构造器。
pub(crate) fn data_slot_kind(value: &str) -> Option<&'static str> {
    match value {
        "item_data" | "物品数据" => Some("item_data"),
        "entity_data" | "实体数据" => Some("entity_data"),
        _ => None,
    }
}

pub(crate) fn message_target(value: &str) -> Option<&'static str> {
    match value {
        "all" | "全部" => Some("all"),
        "self" | "自身" => Some("self"),
        "nearest" | "最近" => Some("nearest"),
        "player" | "玩家" => Some("player"),
        _ => None,
    }
}

/// 文本组件的样式属性名。
pub(crate) fn text_style_property(value: &str) -> Option<&'static str> {
    match value {
        "color" | "颜色" => Some("color"),
        "bold" | "粗体" => Some("bold"),
        "italic" | "斜体" => Some("italic"),
        "underlined" | "下划线" => Some("underlined"),
        "strikethrough" | "删除线" => Some("strikethrough"),
        "obfuscated" | "混淆" => Some("obfuscated"),
        "click" | "点击" => Some("click"),
        "hover" | "悬停" => Some("hover"),
        "interpret" | "解释" => Some("interpret"),
        "plain" | "纯文本" => Some("plain"),
        "separator" | "分隔符" => Some("separator"),
        _ => None,
    }
}

/// 点击事件的动作名。
pub(crate) fn click_action(value: &str) -> Option<&'static str> {
    match value {
        "open_url" | "打开链接" => Some("open_url"),
        "run_command" | "运行命令" => Some("run_command"),
        "suggest_command" | "建议命令" => Some("suggest_command"),
        "copy_to_clipboard" | "复制到剪贴板" => Some("copy_to_clipboard"),
        "change_page" | "翻页" => Some("change_page"),
        _ => None,
    }
}

/// `nbt` 组件的来源类型。
pub(crate) fn nbt_source(value: &str) -> Option<&'static str> {
    match value {
        "entity" | "实体" => Some("entity"),
        "block" | "方块" => Some("block"),
        "storage" | "存储" => Some("storage"),
        _ => None,
    }
}

/// `compute` 的上下文来源。
pub(crate) fn compute_source(value: &str) -> Option<&'static str> {
    match value {
        "default" | "默认" => Some("default"),
        "block" | "方块" => Some("block"),
        "entity" | "实体" => Some("entity"),
        _ => None,
    }
}

/// `compute` 的数值类型。
pub(crate) fn compute_kind(value: &str) -> Option<&'static str> {
    match value {
        "float" | "浮点" => Some("float"),
        "integer" | "整数" => Some("integer"),
        _ => None,
    }
}

/// `data` 命令的方法名。
pub(crate) fn data_method(value: &str) -> Option<&'static str> {
    match value {
        "get" | "取" => Some("get"),
        "merge" | "合并" => Some("merge"),
        "remove" | "移除" => Some("remove"),
        "modify" | "修改" => Some("modify"),
        _ => None,
    }
}

pub(crate) fn effect_method(value: &str) -> Option<&'static str> {
    match value {
        "give" | "给予" => Some("give"),
        "give_infinite" | "给予无限" => Some("give_infinite"),
        "clear" | "清除" => Some("clear"),
        _ => None,
    }
}

pub(crate) fn xp_method(value: &str) -> Option<&'static str> {
    match value {
        "add" | "增加" => Some("add"),
        "set" | "设置" => Some("set"),
        "query" | "查询" => Some("query"),
        _ => None,
    }
}

pub(crate) fn xp_kind(value: &str) -> Option<XpKind> {
    match value {
        "points" | "点数" => Some(XpKind::Points),
        "levels" | "等级" => Some(XpKind::Levels),
        _ => None,
    }
}

pub(crate) fn stopwatch_method(value: &str) -> Option<&'static str> {
    match value {
        "create" | "创建" => Some("create"),
        "query" | "查询" => Some("query"),
        "restart" | "重启" => Some("restart"),
        "remove" | "移除" => Some("remove"),
        _ => None,
    }
}

pub(crate) fn boolean_word(value: &str) -> Option<&'static str> {
    match value {
        "true" | "真" => Some("true"),
        "false" | "假" => Some("false"),
        _ => None,
    }
}

pub(crate) fn time_unit(value: &str) -> Option<&'static str> {
    match value {
        "t" | "刻" => Some("t"),
        "s" | "秒" => Some("s"),
        "d" | "天" => Some("d"),
        _ => None,
    }
}

pub(crate) fn text_color(value: &str) -> Option<&'static str> {
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

pub(crate) fn sound_source(value: &str) -> Option<&'static str> {
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
