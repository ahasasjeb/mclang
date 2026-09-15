//! 语言服务器的会话状态：文档同步、项目分析与请求分派。
//!
//! 分析范围是工作区里的全部 `.mcl` 文件加上已打开但不在工作区内的文档；每次改动都
//! 重新分析整个项目，因为命名空间、跨文件引用和函数标签本来就是项目级概念。项目
//! 规模很小，全量分析换来的是与命令行完全一致的诊断。

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{self, BufReader, BufWriter};
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use crate::analysis::{FileDiagnostic, ProjectAnalysis, SourceFile, analyze};
use crate::discover_sources;

use super::convert::{offset_to_position, path_to_uri, position_to_offset, uri_to_path};
use super::features;
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

impl Session {
    /// 处理一条消息，返回响应（若为请求）和需要发出的通知。
    pub fn handle(&mut self, message: &Value) -> (Option<Value>, Vec<Value>) {
        let Some(method) = message.get("method").and_then(Value::as_str) else {
            // 客户端对服务器请求的响应；本服务器不发起请求，直接忽略。
            return (None, Vec::new());
        };
        let method = method.to_owned();
        let params = message.get("params").cloned().unwrap_or(Value::Null);
        match message.get("id") {
            Some(id) => (Some(self.request(&method, id, &params)), Vec::new()),
            None => (None, self.notification(&method, &params)),
        }
    }

    fn request(&mut self, method: &str, id: &Value, params: &Value) -> Value {
        match method {
            "initialize" => response(id, self.initialize(params)),
            "shutdown" => {
                self.shutdown = true;
                response(id, Value::Null)
            }
            "textDocument/completion" => {
                let Some((path, text, project)) = self.request_document(params) else {
                    return response(id, Value::Null);
                };
                let offset = document_offset(text, params);
                response(
                    id,
                    features::completion(text, offset, &path, &project.analysis.symbols),
                )
            }
            "textDocument/hover" => {
                let Some((path, text, project)) = self.request_document(params) else {
                    return response(id, Value::Null);
                };
                let offset = document_offset(text, params);
                let hover = features::hover(
                    text,
                    offset,
                    &path,
                    &project.analysis.symbols,
                    &project.sources,
                );
                response(id, hover.unwrap_or(Value::Null))
            }
            "textDocument/definition" => {
                let Some((path, text, project)) = self.request_document(params) else {
                    return response(id, Value::Null);
                };
                let offset = document_offset(text, params);
                let definition = features::definition(
                    text,
                    offset,
                    &path,
                    &project.analysis.symbols,
                    &project.sources,
                );
                response(id, definition.unwrap_or(Value::Null))
            }
            _ => error_response(id, -32601, format!("不支持的方法：{method}")),
        }
    }

    fn notification(&mut self, method: &str, params: &Value) -> Vec<Value> {
        match method {
            "initialized" => self.refresh(),
            "textDocument/didOpen" => {
                if let Some((path, text)) = opened_document(params) {
                    // 编辑器里新建的文件不在已发现列表里，需要重新扫描目录。
                    if !self
                        .discovered
                        .values()
                        .flatten()
                        .any(|candidate| candidate == &path)
                    {
                        self.discovery_dirty = true;
                    }
                    self.namespaces.remove(&path);
                    self.open.insert(path, text);
                }
                self.refresh()
            }
            "textDocument/didChange" => {
                if let Some((path, text)) = changed_document(params) {
                    // 命名空间声明可能被改过，失效缓存后重新推断项目边界。
                    self.namespaces.remove(&path);
                    self.open.insert(path, text);
                }
                self.refresh()
            }
            "textDocument/didClose" => {
                if let Some(path) = closed_document(params) {
                    self.open.remove(&path);
                }
                self.refresh()
            }
            "textDocument/didSave" => self.refresh(),
            "workspace/didChangeWorkspaceFolders" => {
                self.change_workspace_folders(params);
                self.refresh()
            }
            "workspace/didChangeConfiguration" => self.refresh(),
            "exit" => {
                self.exiting = true;
                Vec::new()
            }
            _ => Vec::new(),
        }
    }

    fn initialize(&mut self, params: &Value) -> Value {
        self.roots = workspace_roots(params);
        self.discovery_dirty = true;
        json!({
            "capabilities": {
                "positionEncoding": "utf-16",
                "textDocumentSync": {"openClose": true, "change": 1, "save": {"includeText": false}},
                "completionProvider": {"triggerCharacters": ["@", "#"], "resolveProvider": false},
                "hoverProvider": true,
                "definitionProvider": true,
            },
            "serverInfo": {"name": "mclang", "version": env!("CARGO_PKG_VERSION")},
        })
    }

