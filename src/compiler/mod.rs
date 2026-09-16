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

use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::ast::Program;
use crate::diagnostic::Diagnostic;

/// 一次成功编译的产物：数据包内的相对路径到文件内容。
#[derive(Debug)]
pub struct CompiledPack {
    pub files: BTreeMap<PathBuf, String>,
}

/// 数据包函数的默认权限等级：26.3 的 `function-permission-level` 默认
/// `GAMEMASTER`（等级 2）。函数在编译期与运行期都受此限制，`ADMIN`(3)/
/// `OWNER`(4) 命令只能在显式放开后使用。
pub const DEFAULT_FUNCTION_PERMISSION_LEVEL: u8 = 2;

/// 编译选项。
pub struct CompileOptions {
    /// 写入 `pack.mcmeta` 的描述。
    pub description: String,
    /// 函数可用的最高权限等级（0–4）。
    pub function_permission_level: u8,
}

/// 编译已经合并的整程序。
///
/// 语义检查失败时返回全部诊断；通过后生成数据包文件。`description` 会写入
/// `pack.mcmeta`。
pub fn compile(
    program: &Program,
    options: &CompileOptions,
) -> Result<CompiledPack, Vec<Diagnostic>> {
    let diagnostics = validate::validate(program, options.function_permission_level);
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }

    let mut compiler = codegen::Compiler::new(program);
    compiler.compile_functions();
    Ok(compiler.finish(&options.description))
}
