//! 内置语言服务器：`mclang lsp` 通过 stdio 提供即时诊断、补全、悬停与跳转。
//!
//! 协议层只有 [`rpc`]（`Content-Length` 分帧），特性实现集中在 [`features`]，
//! 项目状态与请求分派在 [`server`]；[`convert`] 负责 LSP 位置与编译器字节范围的换算。

mod convert;
mod features;
mod rpc;
mod server;

pub use server::serve;
