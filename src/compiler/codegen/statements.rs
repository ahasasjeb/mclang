//! 控制流下降：语句分派、条件分支、循环、调用和辅助函数分配。
//!
//! `compile_block` 只做分发；需要独立执行上下文的结构块
//! （`if`/`while`/`each`/`spawn`/`in_dimension`/`execute`）通过分配辅助函数实现。
//! `give` 与 `self` 实体操作在 [`super::actions`]。

mod calls;
mod compile;
mod control;
mod execute;
pub(super) mod helpers;
mod optimization;
