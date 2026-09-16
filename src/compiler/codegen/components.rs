//! 文本组件的 JSON 序列化：`tellraw` 与后续界面命令共用。

use serde_json::{Map, Value, json};

use crate::ast::{
    ClickEvent, Holder, MessageTarget, NbtComponentSource, ObjectiveRef, SelectorValue,
    TextComponent, TextComponentKind,
};

use super::Compiler;
use super::emit::entity_query_selector;
use super::names::user_objective_name;
use super::world;

impl Compiler<'_> {
    /// `tellraw <选择器> <组件 JSON>`。
    pub(super) fn compile_message(
        &self,
        target: &MessageTarget,
        component: &TextComponent,
    ) -> String {
        let selector = match target {
            MessageTarget::All => "@a".to_owned(),
            MessageTarget::SelfEntity => "@s".to_owned(),
            MessageTarget::Nearest { within } => {
                format!("@a[sort=nearest,limit=1,distance=..{within}]")
            }
            MessageTarget::Query { name, .. } => entity_query_selector(self.query(name)),
        };
        format!("tellraw {selector} {}", self.component_json(component))
    }

    /// 组件到 JSON 值。
    pub(super) fn component_json(&self, component: &TextComponent) -> Value {
        let mut object = Map::new();
        match &component.kind {
            TextComponentKind::Text(text) => {
                object.insert("text".to_owned(), json!(text));
            }
            TextComponentKind::Translate { key, args } => {
                object.insert("translate".to_owned(), json!(key));
                if !args.is_empty() {
                    object.insert(
                        "with".to_owned(),
                        Value::Array(args.iter().map(|arg| self.component_json(arg)).collect()),
                    );
                }
            }
            TextComponentKind::Keybind(key) => {
                object.insert("keybind".to_owned(), json!(key));
            }
            TextComponentKind::Score {
                holder, objective, ..
            } => {
                let objective = match objective {
                    ObjectiveRef::Declared(name) => {
                        user_objective_name(&self.program.namespace, name)
                    }
                    ObjectiveRef::Raw(name) => name.clone(),
                };
                object.insert(
                    "score".to_owned(),
                    json!({
                        "name": self.component_holder(holder),
                        "objective": objective,
                    }),
                );
            }
            TextComponentKind::Selector(value) => {
                let selector = match value {
                    SelectorValue::Raw(text, _) => text.clone(),
                    SelectorValue::Query(name, _) => entity_query_selector(self.query(name)),
                };
                object.insert("selector".to_owned(), json!(selector));
            }
            TextComponentKind::Nbt {
                source,
                path,
                path_span: _,
                interpret,
                plain,
                separator,
            } => {
                object.insert("nbt".to_owned(), json!(path));
                if *interpret {
                    object.insert("interpret".to_owned(), json!(true));
                }
                if *plain {
                    object.insert("plain".to_owned(), json!(true));
                }
                if let Some(separator) = separator {
                    object.insert("separator".to_owned(), self.component_json(separator));
                }
                match source {
                    NbtComponentSource::Entity(holder) => {
                        object.insert("entity".to_owned(), json!(self.component_holder(holder)));
                    }
                    NbtComponentSource::Block(position) => {
                        object.insert("block".to_owned(), json!(world::position_text(position)));
                    }
                    NbtComponentSource::Storage(storage, _) => {
                        object.insert("storage".to_owned(), json!(storage));
                    }
                }
            }
        }
        if let Some(color) = &component.style.color {
            object.insert("color".to_owned(), json!(color));
        }
        for (key, value) in [
            ("bold", component.style.bold),
            ("italic", component.style.italic),
            ("underlined", component.style.underlined),
            ("strikethrough", component.style.strikethrough),
            ("obfuscated", component.style.obfuscated),
        ] {
            if let Some(value) = value {
                object.insert(key.to_owned(), json!(value));
            }
        }
        if let Some(click) = &component.style.click {
            object.insert("click_event".to_owned(), click_event_json(click));
        }
        if let Some(hover) = &component.style.hover {
            object.insert(
                "hover_event".to_owned(),
                json!({
                    "action": "show_text",
                    "value": self.component_json(hover),
                }),
            );
        }
        Value::Object(object)
    }

    /// 组件里的持有者选择器；`origin` 已在校验阶段拒绝。
    pub(super) fn component_holder(&self, holder: &Holder) -> String {
        match holder {
            Holder::SelfEntity | Holder::Origin => "@s".to_owned(),
            Holder::Query(name, _) => entity_query_selector(self.query(name)),
        }
    }
}

/// 点击事件的 JSON（26.3 的 `click_event` 字段）。
fn click_event_json(click: &ClickEvent) -> Value {
    match click {
        ClickEvent::OpenUrl(url) => json!({"action": "open_url", "url": url}),
        ClickEvent::RunCommand(command) => json!({"action": "run_command", "command": command}),
        ClickEvent::SuggestCommand(command) => {
            json!({"action": "suggest_command", "command": command})
        }
        ClickEvent::CopyToClipboard(value) => {
            json!({"action": "copy_to_clipboard", "value": value})
        }
        ClickEvent::ChangePage(page) => json!({"action": "change_page", "page": page}),
    }
}
