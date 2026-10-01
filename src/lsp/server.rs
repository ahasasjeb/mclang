//! 语言服务器的会话状态：文档同步、项目分析与请求分派。
//!
//! Unchanged projects reuse their analysis. Changed projects still resolve and
//! compile together so cross-file diagnostics stay consistent with the CLI.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde_json::{Value, json};

use crate::analysis::{FileDiagnostic, ProjectAnalysis, SourceFile};
use crate::lines::LineIndex;

use super::convert::{offset_to_position_in, position_to_offset_in, uri_to_path};
pub use transport::serve;

/// 单个工作区最多分析的文件数，避免误把大目录当成项目。
const MAX_PROJECT_FILES: usize = 512;

/// 一个 Mclang 项目：同一命名空间、可跨文件引用的一组源文件。
struct Project {
    key: (PathBuf, Option<String>),
    sources: Vec<SourceFile>,
    analysis: ProjectAnalysis,
    indexes: BTreeMap<PathBuf, LineIndex>,
}

/// 由发现到的源文件构建的项目查找索引；文件发现失效前可跨刷新复用。
struct DiscoveryIndex {
    files: Vec<PathBuf>,
    direct_files: HashMap<PathBuf, Vec<usize>>,
    subtree_files: HashMap<PathBuf, Vec<usize>>,
}

/// 磁盘文件的缓存内容，附带用于判断是否变化的时间戳。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct DiskStamp {
    /// 文件长度。
    pub(super) len: u64,
    /// 最后修改时间；文件系统不提供时为 `None`。
    pub(super) modified: Option<(u64, u32)>,
}

#[derive(Default)]
pub struct Session {
    roots: Vec<PathBuf>,
    open: BTreeMap<PathBuf, String>,
    projects: Vec<Project>,
    /// 每个工作区根目录下已发现的 `.mcl` 文件；避免每次按键都递归扫描大目录。
    discovered: BTreeMap<PathBuf, Vec<PathBuf>>,
    discovery_dirty: bool,
    discovery_index: Option<Arc<DiscoveryIndex>>,
    /// 每个文件的命名空间，用于推断打开文档属于哪个项目。
    namespaces: BTreeMap<PathBuf, Option<String>>,
    /// 磁盘文件的内容与其修改时间戳；未变化的文件不重复读盘。
    disk_text: BTreeMap<PathBuf, (DiskStamp, String)>,
    /// 上次发布的诊断，用于跳过没有变化的通知。
    published: BTreeMap<PathBuf, Value>,
    shutdown: bool,
    exiting: bool,
}

fn document_uri(params: &Value) -> Option<&str> {
    params.get("textDocument")?.get("uri")?.as_str()
}

/// `didOpen` 的文档路径与内容。
fn opened_document(params: &Value) -> Option<(PathBuf, String)> {
    let document = params.get("textDocument")?;
    let uri = document.get("uri")?.as_str()?;
    let text = document.get("text")?.as_str()?;
    Some((absolute(&uri_to_path(uri)?), text.to_owned()))
}

/// `didChange` 的最新全文（本服务器声明为全量同步）。
fn changed_document(params: &Value) -> Option<(PathBuf, String)> {
    let uri = document_uri(params)?;
    let text = params
        .get("contentChanges")?
        .as_array()?
        .first()?
        .get("text")?
        .as_str()?;
    Some((absolute(&uri_to_path(uri)?), text.to_owned()))
}

/// `didClose` 的文档路径。
fn closed_document(params: &Value) -> Option<PathBuf> {
    Some(absolute(&uri_to_path(document_uri(params)?)?))
}

/// 取出源文件的命名空间声明；语法不完整时返回 `None`，调用方按“未知”处理。
fn extract_namespace(text: &str) -> Option<String> {
    let tokens = crate::lexer::lex(text, 0).ok()?;
    for (index, token) in tokens.iter().enumerate() {
        if let crate::lexer::TokenKind::Ident(word) = &token.kind
            && crate::parser::keywords::word_matches(word, "namespace")
        {
            return match tokens.get(index + 1).map(|token| &token.kind) {
                Some(crate::lexer::TokenKind::Ident(name)) => Some(name.clone()),
                _ => None,
            };
        }
    }
    None
}

