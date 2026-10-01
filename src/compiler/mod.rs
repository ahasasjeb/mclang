//! 编译器：整程序语义检查与 Minecraft 数据包代码生成。
//!
//! 编译分成两个互不重叠的阶段：
//!
//! 1. [`validate`] 只读地检查整程序，收集错误与非阻断警告；
//! 2. [`codegen`] 只处理已经通过检查的程序，不重复报告错误。
//!
//! 两个阶段共享本模块下的 [`types`]，常量折叠则来自 crate 级的
//! [`crate::constant`]，其余实现细节分别封装在 `validate` 与 `codegen`
//! 子模块中。

mod codegen;
mod rename;
mod types;
mod validate;

pub(crate) use validate::rules::{valid_resource_path, valid_user_name, windows_reserved_name};

use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::ast::Program;
use crate::diagnostic::{Diagnostic, DiagnosticSeverity};

/// FNV-1a：跨平台稳定的 64 位哈希，用于可复现的生成名称。
pub(crate) fn stable_hash(value: &str) -> u64 {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in value.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

/// 一次成功编译的产物：数据包内的相对路径到文件内容。
#[derive(Debug)]
pub struct CompiledPack {
    pub files: BTreeMap<PathBuf, String>,
    pub binary_files: BTreeMap<PathBuf, Vec<u8>>,
}

/// 成功编译的数据包与仍需向用户展示的警告。
pub struct Compilation {
    pub pack: CompiledPack,
    pub warnings: Vec<Diagnostic>,
}

/// 数据包函数的权限上限：26.3 的 `function-permission-level` 默认
/// `GAMEMASTER`（等级 2）。函数在编译期与运行期都受此限制，`ADMIN`(3)/
/// `OWNER`(4) 命令不能用在数据包函数里。
pub(crate) const FUNCTION_PERMISSION_LEVEL: u8 = 2;

/// 编译选项。
pub struct CompileOptions {
    /// 写入 `pack.mcmeta` 的描述。
    pub description: String,
}

/// 编译已经合并的整程序。
///
/// 语义检查失败时返回全部诊断；通过后先把非 ASCII 标识符内部化为随机 ASCII
/// 别名（见 [`rename`]），再生成数据包文件。`description` 会写入 `pack.mcmeta`。
pub fn compile(
    program: &mut Program,
    options: &CompileOptions,
) -> Result<Compilation, Vec<Diagnostic>> {
    crate::stack::run(|| compile_inner(program, options)).unwrap_or_else(|error| {
        Err(vec![Diagnostic::new(
            format!("无法创建编译工作线程：{error}"),
            crate::ast::Span::default(),
        )])
    })
}

fn compile_inner(
    program: &mut Program,
    options: &CompileOptions,
) -> Result<Compilation, Vec<Diagnostic>> {
    let validated = prepare_program(program)?;
    let diagnostics = validated.diagnostics;
    let mut compiler =
        codegen::Compiler::new(program, &validated.resource_json, &validated.style_json);
    compiler.compile_functions();
    let pack = match compiler.finish(&options.description) {
        Ok(pack) => pack,
        Err(mut errors) => {
            errors.extend(diagnostics);
            return Err(errors);
        }
    };
    Ok(Compilation {
        pack,
        warnings: diagnostics,
    })
}

/// 编辑器复用生成命令的校验，但不构造函数文件、资源文件与包元数据。
pub(crate) fn check(program: &mut Program) -> Vec<Diagnostic> {
    crate::stack::run(|| {
        let validated = match prepare_program(program) {
            Ok(validated) => validated,
            Err(diagnostics) => return diagnostics,
        };
        let mut compiler =
            codegen::Compiler::new(program, &validated.resource_json, &validated.style_json);
        compiler.compile_functions();
        let mut diagnostics = compiler.check_commands();
        diagnostics.extend(validated.diagnostics);
        diagnostics
    })
    .unwrap_or_else(|error| {
        vec![Diagnostic::new(
            format!("无法创建编译工作线程：{error}"),
            crate::ast::Span::default(),
        )]
    })
}

fn prepare_program(program: &mut Program) -> Result<validate::Validated, Vec<Diagnostic>> {
    let validated = validate::validate(program);
    let diagnostics = &validated.diagnostics;
    if diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error)
    {
        return Err(validated.diagnostics);
    }

    rename::rename_program(program);

    Ok(validated)
}