    fn change_workspace_folders(&mut self, params: &Value) {
        let Some(event) = params.get("event") else {
            return;
        };
        if let Some(removed) = event.get("removed").and_then(Value::as_array) {
            let removed: Vec<PathBuf> = removed.iter().filter_map(folder_path).collect();
            self.roots.retain(|root| !removed.contains(root));
        }
        if let Some(added) = event.get("added").and_then(Value::as_array) {
            for path in added.iter().filter_map(folder_path) {
                if !self.roots.contains(&path) {
                    self.roots.push(path);
                }
            }
        }
        self.discovery_dirty = true;
    }

    /// 重新收集项目、逐个分析并发布诊断。
    fn refresh(&mut self) -> Vec<Value> {
        self.projects = self.collect_projects();
        for project in &mut self.projects {
            project.analysis = analyze(&project.sources);
        }
        self.publish_diagnostics()
    }

    /// 收集所有打开文档所属项目的源文件。
    ///
    /// 工作区里可以并存多个互不相关的项目（例如本仓库的 `examples/`）。项目边界由
    /// 打开文档推断：向上走到最远的、目录里没有其他命名空间的父目录；项目内只保留
    /// 同命名空间的文件。每个项目单独分析，符号表与诊断不会互相污染。
    fn collect_projects(&mut self) -> Vec<Project> {
        self.ensure_discovered();
        let discovered = self.discovered_files();
        let open_paths: Vec<PathBuf> = self.open.keys().cloned().collect();

        let mut keys: Vec<(PathBuf, Option<String>)> = Vec::new();
        for path in &open_paths {
            let key = self.project_root(path, &discovered);
            if !keys.contains(&key) {
                keys.push(key);
            }
        }

        let open: Vec<(PathBuf, String)> = self
            .open
            .iter()
            .map(|(path, text)| (path.clone(), text.clone()))
            .collect();
        keys.into_iter()
            .map(|(root, namespace)| {
                let mut texts: BTreeMap<PathBuf, String> = BTreeMap::new();
                for path in discovered
                    .iter()
                    .filter(|path| path.starts_with(&root))
                    .take(MAX_PROJECT_FILES)
                {
                    if !self.namespace_matches(path, &namespace) {
                        continue;
                    }
                    if let Ok(text) = fs::read_to_string(path) {
                        texts.insert(path.clone(), text);
                    }
                }
                // 打开的文档覆盖磁盘内容，未保存的编辑也能得到诊断。
                for (path, text) in &open {
                    if path.starts_with(&root) && self.namespace_matches(path, &namespace) {
                        texts.insert(path.clone(), text.clone());
                    }
                }
                Project {
                    sources: texts
                        .into_iter()
                        .map(|(path, text)| SourceFile { path, text })
                        .collect(),
                    analysis: ProjectAnalysis::default(),
                }
            })
            .collect()
    }

    /// 文件是否属于给定命名空间的项目；命名空间未知（语法不完整）时按属于处理。
    fn namespace_matches(&mut self, path: &Path, namespace: &Option<String>) -> bool {
        match namespace {
            Some(expected) => self
                .namespace_of(path)
                .is_none_or(|actual| &actual == expected),
            None => true,
        }
    }

    /// 请求文档所在的项目。
    fn project_for(&self, path: &Path) -> Option<&Project> {
        self.projects
            .iter()
            .find(|project| project.sources.iter().any(|source| source.path == path))
    }

    fn ensure_discovered(&mut self) {
        for root in &self.roots {
            if self.discovery_dirty || !self.discovered.contains_key(root) {
                let directory = if root.is_file() {
                    root.parent().map_or(root.as_path(), |parent| parent)
                } else {
                    root.as_path()
                };
                let mut paths = Vec::new();
                if discover_sources(directory, &mut paths).is_err() {
                    paths.clear();
                }
                paths.sort();
                paths.truncate(MAX_PROJECT_FILES);
                self.discovered.insert(root.clone(), paths);
            }
        }
        self.discovery_dirty = false;
    }

    fn discovered_files(&self) -> Vec<PathBuf> {
        let mut files: Vec<PathBuf> = self.discovered.values().flatten().cloned().collect();
        files.sort();
        files.dedup();
        files
    }

