//! 文本组件（1.3）的语义检查：颜色、本地化键、选择器、NBT 路径与事件。

use crate::ast::{
    ClickEvent, Holder, HoverEvent, MessageArgument, NbtComponentSource, ObjectContent,
    ObjectiveRef, SelectorValue, TextComponent, TextComponentKind,
};
use crate::diagnostic::Diagnostic;

use super::rules::valid_resource_location;
use super::statements::ValidationContext;

pub(super) fn validate_message_argument(
    message: &MessageArgument,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if message.text.trim().is_empty() || message.text.contains(['\n', '\r', '\0']) {
        diagnostics.push(Diagnostic::new(
            "聊天消息必须是非空的单行文本",
            message.span,
        ));
    }
    if message.text.encode_utf16().count() > 256 {
        diagnostics.push(Diagnostic::new(
            "聊天消息不能超过 256 个 UTF-16 字符",
            message.span,
        ));
    }
    if message.text.trim_end().ends_with('\\') {
        diagnostics.push(Diagnostic::new(
            "聊天消息不能以反斜杠结尾；Minecraft 会把下一行拼接进来",
            message.span,
        ));
    }
    // MessageArgument eagerly parses selectors before the command is loaded.
    // Filtered selectors need the complete Brigadier option grammar; reject
    // them here until that parser is available instead of emitting bad packs.
    if message
        .text
        .as_bytes()
        .windows(3)
        .any(|part| part[0] == b'@' && b"paresn".contains(&part[1]) && part[2] == b'[')
    {
        diagnostics.push(Diagnostic::new(
            "聊天消息暂不支持带 [...] 选项的实体选择器；请使用文本组件或无选项的 @a/@e 等",
            message.span,
        ));
    }
}

/// 校验整个组件树。
pub(super) fn validate_component(
    component: &TextComponent,
    ctx: ValidationContext<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let Some(color) = &component.style.color
        && !valid_component_color(color)
    {
        diagnostics.push(Diagnostic::new(
            format!("`{color}` 不是有效的文本颜色（16 个颜色名或 `#rrggbb`）"),
            component.span,
        ));
    }
    if let Some(click) = &component.style.click {
        validate_click(click, component.span, diagnostics);
    }
    if let Some(hover) = &component.style.hover {
        match hover {
            HoverEvent::Text(value) => validate_component(value, ctx, diagnostics),
            HoverEvent::Item { id, count } => {
                super::registry::validate_id("item", "悬停物品", id, component.span, diagnostics);
                if count.is_some_and(|count| !(1..=99).contains(&count)) {
                    diagnostics.push(Diagnostic::new(
                        "show_item 的数量必须在 1 到 99 之间",
                        component.span,
                    ));
                }
            }
            HoverEvent::Entity { id, uuid, name } => {
                super::registry::validate_id(
                    "entity_type",
                    "悬停实体",
                    id,
                    component.span,
                    diagnostics,
                );
                if !valid_uuid(uuid) {
                    diagnostics.push(Diagnostic::new(
                        format!("show_entity 的 `{uuid}` 不是有效的 UUID"),
                        component.span,
                    ));
                }
                if let Some(name) = name {
                    validate_component(name, ctx, diagnostics);
                }
            }
        }
    }
    match &component.kind {
        TextComponentKind::Text(_) => {}
        TextComponentKind::Object(content) => match content {
            ObjectContent::Atlas {
                atlas,
                sprite,
                fallback,
            } => {
                for id in atlas.iter().chain(std::iter::once(sprite)) {
                    if !valid_resource_location(id) {
                        diagnostics.push(Diagnostic::new(
                            format!("object 图集资源位置 `{id}` 无效"),
                            component.span,
                        ));
                    }
                }
                if let Some(fallback) = fallback {
                    validate_component(fallback, ctx, diagnostics);
                }
            }
            ObjectContent::Player { name, fallback, .. } => {
                if name.is_empty()
                    || name.len() > 16
                    || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
                {
                    diagnostics.push(Diagnostic::new(
                        format!(
                            "object 玩家名称 `{name}` 需要 1 到 16 个 ASCII 字母、数字或下划线"
                        ),
                        component.span,
                    ));
                }
                if let Some(fallback) = fallback {
                    validate_component(fallback, ctx, diagnostics);
                }
            }
        },
        TextComponentKind::Keybind(key) => {
            if !valid_translation_key(key) {
                diagnostics.push(Diagnostic::new(
                    format!("`{key}` 不是有效的按键名"),
                    component.span,
                ));
            }
        }
        TextComponentKind::Translate { key, args } => {
            if !valid_translation_key(key) {
                diagnostics.push(Diagnostic::new(
                    format!("`{key}` 不是有效的本地化键"),
                    component.span,
                ));
            }
            for arg in args {
                validate_component(arg, ctx, diagnostics);
            }
        }
        TextComponentKind::Score {
            holder,
            objective,
            objective_span,
        } => {
            validate_component_holder(holder, component.span, ctx, diagnostics);
            match objective {
                ObjectiveRef::Declared(name) => {
                    if !ctx.symbols.objectives.contains(name.as_str()) {
                        diagnostics.push(Diagnostic::new(
                            format!("找不到计分板目标 `{name}`；先声明 `objective {name};`"),
                            *objective_span,
                        ));
                    }
                }
                ObjectiveRef::Raw(name) => {
                    if name.is_empty() {
                        diagnostics.push(Diagnostic::new(
                            "score 组件的目标名不能为空",
                            *objective_span,
                        ));
                    }
                }
            }
        }
        TextComponentKind::Selector(value) => match value {
            SelectorValue::Raw(text, span) => {
                if !text.starts_with('@') {
                    diagnostics.push(Diagnostic::new(
                        format!("选择器组件需要 `@a`、`@e[...]` 这类选择器，实际为 `{text}`"),
                        *span,
                    ));
                }
            }
            SelectorValue::Query(name, span) => {
                if !ctx.symbols.queries.contains_key(name.as_str()) {
                    diagnostics.push(Diagnostic::new(format!("找不到实体查询 `{name}`"), *span));
                }
            }
        },
        TextComponentKind::Nbt {
            source,
            path,
            path_span,
            interpret,
            plain,
            separator,
        } => {
            if let Err(reason) = crate::ast::NbtPath::parse(path) {
                diagnostics.push(Diagnostic::new(
                    format!("`{path}` 不是有效的 NBT 路径：{reason}"),
                    *path_span,
                ));
            }
            if *interpret && *plain {
                diagnostics.push(Diagnostic::new(
                    "nbt 组件的 interpret 与 plain 不能同时为真",
                    component.span,
                ));
            }
            if let Some(separator) = separator {
                validate_component(separator, ctx, diagnostics);
            }
            validate_nbt_source(source, component.span, ctx, diagnostics);
        }
    }
}

