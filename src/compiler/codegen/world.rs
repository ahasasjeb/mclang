//! 世界与方块命令的下降：坐标与方块状态文本，以及各命令的最终形状。
//!
//! 这些函数都是纯格式化：语义阶段已经保证引用与范围合法。可选的方块状态属性
//! 按声明顺序输出，标签谓词与属性过滤器直接转发为原版参数写法。

use crate::ast::{
    BlockPosition, BlockStateValue, CloneFilter, CloneMode, ColumnPosition, FillMode,
    ForceLoadOperation, GameRuleValue, NbtValue, PositionValue, RotationValue, SetBlockMode,
    TemplateMirror, TemplateRotation, TimeOperation, Vec2Value, Vec3Value, WeatherKind,
    WorldBorderOperation,
};

use super::emit::nbt_text;

/// `place.template` 的完整参数，避免格式化函数接收一长串松散参数。
pub(super) struct PlaceTemplateOptions<'a> {
    pub template: &'a str,
    pub pos: &'a BlockPosition,
    pub rotation: Option<TemplateRotation>,
    pub mirror: Option<TemplateMirror>,
    pub integrity: Option<&'a str>,
    pub seed: Option<i32>,
    pub strict: bool,
}

/// `clone` 的完整参数，选项顺序已在语义阶段固定。
pub(super) struct CloneOptions<'a> {
    pub begin: &'a BlockPosition,
    pub end: &'a BlockPosition,
    pub destination: &'a BlockPosition,
    pub from_dimension: Option<&'a str>,
    pub to_dimension: Option<&'a str>,
    pub filter: &'a CloneFilter,
    pub mode: CloneMode,
    pub strict: bool,
}

/// `<x> <y> <z>` 方块坐标文本。
pub(super) fn position_text(position: &BlockPosition) -> String {
    format!(
        "{} {} {}",
        position.x.text(),
        position.y.text(),
        position.z.text()
    )
}

/// `<x> <y> <z>` 精确坐标文本。
pub(super) fn vec3_text(position: &Vec3Value) -> String {
    format!(
        "{} {} {}",
        position.x.text(),
        position.y.text(),
        position.z.text()
    )
}

/// `<x> <z>` 水平精确坐标文本（`worldborder.center` 等）。
pub(super) fn vec2_text(position: &Vec2Value) -> String {
    format!("{} {}", position.x.text(), position.z.text())
}

/// 朝向文本：`<yaw> <pitch>`（`teleport` 的旋转参数）。
pub(super) fn rotation_text(rotation: &RotationValue) -> String {
    format!("{} {}", rotation.yaw.text(), rotation.pitch.text())
}

/// 位置值文本：方块坐标与精确坐标共用 `<x> <y> <z>` 形状。
pub(super) fn position_value_text(position: &PositionValue) -> String {
    match position {
        PositionValue::Block(position) => position_text(position),
        PositionValue::Exact(position) => vec3_text(position),
    }
}

/// `<x> <z>` 列坐标文本。
pub(super) fn column_text(position: &ColumnPosition) -> String {
    format!("{} {}", position.x.text(), position.z.text())
}

/// `命名空间:方块[属性=值,...]` 文本，也用于 `#标签` 谓词。
pub(super) fn block_state_text(block: &BlockStateValue) -> String {
    if block.properties.is_empty() {
        return block.id.clone();
    }
    let properties = block
        .properties
        .iter()
        .map(|property| format!("{}={}", property.name, property.value))
        .collect::<Vec<_>>()
        .join(",");
    format!("{}[{properties}]", block.id)
}

/// 方块参数：方块状态文本后紧跟方块实体 NBT（`<block>{<nbt>}`）。
pub(super) fn block_argument_text(block: &BlockStateValue, nbt: Option<&NbtValue>) -> String {
    match nbt {
        Some(nbt) => format!("{}{}", block_state_text(block), nbt_text(nbt)),
        None => block_state_text(block),
    }
}

pub(super) fn set_block_command(
    pos: &BlockPosition,
    block: &BlockStateValue,
    mode: SetBlockMode,
    nbt: Option<&NbtValue>,
) -> String {
    let block = block_argument_text(block, nbt);
    match mode.as_str() {
        Some(mode) => format!("setblock {} {block} {mode}", position_text(pos)),
        None => format!("setblock {} {block}", position_text(pos)),
    }
}

pub(super) fn fill_command(
    from: &BlockPosition,
    to: &BlockPosition,
    block: &BlockStateValue,
    mode: FillMode,
    filter: Option<&BlockStateValue>,
    nbt: Option<&NbtValue>,
) -> String {
    let mut command = format!(
        "fill {} {} {}",
        position_text(from),
        position_text(to),
        block_argument_text(block, nbt)
    );
    if let Some(filter) = filter {
        command.push_str(&format!(" replace {}", block_state_text(filter)));
        return command;
    }
    if let Some(mode) = mode.as_str() {
        command.push_str(&format!(" {mode}"));
    }
    command
}

pub(super) fn fill_biome_command(
    from: &BlockPosition,
    to: &BlockPosition,
    biome: &str,
    filter: Option<&str>,
) -> String {
    let mut command = format!(
        "fillbiome {} {} {biome}",
        position_text(from),
        position_text(to)
    );
    if let Some(filter) = filter {
        command.push_str(&format!(" replace {filter}"));
    }
    command
}