    /// 打开文档所属项目：项目根目录与命名空间。
    fn project_root(&mut self, file: &Path, discovered: &[PathBuf]) -> (PathBuf, Option<String>) {
        let namespace = self.namespace_of(file);
        let boundary = self.workspace_boundary(file);
        let mut root = file
            .parent()
            .map_or_else(|| file.to_path_buf(), Path::to_path_buf);
        while root != boundary {
            let Some(parent) = root.parent().map(Path::to_path_buf) else {
                break;
            };
            if !parent.starts_with(&boundary)
                || !self.directory_compatible(&parent, namespace.as_deref(), discovered)
            {
                break;
            }
            root = parent;
        }
        (root, namespace)
    }

    /// 文件所属的最内层工作区根；不在任何工作区内时以文件所在目录为界。
    fn workspace_boundary(&self, file: &Path) -> PathBuf {
        self.roots
            .iter()
            .filter(|root| file.starts_with(root))
            .max_by_key(|root| root.components().count())
            .cloned()
            .unwrap_or_else(|| {
                file.parent()
                    .map_or_else(|| file.to_path_buf(), Path::to_path_buf)
            })
    }

    /// 父目录直接放着的 `.mcl` 只要出现不同命名空间，就说明它不是同一个项目。
    fn directory_compatible(
        &mut self,
        directory: &Path,
        namespace: Option<&str>,
        discovered: &[PathBuf],
    ) -> bool {
        let Some(namespace) = namespace else {
            return true;
        };
        discovered
            .iter()
            .filter(|path| path.parent() == Some(directory))
            .all(|path| {
                self.namespace_of(path)
                    .is_none_or(|actual| actual == namespace)
            })
    }

    /// 读取并缓存文件的命名空间；已打开的文档用缓冲区内容。
    fn namespace_of(&mut self, path: &Path) -> Option<String> {
        if let Some(cached) = self.namespaces.get(path) {
            return cached.clone();
        }
        let text = self
            .open
            .get(path)
            .cloned()
            .or_else(|| fs::read_to_string(path).ok());
        let namespace = text.as_deref().and_then(extract_namespace);
        self.namespaces
            .insert(path.to_path_buf(), namespace.clone());
        namespace
    }

    fn publish_diagnostics(&mut self) -> Vec<Value> {
        let mut grouped: BTreeMap<PathBuf, Vec<Value>> = BTreeMap::new();
        for project in &self.projects {
            for diagnostic in &project.analysis.diagnostics {
                let Some(source) = project
                    .sources
                    .iter()
                    .find(|source| source.path == diagnostic.path)
                else {
                    continue;
                };
                grouped
                    .entry(diagnostic.path.clone())
                    .or_default()
                    .push(diagnostic_json(&source.text, diagnostic));
            }
        }

        let mut paths: BTreeSet<PathBuf> = self.published.keys().cloned().collect();
        paths.extend(grouped.keys().cloned());
        let mut notifications = Vec::new();
        for path in paths {
            let diagnostics = grouped.remove(&path).unwrap_or_default();
            let params = json!({"uri": path_to_uri(&path), "diagnostics": diagnostics});
            if self.published.get(&path) == Some(&params) {
                continue;
            }
            self.published.insert(path, params.clone());
            notifications.push(json!({
                "jsonrpc": "2.0",
                "method": "textDocument/publishDiagnostics",
                "params": params,
            }));
        }
        notifications
    }

