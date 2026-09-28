use super::*;

use std::collections::{BTreeSet, HashMap, HashSet};
use std::fs;

use crate::analysis::analyze;

use crate::discover_sources;
use crate::lines::LineIndex;
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
        self.disk_text.clear();
        self.namespaces.clear();
    }

    /// 重新收集项目、逐个分析并发布诊断。
    pub(super) fn refresh(&mut self) -> Vec<Value> {
        let mut previous: BTreeMap<_, _> = std::mem::take(&mut self.projects)
            .into_iter()
            .map(|project| (project.key.clone(), project))
            .collect();
        self.projects = self.collect_projects();
        let active_paths: HashSet<&Path> = self
            .projects
            .iter()
            .flat_map(|project| project.sources.iter().map(|source| source.path.as_path()))
            .collect();
        self.disk_text
            .retain(|path, _| active_paths.contains(path.as_path()));
        for project in &mut self.projects {
            if let Some(cached) = previous.remove(&project.key)
                && cached.sources == project.sources
            {
                project.analysis = cached.analysis;
            } else {
                project.analysis = analyze(&project.sources);
            }
        }
        self.publish_diagnostics()
    }

    /// Reuse disk reads between edits. Text is still copied into the analysis
    /// snapshot; explicit file events invalidate stamps that may be unchanged.
    fn cached_disk_text(&mut self, path: &Path) -> Option<String> {
        let stamp = fs::metadata(path).ok().map(|metadata| DiskStamp {
            len: metadata.len(),
            modified: metadata
                .modified()
                .ok()
                .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|delta| (delta.as_secs(), delta.subsec_nanos())),
        })?;
        if let Some((cached_stamp, text)) = self.disk_text.get(path)
            && stamp.modified.is_some()
            && *cached_stamp == stamp
        {
            return Some(text.clone());
        }
        let text = fs::read_to_string(path).ok()?;
        self.disk_text
            .insert(path.to_path_buf(), (stamp, text.clone()));
        Some(text)
    }

    pub(super) fn invalidate_disk_text(&mut self, path: &Path) {
        self.disk_text.remove(path);
        self.namespaces.remove(path);
    }

    /// 收集所有打开文档所属项目的源文件。
    ///
    /// 工作区里可以并存多个互不相关的项目（例如本仓库的 `examples/`）。项目边界由
    /// 打开文档推断：向上走到最远的、目录里没有其他命名空间的父目录；项目内只保留
    /// 同命名空间的文件。每个项目单独分析，符号表与诊断不会互相污染。
    fn collect_projects(&mut self) -> Vec<Project> {
        self.ensure_discovered();
        let index = match &self.discovery_index {
            Some(index) => Arc::clone(index),
            None => {
                let index = Arc::new(self.build_discovery_index());
                self.discovery_index = Some(Arc::clone(&index));
                index
            }
        };
        let open_paths: Vec<PathBuf> = self.open.keys().cloned().collect();

        let mut keys: Vec<(PathBuf, Option<String>)> = Vec::new();
        let mut seen_keys = HashSet::new();
        for path in &open_paths {
            let key = self.project_root(path, &index.files, &index.direct_files);
            if seen_keys.insert(key.clone()) {
                keys.push(key);
            }
        }

        keys.into_iter()
            .map(|(root, namespace)| {
                let mut texts: BTreeMap<PathBuf, String> = BTreeMap::new();
                if let Some(indices) = index.subtree_files.get(&root) {
                    for file_index in indices.iter().take(MAX_PROJECT_FILES) {
                        let path = &index.files[*file_index];
                        // Open buffers are authoritative and will be added below.
                        if self.open.contains_key(path) {
                            continue;
                        }
                        if !self.namespace_matches(path, &namespace) {
                            continue;
                        }
                        if let Some(text) = self.cached_disk_text(path) {
                            texts.insert(path.clone(), text);
                        }
                    }
                }
                // 打开的文档覆盖磁盘内容，未保存的编辑也能得到诊断。
                for path in &open_paths {
                    if path.starts_with(&root) && self.namespace_matches(path, &namespace) {
                        texts.insert(path.clone(), self.open[path].clone());
                    }
                }
                Project {
                    key: (root, namespace),
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

    fn build_discovery_index(&self) -> DiscoveryIndex {
        let files = self.discovered_files();
        let mut direct_files: HashMap<PathBuf, Vec<usize>> = HashMap::new();
        let mut subtree_files: HashMap<PathBuf, Vec<usize>> = HashMap::new();
        for (index, path) in files.iter().enumerate() {
            if let Some(parent) = path.parent() {
                direct_files
                    .entry(parent.to_path_buf())
                    .or_default()
                    .push(index);
            }
            for ancestor in path.ancestors().skip(1) {
                subtree_files
                    .entry(ancestor.to_path_buf())
                    .or_default()
                    .push(index);
            }
        }
        DiscoveryIndex {
            files,
            direct_files,
            subtree_files,
        }
    }

    fn ensure_discovered(&mut self) {
        let dirty = self.discovery_dirty;
        if dirty {
            let roots = self.roots.clone();
            self.discovered.retain(|root, _| roots.contains(root));
        }
        let mut changed = dirty;
        for root in &self.roots {
            if dirty || !self.discovered.contains_key(root) {
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
                changed = true;
            }
        }
        self.discovery_dirty = false;
        if changed {
            self.discovery_index = None;
        }
    }

    fn discovered_files(&self) -> Vec<PathBuf> {
        let mut files: Vec<PathBuf> = self.discovered.values().flatten().cloned().collect();
        files.sort();
        files.dedup();
        files
    }

    /// 打开文档所属项目：项目根目录与命名空间。
    fn project_root(
        &mut self,
        file: &Path,
        discovered: &[PathBuf],
        direct_files: &HashMap<PathBuf, Vec<usize>>,
    ) -> (PathBuf, Option<String>) {
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
                || !self.directory_compatible(
                    &parent,
                    namespace.as_deref(),
                    discovered,
                    direct_files,
                )
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
        direct_files: &HashMap<PathBuf, Vec<usize>>,
    ) -> bool {
        let Some(namespace) = namespace else {
            return true;
        };
        direct_files
            .get(directory)
            .into_iter()
            .flatten()
            .all(|index| {
                self.namespace_of(&discovered[*index])
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
            if project.analysis.diagnostics.is_empty() {
                continue;
            }
            let sources: HashMap<&Path, &SourceFile> = project
                .sources
                .iter()
                .map(|source| (source.path.as_path(), source))
                .collect();
            // 同一文件的所有诊断共用一个行首索引，避免按诊断数重扫源码前缀。
            let mut indexes: HashMap<&Path, LineIndex> = HashMap::new();
            for diagnostic in &project.analysis.diagnostics {
                let Some(source) = sources.get(diagnostic.path.as_path()) else {
                    continue;
                };
                let index = indexes
                    .entry(diagnostic.path.as_path())
                    .or_insert_with(|| LineIndex::new(&source.text));
                grouped
                    .entry(diagnostic.path.clone())
                    .or_default()
                    .push(diagnostic_json(&source.text, index, diagnostic));
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
