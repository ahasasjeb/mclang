use super::*;

use crate::lsp::features;

impl Session {
    /// 处理一条消息，返回响应（若为请求）和需要发出的通知。
    pub fn handle(&mut self, message: &Value) -> (Option<Value>, Vec<Value>) {
        let Some(method) = message.get("method").and_then(Value::as_str) else {
            // 客户端对服务器请求的响应；本服务器不发起请求，直接忽略。
            return (None, Vec::new());
        };
        let params = message.get("params").unwrap_or(&Value::Null);
        match message.get("id") {
            Some(id) => (Some(self.request(method, id, params)), Vec::new()),
            None => (None, self.notification(method, params)),
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
                let offset = document_offset(text, params, &project.indexes[&path]);
                response(
                    id,
                    features::completion(text, offset, &path, &project.analysis.symbols),
                )
            }
            "textDocument/hover" => {
                let Some((path, text, project)) = self.request_document(params) else {
                    return response(id, Value::Null);
                };
                let offset = document_offset(text, params, &project.indexes[&path]);
                let hover = features::hover(
                    text,
                    offset,
                    &path,
                    &project.analysis.symbols,
                    &project.sources,
                    &project.indexes,
                );
                response(id, hover.unwrap_or(Value::Null))
            }
            "textDocument/definition" => {
                let Some((path, text, project)) = self.request_document(params) else {
                    return response(id, Value::Null);
                };
                let offset = document_offset(text, params, &project.indexes[&path]);
                let definition = features::definition(
                    text,
                    offset,
                    &path,
                    &project.analysis.symbols,
                    &project.sources,
                    &project.indexes,
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
                if self.apply_document_change(params) {
                    self.refresh()
                } else {
                    Vec::new()
                }
            }
            "textDocument/didClose" => {
                if let Some(path) = closed_document(params) {
                    self.open.remove(&path);
                    self.invalidate_disk_text(&path);
                }
                self.refresh()
            }
            "textDocument/didSave" => {
                if let Some(path) = closed_document(params) {
                    self.invalidate_disk_text(&path);
                }
                self.refresh()
            }
            "workspace/didChangeWatchedFiles" => {
                if let Some(changes) = params.get("changes").and_then(Value::as_array) {
                    for change in changes {
                        if let Some(path) = change
                            .get("uri")
                            .and_then(Value::as_str)
                            .and_then(uri_to_path)
                        {
                            self.invalidate_disk_text(&absolute(&path));
                        }
                    }
                }
                self.discovery_dirty = true;
                self.refresh()
            }
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

    pub(super) fn apply_document_change(&mut self, params: &Value) -> bool {
        let Some((path, text)) = changed_document(params) else {
            return false;
        };
        if self.open.get(&path) == Some(&text) {
            return false;
        }
        self.namespaces.remove(&path);
        self.open.insert(path, text);
        true
    }
}
