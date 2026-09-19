//! 关键词、属性和枚举值的中英文规范化表。
//!
//! [`KEYWORDS`] 是语言关键词的唯一来源：解析器的 `word_matches`、保留字检查和
//! 诊断文本都从这里派生，新增关键词只改这一处。枚举值（排序、稀有度、颜色、
//! 声音分类等）按领域保持独立函数，规范形式统一为英文；中文别名只在解析边界
//! 出现，进入 AST 后所有阶段只处理英文规范值。

mod actions;
mod commands;
mod table;
mod values;
mod world_values;

pub(crate) use actions::*;
pub(crate) use commands::*;
pub(crate) use table::*;
pub(crate) use values::*;
pub(crate) use world_values::*;
