//! 语言服务器的会话状态：文档同步、项目分析与请求分派。
//!
//! 分析范围是工作区里的全部 `.mcl` 文件加上已打开但不在工作区内的文档；每次改动都
//! 重新分析整个项目，因为命名空间、跨文件引用和函数标签本来就是项目级概念。项目
//! 规模很小，全量分析换来的是与命令行完全一致的诊断。

use std::collections::BTreeMap;
use std::io::{self, BufReader, BufWriter};
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use crate::analysis::{FileDiagnostic, ProjectAnalysis, SourceFile};

use super::convert::{offset_to_position, position_to_offset, uri_to_path};
use super::rpc;

/// 单个工作区最多分析的文件数，避免误把大目录当成项目。
const MAX_PROJECT_FILES: usize = 512;

/// 启动标准输入输出上的语言服务器，直到客户端发送 `exit`。
pub fn serve() -> io::Result<()> {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut reader = BufReader::new(stdin.lock());
    let mut writer = BufWriter::new(stdout.lock());
    let mut session = Session::default();
    while let Some(message) = rpc::read_message(&mut reader)? {
        let (response, notifications) = session.handle(&message);
        for notification in notifications {
            rpc::write_message(&mut writer, &notification)?;
        }
        if let Some(response) = response {
            rpc::write_message(&mut writer, &response)?;
        }
        if session.exiting {
            break;
        }
    }
    Ok(())
}

/// 一个 Mclang 项目：同一命名空间、可跨文件引用的一组源文件。
struct Project {
    sources: Vec<SourceFile>,
    analysis: ProjectAnalysis,
}

#[derive(Default)]
pub struct Session {
    roots: Vec<PathBuf>,
    open: BTreeMap<PathBuf, String>,
    projects: Vec<Project>,
    /// 每个工作区根目录下已发现的 `.mcl` 文件；避免每次按键都递归扫描大目录。
    discovered: BTreeMap<PathBuf, Vec<PathBuf>>,
    discovery_dirty: bool,
    /// 每个文件的命名空间，用于推断打开文档属于哪个项目。
    namespaces: BTreeMap<PathBuf, Option<String>>,
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

fn document_offset(text: &str, params: &Value) -> usize {
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
    position_to_offset(text, line, character)
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

fn diagnostic_json(text: &str, diagnostic: &FileDiagnostic) -> Value {
    let (start_line, start_character) = offset_to_position(text, diagnostic.span.start);
    let (end_line, end_character) = offset_to_position(text, diagnostic.span.end);
    json!({
        "range": {
            "start": {"line": start_line, "character": start_character},
            "end": {"line": end_line, "character": end_character},
        },
        "severity": 1,
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