fn document_offset(text: &str, params: &Value, index: &LineIndex) -> usize {
    let line = params
        .get("position")
        .and_then(|position| position.get("line"))
        .and_then(Value::as_u64)
        .unwrap_or(0) as u32;
    let character = params
        .get("position")
        .and_then(|position| position.get("character"))
        .and_then(Value::as_u64)
        .unwrap_or(0) as u32;
    position_to_offset_in(text, line, character, index)
}

fn workspace_roots(params: &Value) -> Vec<PathBuf> {
    let mut roots: Vec<PathBuf> = params
        .get("workspaceFolders")
        .and_then(Value::as_array)
        .map(|folders| folders.iter().filter_map(folder_path).collect())
        .unwrap_or_default();
    if roots.is_empty() {
        roots.extend(
            params
                .get("rootUri")
                .and_then(Value::as_str)
                .and_then(uri_to_path)
                .map(|path| absolute(&path)),
        );
    }
    roots
}

fn folder_path(folder: &Value) -> Option<PathBuf> {
    uri_to_path(folder.get("uri")?.as_str()?).map(|path| absolute(&path))
}

fn diagnostic_json(text: &str, index: &LineIndex, diagnostic: &FileDiagnostic) -> Value {
    let (start_line, start_character) = offset_to_position_in(text, diagnostic.span.start, index);
    let (end_line, end_character) = offset_to_position_in(text, diagnostic.span.end, index);
    json!({
        "range": {
            "start": {"line": start_line, "character": start_character},
            "end": {"line": end_line, "character": end_character},
        },
        "severity": match diagnostic.severity {
            crate::diagnostic::DiagnosticSeverity::Error => 1,
            crate::diagnostic::DiagnosticSeverity::Warning => 2,
        },
        "source": "mclang",
        "message": diagnostic.message,
    })
}

fn response(id: &Value, result: Value) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "result": result})
}

fn error_response(id: &Value, code: i64, message: String) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}})
}

/// 统一用绝对路径做键，避免工作区发现与编辑器 URI 在相对路径上不一致。
fn absolute(path: &Path) -> PathBuf {
    std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf())
}

mod dispatch;
mod lifecycle;
mod transport;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lsp::convert::path_to_uri;

    #[test]
    fn document_changes_replace_cached_indexes_used_by_hover_and_definition() {
        let path =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("target/lsp-index-regression/main.mcl");
        let uri = path_to_uri(&path);
        let mut session = Session::default();
        session.handle(&json!({"id": 1, "method": "initialize", "params": {}}));
        let first = "namespace audit;\nscore 变量 = 1;\nfn main() { 变量 += 1; }";
        session.handle(&json!({"method": "textDocument/didOpen", "params": {
            "textDocument": {"uri": uri, "text": first}
        }}));
        let updated =
            "// 🚀 中文注释\n\nnamespace audit;\nscore 变量 = 1;\nfn main() { 变量 += 1; }";
        session.handle(&json!({"method": "textDocument/didChange", "params": {
            "textDocument": {"uri": uri}, "contentChanges": [{"text": updated}]
        }}));
        for (id, method) in [(2, "textDocument/hover"), (3, "textDocument/definition")] {
            let (response, _) = session.handle(&json!({"id": id, "method": method, "params": {
                "textDocument": {"uri": uri}, "position": {"line": 4, "character": 14}
            }}));
            let result = response.unwrap()["result"].clone();
            assert!(!result.is_null(), "{method}: {result}");
            if method.ends_with("hover") {
                assert_eq!(result["range"]["start"]["line"], 4);
                assert!(
                    result["contents"]["value"]
                        .as_str()
                        .unwrap()
                        .contains("main.mcl:4")
                );
            } else {
                assert_eq!(result["range"]["start"]["line"], 3);
                assert_eq!(result["range"]["start"]["character"], 6);
            }
        }
    }
}
