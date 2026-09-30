//! 中英关键词互译：把 `.mcl` 源码改写成另一种关键词写法。
//!
//! 只翻译语言词汇——关键词、函数属性、方法与枚举值；字符串、注释、数字、
//! 用户标识符与源码格式原样保留。两种写法语义完全等价，因此翻译结果编译出的
//! 数据包与原文逐字节一致。
//!
//! 改写规则沿用文档工具的 `docs/tools/translate.mjs`（它负责手册的双语示例），
//! 词表则不同：这里不解析 Rust 源码，而是探测 `src/parser/keywords/` 的规范化
//! 函数，见 [`tables`]。此外，翻译前先用真实的词法与语法分析收集**用户声明的
//! 标识符**：`item reward = …` 的物品名、查询名 `players`、参数与局部变量不会
//! 因为拼写撞上方法/属性/枚举表而被改写。明确的方法、块成员和枚举位置则按语法
//! 翻译，不会被另一处同名声明挡住；不完整源码也通过浅层扫描保护已写出的声明。
//!
//! ```
//! use mclang::{KeywordLanguage, translate};
//! let source = "score ticks = 0;\n";
//! let chinese = translate(source, KeywordLanguage::Chinese).unwrap();
//! assert_eq!(chinese, "计分 ticks = 0;\n");
//! ```

mod context;
mod rewrite;
mod syntax;
mod tables;
mod token;

use std::collections::HashSet;

use tables::tables;

/// 翻译的目标写法。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KeywordLanguage {
    English,
    Chinese,
}

impl KeywordLanguage {
    /// 解析命令行写法：`en`/`english` 或 `zh`/`chinese`。
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "en" | "english" => Some(Self::English),
            "zh" | "chinese" => Some(Self::Chinese),
            _ => None,
        }
    }
}

/// 把源码中的语言词汇改写成目标语言。
///
/// 返回 `Err` 只可能来自编译器自身的词表问题（例如关键词表出现歧义），
/// 与输入源码无关。已经使用目标规范写法的词保持原样；英文兼容拼写会归一化，
/// 例如命令中的 `displayname` 归一为 `display_name`，使双向往返稳定。
pub fn translate(source: &str, language: KeywordLanguage) -> Result<String, String> {
    let declared = declared_names(source);
    translate_declared(source, language, &declared)
}

/// 与 [`translate`] 相同，但复用调用方收集的已声明标识符：项目翻译会把全部
/// 模块的声明合并起来，让跨模块引用也受到保护。
pub(crate) fn translate_declared(
    source: &str,
    language: KeywordLanguage,
    declared: &HashSet<String>,
) -> Result<String, String> {
    let tables = tables()?;
    Ok(rewrite::translate(source, tables, language, declared))
}

/// 收集源码里用户声明的全部标识符（计分、查询、物品、函数、参数、局部变量等）。
///
/// 优先使用完整语法树；源码未写完或包含语法错误时，浅层扫描仍保护声明与参数。
pub(crate) fn declared_names(source: &str) -> HashSet<String> {
    let scanned = token::tokenize(source);
    let mut names = syntax::declared_names(&scanned, &token::Neighbors::new(&scanned));
    let Ok(tokens) = crate::lexer::lex(source, 0) else {
        return names;
    };
    let Ok(mut program) = crate::parser::parse(tokens) else {
        return names;
    };
    program.for_each_name_mut(&mut |_context, site, _role, name| {
        if site == crate::name_walk::NameSite::Declaration {
            names.insert(name.clone());
        }
    });
    names
}
