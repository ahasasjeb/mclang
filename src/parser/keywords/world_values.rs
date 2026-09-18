use crate::ast::{
    Anchor, BossBarField, CloneMode, EntityRelation, FillMode, LocateKind, SetBlockMode,
    StoreDataMode, StoreDataType, TemplateMirror, TemplateRotation, WeatherKind,
};

/// `setblock` 的放置模式。
pub(crate) fn set_block_mode(value: &str) -> Option<SetBlockMode> {
    match value {
        "destroy" | "摧毁" => Some(SetBlockMode::Destroy),
        "keep" | "保留" => Some(SetBlockMode::Keep),
        "replace" | "替换" => Some(SetBlockMode::Replace),
        "strict" | "严格" => Some(SetBlockMode::Strict),
        _ => None,
    }
}

/// `fill` 的填充模式。
pub(crate) fn fill_mode(value: &str) -> Option<FillMode> {
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
pub(crate) fn clone_filter(value: &str) -> Option<&'static str> {
    match value {
        "replace" | "替换" => Some("replace"),
        "masked" | "遮罩" => Some("masked"),
        "filtered" | "过滤" => Some("filtered"),
        _ => None,
    }
}

/// `clone` 的复制模式。
pub(crate) fn clone_mode(value: &str) -> Option<CloneMode> {
    match value {
        "normal" | "普通" => Some(CloneMode::Normal),
        "force" | "强制" => Some(CloneMode::Force),
        "move" | "移动" => Some(CloneMode::Move),
        _ => None,
    }
}

/// `place.template` 的旋转值；`180` 既是数字字面量也是原版枚举名。
pub(crate) fn template_rotation(value: &str) -> Option<TemplateRotation> {
    match value {
        "none" | "无" => Some(TemplateRotation::None),
        "clockwise_90" | "顺时针90" => Some(TemplateRotation::Clockwise90),
        "180" => Some(TemplateRotation::Clockwise180),
        "counterclockwise_90" | "逆时针90" => Some(TemplateRotation::Counterclockwise90),
        _ => None,
    }
}

/// `place.template` 的镜像值。
pub(crate) fn template_mirror(value: &str) -> Option<TemplateMirror> {
    match value {
        "none" | "无" => Some(TemplateMirror::None),
        "left_right" | "左右" => Some(TemplateMirror::LeftRight),
        "front_back" | "前后" => Some(TemplateMirror::FrontBack),
        _ => None,
    }
}

/// `clone` 的跨维度选项。
pub(crate) fn clone_dimension(value: &str) -> Option<&'static str> {
    match value {
        "from_dimension" | "起始维度" => Some("from"),
        "to_dimension" | "目标维度" => Some("to"),
        _ => None,
    }
}

/// `strict` 标志：应用在 `set_block`、`fill`、`clone`、`place.template` 上。
pub(crate) fn strict_word(value: &str) -> bool {
    matches!(value, "strict" | "严格")
}

pub(crate) fn place_method(value: &str) -> Option<&'static str> {
    match value {
        "feature" | "地物" => Some("feature"),
        "jigsaw" | "拼图" => Some("jigsaw"),
        "structure" | "结构" => Some("structure"),
        "template" | "模板" => Some("template"),
        _ => None,
    }
}

pub(crate) fn forceload_method(value: &str) -> Option<&'static str> {
    match value {
        "add" | "添加" => Some("add"),
        "remove" | "移除" => Some("remove"),
        "remove_all" | "全部移除" => Some("remove_all"),
        "query" | "查询" => Some("query"),
        _ => None,
    }
}

pub(crate) fn time_method(value: &str) -> Option<&'static str> {
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

pub(crate) fn weather_kind(value: &str) -> Option<WeatherKind> {
    match value {
        "clear" | "晴朗" => Some(WeatherKind::Clear),
        "rain" | "下雨" => Some(WeatherKind::Rain),
        "thunder" | "雷暴" => Some(WeatherKind::Thunder),
        _ => None,
    }
}

