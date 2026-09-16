//! 版本数据：从仓库内的 Minecraft 源码提取可复现的静态表。
//!
//! - [`generate`]：生成器（`cargo xtask generate-version-data`）；
//! - [`snapshot`]：编译期加载的快照查询接口；
//! - [`entity_nbt`]：实体 NBT 标签表。
//!
//! 命令树快照（1.1）随后并入本模块。

pub mod entity_nbt;
pub mod generate;
pub mod snapshot;
