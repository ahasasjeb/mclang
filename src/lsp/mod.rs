//! 内置语言服务器：`mclang lsp` 通过 stdio 提供即时诊断、补全、悬停与跳转。
//!
//! 协议层只有 [`rpc`]（`Content-Length` 分帧），特性实现集中在 [`features`]，
//! 项目状态与请求分派在 [`server`]；[`convert`] 负责 LSP 位置与编译器字节范围的换算。

mod convert;
mod features;
mod rpc;
mod server;

pub use server::serve;

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use crate::parser::keywords::{ATTRIBUTES, KEYWORDS};

    /// 高亮语法与关键词表必须同步：新增关键词却忘了改编辑器插件时这里会失败。
    #[test]
    fn grammar_covers_every_keyword() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("editors/vscode/syntaxes/mclang.tmLanguage.json");
        let text = fs::read_to_string(&path).unwrap();
        let json: serde_json::Value =
            serde_json::from_str(&text).expect("语法高亮文件必须是合法 JSON");
        assert_eq!(json["scopeName"], "source.mclang");
        for keyword in KEYWORDS.iter().chain(ATTRIBUTES.iter()) {
            for word in [keyword.english, keyword.chinese] {
                assert!(
                    contains_word(&text, word),
                    "语法高亮里找不到 `{word}`，请在 editors/vscode/syntaxes/mclang.tmLanguage.json 里补上"
                );
            }
        }
    }

    fn contains_word(text: &str, word: &str) -> bool {
        let mut search = 0;
        while let Some(index) = text[search..].find(word) {
            let start = search + index;
            let end = start + word.len();
            let before = text[..start].chars().next_back();
            let after = text[end..].chars().next();
            let free = |character: Option<char>| {
                character.is_none_or(|character| !character.is_alphanumeric() && character != '_')
            };
            if free(before) && free(after) {
                return true;
            }
            search = end;
        }
        false
    }
}