pub(crate) fn gamerule_method(value: &str) -> Option<&'static str> {
    match value {
        "set" | "设置" => Some("set"),
        "query" | "查询" => Some("query"),
        _ => None,
    }
}

pub(crate) fn worldborder_method(value: &str) -> Option<&'static str> {
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

pub(crate) fn locate_kind(value: &str) -> Option<LocateKind> {
    match value {
        "structure" | "结构" => Some(LocateKind::Structure),
        "biome" | "生物群系" => Some(LocateKind::Biome),
        "poi" | "兴趣点" => Some(LocateKind::Poi),
        _ => None,
    }
}

/// 结构化 `execute` 的修饰符子句名，对应原版 `ExecuteCommand.register` 的分支。
pub(crate) fn execute_clause(value: &str) -> Option<&'static str> {
    match value {
        "as" | "作为" => Some("as"),
        "at" | "在" => Some("at"),
        "positioned" | "定位于" => Some("positioned"),
        "rotated" | "转向" => Some("rotated"),
        "facing" | "面向" => Some("facing"),
        "align" | "对齐" => Some("align"),
        "anchored" | "锚点" => Some("anchored"),
        "in" | "进入维度" => Some("in"),
        "on" | "关系" => Some("on"),
        "summon" | "召唤实体" => Some("summon"),
        _ => None,
    }
}

/// `execute on` 的实体关系名。
pub(crate) fn entity_relation(value: &str) -> Option<EntityRelation> {
    match value {
        "owner" | "主人" => Some(EntityRelation::Owner),
        "leasher" | "拴绳者" => Some(EntityRelation::Leasher),
        "target" | "攻击目标" => Some(EntityRelation::Target),
        "attacker" | "攻击者" => Some(EntityRelation::Attacker),
        "vehicle" | "载具" => Some(EntityRelation::Vehicle),
        "controller" | "控制者" => Some(EntityRelation::Controller),
        "origin" | "起源" => Some(EntityRelation::Origin),
        "passengers" | "乘客" => Some(EntityRelation::Passengers),
        _ => None,
    }
}

/// 实体锚点：`facing entity` 与 `anchored` 共用。
pub(crate) fn anchor_value(value: &str) -> Option<Anchor> {
    match value {
        "eyes" | "眼睛" => Some(Anchor::Eyes),
        "feet" | "脚" => Some(Anchor::Feet),
        _ => None,
    }
}

/// `execute store` 的方法名；`store.data` 缺省写入 result。
pub(crate) fn store_method(value: &str) -> Option<&'static str> {
    match value {
        "result" | "结果" => Some("result"),
        "success" | "成功" => Some("success"),
        "data" | "数据" => Some("data"),
        _ => None,
    }
}

/// `store.data` 的写入模式；与方法名共用英文拼写，但只接受 result 与 success。
pub(crate) fn store_data_mode(value: &str) -> Option<StoreDataMode> {
    match value {
        "result" | "结果" => Some(StoreDataMode::Result),
        "success" | "成功" => Some(StoreDataMode::Success),
        _ => None,
    }
}

/// Boss 栏的可写字段：当前值与上限。
pub(crate) fn bossbar_field(value: &str) -> Option<BossBarField> {
    match value {
        "value" | "值" => Some(BossBarField::Value),
        "max" | "上限" => Some(BossBarField::Max),
        _ => None,
    }
}

/// `store.data` 的数值类型。
pub(crate) fn store_data_type(value: &str) -> Option<StoreDataType> {
    match value {
        "byte" | "字节" => Some(StoreDataType::Byte),
        "short" | "短整数" => Some(StoreDataType::Short),
        "int" | "整数" => Some(StoreDataType::Int),
        "long" | "长整数" => Some(StoreDataType::Long),
        "float" | "浮点" => Some(StoreDataType::Float),
        "double" | "双精度" => Some(StoreDataType::Double),
        _ => None,
    }
}
