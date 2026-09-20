/// 语言关键词的规范英文写法与中文别名。
pub(crate) struct Keyword {
    pub english: &'static str,
    pub chinese: &'static str,
}

/// 全部语言关键词。中英文写法都必须全局唯一，测试 `keywords_are_unique` 保证这一点。
pub(crate) const KEYWORDS: &[Keyword] = &[
    Keyword {
        english: "with",
        chinese: "用",
    },
    Keyword {
        english: "macro",
        chinese: "宏",
    },
    Keyword {
        english: "item_predicate",
        chinese: "物品谓词",
    },
    Keyword {
        english: "loot",
        chinese: "战利品",
    },
    Keyword {
        english: "recipe",
        chinese: "配方",
    },
    Keyword {
        english: "reload",
        chinese: "重载",
    },
    Keyword {
        english: "datapack",
        chinese: "数据包",
    },
    Keyword {
        english: "kill",
        chinese: "杀死",
    },
    Keyword {
        english: "tag",
        chinese: "标签",
    },
    Keyword {
        english: "enchant",
        chinese: "施加附魔",
    },
    Keyword {
        english: "damage",
        chinese: "伤害",
    },
    Keyword {
        english: "attribute",
        chinese: "实体属性",
    },
    Keyword {
        english: "ride",
        chinese: "乘骑",
    },
    Keyword {
        english: "rotate",
        chinese: "旋转",
    },
    Keyword {
        english: "spreadplayers",
        chinese: "分散实体",
    },
    Keyword {
        english: "spectate",
        chinese: "旁观",
    },
    Keyword {
        english: "swing",
        chinese: "挥动",
    },
    Keyword {
        english: "trigger",
        chinese: "触发",
    },
    Keyword {
        english: "gamemode",
        chinese: "游戏模式",
    },
    Keyword {
        english: "defaultgamemode",
        chinese: "默认游戏模式",
    },
    Keyword {
        english: "difficulty",
        chinese: "难度",
    },
    Keyword {
        english: "spawnpoint",
        chinese: "设置出生点",
    },
    Keyword {
        english: "setworldspawn",
        chinese: "设置世界出生点",
    },
    Keyword {
        english: "team",
        chinese: "队伍",
    },
    Keyword {
        english: "waypoint",
        chinese: "路径点",
    },
    Keyword {
        english: "list",
        chinese: "在线玩家",
    },
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
        english: "unless",
        chinese: "除非",
    },
    Keyword {
        english: "store",
        chinese: "存值",
    },
    Keyword {
        english: "bossbar",
        chinese: "Boss栏",
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
        english: "for",
        chinese: "对于",
    },
    Keyword {
        english: "in",
        chinese: "在",
    },
    Keyword {
        english: "break",
        chinese: "跳出",
    },
    Keyword {
        english: "continue",
        chinese: "跳过",
    },
    Keyword {
        english: "import",
        chinese: "导入",
    },
    Keyword {
        english: "export",
        chinese: "公开",
    },
    Keyword {
        english: "as",
        chinese: "作为",
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
        chinese: "执行",
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
        english: "block_pos",
        chinese: "方块坐标",
    },
    Keyword {
        english: "vec3",
        chinese: "精确坐标",
    },
    Keyword {
        english: "vec2",
        chinese: "平面坐标",
    },
    Keyword {
        english: "rotation",
        chinese: "朝向",
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
    Keyword {
        english: "text",
        chinese: "文本",
    },
    Keyword {
        english: "translate",
        chinese: "翻译",
    },
    Keyword {
        english: "keybind",
        chinese: "按键",
    },
    Keyword {
        english: "selector",
        chinese: "选择器",
    },
    Keyword {
        english: "count",
        chinese: "计数",
    },
    Keyword {
        english: "random",
        chinese: "随机",
    },
    Keyword {
        english: "compute",
        chinese: "计算",
    },
    Keyword {
        english: "data",
        chinese: "数据操作",
    },
    Keyword {
        english: "block",
        chinese: "方块",
    },
    Keyword {
        english: "blocks",
        chinese: "区域方块",
    },
    Keyword {
        english: "biome",
        chinese: "生物群系",
    },
    Keyword {
        english: "loaded",
        chinese: "已加载",
    },
    Keyword {
        english: "dimension",
        chinese: "维度",
    },
    Keyword {
        english: "items",
        chinese: "物品条件",
    },
    Keyword {
        english: "slots",
        chinese: "槽位条件",
    },
    Keyword {
        english: "function",
        chinese: "函数条件",
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