pub(super) fn clone_command(options: &CloneOptions<'_>) -> String {
    let mut command = String::from("clone");
    if let Some(dimension) = options.from_dimension {
        command.push_str(&format!(" from {dimension}"));
    }
    command.push_str(&format!(
        " {} {} {}",
        position_text(options.begin),
        position_text(options.end),
        position_text(options.destination)
    ));
    if let Some(dimension) = options.to_dimension {
        command.push_str(&format!(" to {dimension}"));
    }
    match options.filter {
        CloneFilter::Replace => {}
        CloneFilter::Masked => command.push_str(" masked"),
        CloneFilter::Filtered(filter) => {
            command.push_str(&format!(" filtered {}", block_state_text(filter)));
        }
    }
    if let Some(mode) = options.mode.as_str() {
        command.push_str(&format!(" {mode}"));
    }
    if options.strict {
        command.push_str(" strict");
    }
    command
}

pub(super) fn place_feature_command(feature: &str, pos: Option<&BlockPosition>) -> String {
    match pos {
        Some(pos) => format!("place feature {feature} {}", position_text(pos)),
        None => format!("place feature {feature}"),
    }
}

pub(super) fn place_structure_command(structure: &str, pos: Option<&BlockPosition>) -> String {
    match pos {
        Some(pos) => format!("place structure {structure} {}", position_text(pos)),
        None => format!("place structure {structure}"),
    }
}

pub(super) fn place_jigsaw_command(
    pool: &str,
    target: &str,
    max_depth: u32,
    pos: Option<&BlockPosition>,
) -> String {
    match pos {
        Some(pos) => format!(
            "place jigsaw {pool} {target} {max_depth} {}",
            position_text(pos)
        ),
        None => format!("place jigsaw {pool} {target} {max_depth}"),
    }
}

pub(super) fn place_template_command(options: &PlaceTemplateOptions<'_>) -> String {
    let mut command = format!(
        "place template {} {}",
        options.template,
        position_text(options.pos)
    );
    if let Some(rotation) = options.rotation {
        command.push_str(&format!(" {}", rotation.as_str()));
        if let Some(mirror) = options.mirror {
            command.push_str(&format!(" {}", mirror.as_str()));
            if let Some(integrity) = options.integrity {
                command.push_str(&format!(" {integrity}"));
                if let Some(seed) = options.seed {
                    command.push_str(&format!(" {seed}"));
                }
            }
        }
    }
    if options.strict {
        command.push_str(" strict");
    }
    command
}

pub(super) fn forceload_command(operation: &ForceLoadOperation) -> String {
    match operation {
        ForceLoadOperation::Add { from, to } => match to {
            Some(to) => format!("forceload add {} {}", column_text(from), column_text(to)),
            None => format!("forceload add {}", column_text(from)),
        },
        ForceLoadOperation::Remove { from, to } => match to {
            Some(to) => format!("forceload remove {} {}", column_text(from), column_text(to)),
            None => format!("forceload remove {}", column_text(from)),
        },
        ForceLoadOperation::RemoveAll => "forceload remove all".to_owned(),
        ForceLoadOperation::Query { pos } => match pos {
            Some(pos) => format!("forceload query {}", column_text(pos)),
            None => "forceload query".to_owned(),
        },
    }
}

pub(super) fn time_command(operation: &TimeOperation, clock: Option<&str>) -> String {
    let scope = match clock {
        Some(clock) => format!("time of {clock} "),
        None => "time ".to_owned(),
    };
    match operation {
        TimeOperation::Set(value) => format!("{scope}set {value}"),
        TimeOperation::Add(value) => format!("{scope}add {value}"),
        TimeOperation::Pause => format!("{scope}pause"),
        TimeOperation::Resume => format!("{scope}resume"),
        TimeOperation::Rate(rate) => format!("{scope}rate {rate}"),
    }
}

pub(super) fn weather_command(kind: WeatherKind, duration: Option<&str>) -> String {
    match duration {
        Some(duration) => format!("weather {} {duration}", kind.as_str()),
        None => format!("weather {}", kind.as_str()),
    }
}

pub(super) fn gamerule_command(name: &str, value: GameRuleValue) -> String {
    let value = match value {
        GameRuleValue::Bool(true) => "true".to_owned(),
        GameRuleValue::Bool(false) => "false".to_owned(),
        GameRuleValue::Integer(value) => value.to_string(),
    };
    format!("gamerule {name} {value}")
}

pub(super) fn worldborder_command(operation: &WorldBorderOperation) -> String {
    match operation {
        WorldBorderOperation::Add { distance, time } => {
            border_size_command("add", distance, time.as_deref())
        }
        WorldBorderOperation::Set { distance, time } => {
            border_size_command("set", distance, time.as_deref())
        }
        WorldBorderOperation::Center(value) => format!("worldborder center {}", vec2_text(value)),
        WorldBorderOperation::DamageAmount(value) => format!("worldborder damage amount {value}"),
        WorldBorderOperation::DamageBuffer(value) => format!("worldborder damage buffer {value}"),
        WorldBorderOperation::WarningDistance(value) => {
            format!("worldborder warning distance {value}")
        }
        WorldBorderOperation::WarningTime(value) => format!("worldborder warning time {value}"),
    }
}

fn border_size_command(operation: &str, distance: &str, time: Option<&str>) -> String {
    match time {
        Some(time) => format!("worldborder {operation} {distance} {time}"),
        None => format!("worldborder {operation} {distance}"),
    }
}

pub(super) fn locate_command(kind: &str, target: &str) -> String {
    format!("locate {kind} {target}")
}
