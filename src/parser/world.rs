//! 世界与方块语句：坐标、方块状态值，以及 `set_block`/`fill`/`clone`/`place`、
//! `forceload`、`time`、`weather`、`gamerule`、`worldborder`、`locate` 的参数解析。
//!
//! 位置与方块状态是这些命令共享的具名参数（`pos(...)`、`block_state(...)`），
//! 与查询、物品声明一样在解析期规范化；绝对坐标的范围和资源位置在语义阶段检查。

use super::Parser;
use crate::ast::*;

/// `vec2_component` 的文本转成坐标分量：`~` 前缀是相对坐标。
fn component_coordinate(text: String) -> Coordinate {
    if text.starts_with('~') {
        Coordinate::Relative(text)
    } else {
        Coordinate::Absolute(text)
    }
}

/// `place.template` 的可选后缀，按原版顺序收集。
struct TemplateOptions {
    rotation: Option<TemplateRotation>,
    mirror: Option<TemplateMirror>,
    integrity: Option<String>,
    seed: Option<i32>,
    strict: bool,
}

mod blocks;
mod ops;
mod positions;
mod states;