    /// 请求涉及的文档：路径、最近一次分析的文本与所属项目。
    fn request_document<'a>(&'a self, params: &Value) -> Option<(PathBuf, &'a str, &'a Project)> {
        let path = absolute(&uri_to_path(document_uri(params)?)?);
        let project = self.project_for(&path)?;
        let source = project.sources.iter().find(|source| source.path == path)?;
        Some((path, source.text.as_str(), project))
    }
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

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    fn temp_project() -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("mclang-lsp-{}-{nonce}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        root
    }

    fn initialize(session: &mut Session, root: &Path) -> Vec<Value> {
        let (response, notifications) = session.handle(&json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {"rootUri": path_to_uri(root)},
        }));
        assert!(response.unwrap()["result"]["capabilities"]["completionProvider"].is_object());
        assert!(notifications.is_empty());
        session
            .handle(&json!({"jsonrpc": "2.0", "method": "initialized", "params": {}}))
            .1
    }

    fn open(session: &mut Session, path: &Path, text: &str) -> Vec<Value> {
        session
            .handle(&json!({
                "jsonrpc": "2.0",
                "method": "textDocument/didOpen",
                "params": {"textDocument": {"uri": path_to_uri(path), "text": text}},
            }))
            .1
    }

    #[test]
    fn publishes_diagnostics_and_clears_them_after_a_fix() {
        let root = temp_project();
        let path = root.join("main.mcl");
        fs::write(&path, "namespace demo;\n").unwrap();
        let mut session = Session::default();
        assert!(initialize(&mut session, &root).is_empty());

        let notifications = open(
            &mut session,
            &path,
            "namespace demo;\nfn main() { unknown(); }\n",
        );
        let diagnostics = notifications
            .iter()
            .find(|notification| notification["method"] == "textDocument/publishDiagnostics")
            .expect("应发布诊断");
        assert_eq!(diagnostics["params"]["uri"], path_to_uri(&path));
        assert!(
            !diagnostics["params"]["diagnostics"]
                .as_array()
                .unwrap()
                .is_empty()
        );

        let notifications = session
            .handle(&json!({
                "jsonrpc": "2.0",
                "method": "textDocument/didChange",
                "params": {
                    "textDocument": {"uri": path_to_uri(&path)},
                    "contentChanges": [{"text": "namespace demo;\nfn main() { }\n"}],
                },
            }))
            .1;
        let diagnostics = notifications
            .iter()
            .find(|notification| notification["method"] == "textDocument/publishDiagnostics")
            .expect("修复后也要通知一次清空");
        assert!(
            diagnostics["params"]["diagnostics"]
                .as_array()
                .unwrap()
                .is_empty()
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn keeps_unrelated_projects_apart() {
        let root = temp_project();
        let examples = root.join("examples");
        let right = root.join("right");
        fs::create_dir_all(&examples).unwrap();
        fs::create_dir_all(&right).unwrap();
        let left = examples.join("left.mcl");
        let right_main = right.join("main.mcl");
        let left_text = "namespace left;\nfn left_fn() { }\n";
        let right_text = "namespace right;\nfn main() { left_fn(); }\n";
        fs::write(&left, left_text).unwrap();
        fs::write(&right_main, right_text).unwrap();

        let mut session = Session::default();
        initialize(&mut session, &root);
        open(&mut session, &left, left_text);
        let notifications = open(&mut session, &right_main, right_text);

        // 不同命名空间的文件不会被并进同一个项目，也不会报命名空间不一致。
        let diagnostics = notifications
            .iter()
            .find(|notification| notification["params"]["uri"] == path_to_uri(&right_main))
            .expect("右侧项目应有诊断");
        let messages: Vec<&str> = diagnostics["params"]["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .map(|diagnostic| diagnostic["message"].as_str().unwrap())
            .collect();
        assert_eq!(messages, vec!["找不到函数 `left_fn`"]);

        let (response, _) = session.handle(&json!({
            "jsonrpc": "2.0",
            "id": 9,
            "method": "textDocument/completion",
            "params": {
                "textDocument": {"uri": path_to_uri(&right_main)},
                "position": {"line": 1, "character": 12},
            },
        }));
        let response = response.unwrap();
        let items = response["result"].as_array().unwrap();
        let labels: Vec<&str> = items
            .iter()
            .map(|item| item["label"].as_str().unwrap())
            .collect();
        assert!(!labels.contains(&"left_fn"), "补全不应泄露其他项目的符号");
        assert!(labels.contains(&"main"), "补全应包含本项目符号");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn completes_keywords_and_jumps_across_files() {
        let root = temp_project();
        let main = root.join("main.mcl");
        let helper = root.join("helper.mcl");
        fs::write(&main, "namespace demo;\nfn main() { helper(); }\n").unwrap();
        fs::write(&helper, "namespace demo;\nfn helper() { }\n").unwrap();
        let mut session = Session::default();
        initialize(&mut session, &root);
        open(
            &mut session,
            &main,
            "namespace demo;\nfn main() { helper(); }\n",
        );

        let (response, _) = session.handle(&json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "textDocument/completion",
            "params": {
                "textDocument": {"uri": path_to_uri(&main)},
                "position": {"line": 1, "character": 22},
            },
        }));
        let items = response.unwrap()["result"].as_array().unwrap().to_vec();
        assert!(
            items.iter().any(|item| item["label"] == "if"),
            "补全应包含关键词"
        );
        assert!(
            items.iter().any(|item| item["label"] == "helper"),
            "补全应包含跨文件函数"
        );

        let (response, _) = session.handle(&json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "textDocument/definition",
            "params": {
                "textDocument": {"uri": path_to_uri(&main)},
                "position": {"line": 1, "character": 17},
            },
        }));
        assert_eq!(
            response.unwrap()["result"]["uri"],
            path_to_uri(&helper),
            "定义应跳到 helper.mcl"
        );
        fs::remove_dir_all(root).unwrap();
    }
}
