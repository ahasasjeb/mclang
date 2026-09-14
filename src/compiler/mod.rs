//! 编译器：整程序语义检查与 Minecraft 数据包代码生成。
//!
//! 编译分成两个互不重叠的阶段：
//!
//! 1. [`validate`] 只读地检查整程序，要么返回全部诊断，要么返回空列表；
//! 2. [`codegen`] 只处理已经通过检查的程序，不重复报告错误。
//!
//! 两个阶段共享本模块下的 [`types`] 和 [`constant`]，其余实现细节分别封装在
//! `validate` 与 `codegen` 子模块中。

mod codegen;
mod constant;
mod types;
mod validate;

#[cfg(test)]
mod tests;

use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::ast::Program;
use crate::diagnostic::Diagnostic;

/// 一次成功编译的产物：数据包内的相对路径到文件内容。
#[derive(Debug)]
pub struct CompiledPack {
    pub files: BTreeMap<PathBuf, String>,
}

/// 编译已经合并的整程序。
///
/// 语义检查失败时返回全部诊断；通过后生成数据包文件。`description` 会写入
/// `pack.mcmeta`。
pub fn compile(program: &Program, description: &str) -> Result<CompiledPack, Vec<Diagnostic>> {
    let diagnostics = validate::validate(program);
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }

    let mut compiler = codegen::Compiler::new(program);
    compiler.compile_functions();
    Ok(compiler.finish(description))
}