/// 数据来源（`nbt` 组件与 `data.get` 共用）。
pub(super) fn validate_nbt_source(
    source: &NbtComponentSource,
    span: crate::ast::Span,
    ctx: ValidationContext<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match source {
        NbtComponentSource::Entity(holder) => {
            validate_component_holder(holder, span, ctx, diagnostics);
        }
        NbtComponentSource::Block(position) => {
            super::world::validate_block_position(position, diagnostics);
        }
        NbtComponentSource::Storage(storage, storage_span) => {
            if !valid_resource_location(storage) {
                diagnostics.push(Diagnostic::new(
                    format!("`{storage}` 不是有效的存储资源位置"),
                    *storage_span,
                ));
            }
        }
    }
}

/// Data commands use `EntityArgument.entity()`, so an entity query must be
/// syntactically limited to one result. Text components share
/// [`validate_nbt_source`] but allow multiple entities; only data entry points
/// call this stricter wrapper.
pub(super) fn validate_data_nbt_source(
    source: &NbtComponentSource,
    span: crate::ast::Span,
    ctx: ValidationContext<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    validate_nbt_source(source, span, ctx, diagnostics);
    if let NbtComponentSource::Entity(holder) = source {
        match holder {
            Holder::Origin => diagnostics.push(Diagnostic::new(
                "data 实体参数不接受 origin；请在 execute on origin 块中使用 self",
                span,
            )),
            Holder::Query(name, query_span)
                if ctx
                    .symbols
                    .queries
                    .get(name.as_str())
                    .is_some_and(|query| query.limit != Some(1)) =>
            {
                diagnostics.push(Diagnostic::new(
                    format!("data 实体参数查询 `{name}` 必须使用 limit(1)"),
                    *query_span,
                ));
            }
            Holder::SelfEntity | Holder::Query(_, _) => {}
        }
    }
}

