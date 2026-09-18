use super::*;

use std::collections::BTreeSet;
use std::fs;

use crate::analysis::analyze;

use crate::discover_sources;
use crate::lsp::convert::path_to_uri;

impl Session {
    pub(super) fn initialize(&mut self, params: &Value) -> Value {
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

    pub(super) fn change_workspace_folders(&mut self, params: &Value) {
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
    pub(super) fn refresh(&mut self) -> Vec<Value> {
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
    pub(super) fn request_document<'a>(
        &'a self,
        params: &Value,
    ) -> Option<(PathBuf, &'a str, &'a Project)> {
        let path = absolute(&uri_to_path(document_uri(params)?)?);
        let project = self.project_for(&path)?;
        let source = project.sources.iter().find(|source| source.path == path)?;
        Some((path, source.text.as_str(), project))
    }
}