/// Validate the writable target side of `data merge/remove/modify`.
pub(super) fn validate_writable_data_nbt_target(
    target: &NbtComponentSource,
    span: crate::ast::Span,
    ctx: ValidationContext<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    validate_data_nbt_source(target, span, ctx, diagnostics);
    let NbtComponentSource::Entity(holder) = target else {
        return;
    };
    match holder {
        Holder::SelfEntity if ctx.context != crate::compiler::types::ExecutionContext::Mob => {
            diagnostics.push(Diagnostic::new(
                "data 写入实体 NBT 时 self 必须处于确定的非玩家上下文",
                span,
            ));
        }
        Holder::Query(name, query_span)
            if ctx
                .symbols
                .queries
                .get(name.as_str())
                .is_some_and(|query| query.entity_type == "minecraft:player") =>
        {
            diagnostics.push(Diagnostic::new(
                format!("data 不能写入玩家 NBT：查询 `{name}` 匹配 minecraft:player"),
                *query_span,
            ));
        }
        Holder::SelfEntity | Holder::Origin | Holder::Query(_, _) => {}
    }
}

/// 组件里的持有者：`origin`（投掷者）无法在 JSON 组件里表达，直接拒绝。
fn validate_component_holder(
    holder: &Holder,
    span: crate::ast::Span,
    ctx: ValidationContext<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if matches!(holder, Holder::Origin) {
        diagnostics.push(Diagnostic::new(
            "文本组件里的持有者不能是投掷者；请用 self/自身 或实体查询",
            span,
        ));
        return;
    }
    super::statements::validate_holder(holder, span, ctx, diagnostics);
}

fn validate_click(click: &ClickEvent, span: crate::ast::Span, diagnostics: &mut Vec<Diagnostic>) {
    match click {
        ClickEvent::OpenUrl(url) => {
            if !(url.starts_with("https://") || url.starts_with("http://")) {
                diagnostics.push(Diagnostic::new(
                    format!("open_url 需要 http:// 或 https:// 链接，实际为 `{url}`"),
                    span,
                ));
            }
        }
        ClickEvent::RunCommand(command) | ClickEvent::SuggestCommand(command) => {
            if command.trim().is_empty() {
                diagnostics.push(Diagnostic::new("点击事件里的命令不能为空", span));
            }
            if command
                .chars()
                .any(|character| matches!(character as u32, 0..=31 | 127 | 167))
            {
                diagnostics.push(Diagnostic::new(
                    "run_command/suggest_command 含有 Minecraft 聊天字符串禁止的控制字符、DEL 或 §",
                    span,
                ));
            }
        }
        ClickEvent::CopyToClipboard(value) => {
            if value.is_empty() {
                diagnostics.push(Diagnostic::new("copy_to_clipboard 的文本不能为空", span));
            }
        }
        ClickEvent::ChangePage(page) => {
            if *page == 0 {
                diagnostics.push(Diagnostic::new("change_page 的页号从 1 开始", span));
            } else if *page > i32::MAX as u32 {
                diagnostics.push(Diagnostic::new(
                    "change_page 的页号不能超过 2147483647",
                    span,
                ));
            }
        }
        ClickEvent::ShowDialog(dialog) => {
            super::registry::validate_id("dialog", "对话框", dialog, span, diagnostics);
        }
        ClickEvent::Custom { id, .. } => {
            if !valid_resource_location(id) {
                diagnostics.push(Diagnostic::new(
                    format!("custom 的 `{id}` 不是有效的资源位置"),
                    span,
                ));
            }
        }
    }
}

fn valid_uuid(value: &str) -> bool {
    value.len() == 36
        && value.bytes().enumerate().all(|(index, byte)| {
            if [8, 13, 18, 23].contains(&index) {
                byte == b'-'
            } else {
                byte.is_ascii_hexdigit()
            }
        })
}

/// 16 个颜色名或 `#rrggbb`。
pub(super) fn valid_component_color(color: &str) -> bool {
    super::rules::valid_text_color(color)
        || (color.len() == 7
            && color.starts_with('#')
            && color[1..]
                .chars()
                .all(|character| character.is_ascii_hexdigit()))
}

/// 本地化键与按键名：小写字母、数字与 `._-`，至少一段。
fn valid_translation_key(key: &str) -> bool {
    !key.is_empty()
        && key.len() <= 256
        && key.chars().all(|character| {
            character.is_ascii_lowercase()
                || character.is_ascii_digit()
                || matches!(character, '.' | '_' | '-')
        })
}

/// 数据与文本组件共用的 NBT path 语法。
pub(super) fn valid_nbt_component_path(path: &str) -> bool {
    crate::ast::NbtPath::parse(path).is_ok()
}
