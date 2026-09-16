//! 文本组件（1.3）的语义检查：颜色、本地化键、选择器、NBT 路径与事件。

use crate::ast::{
    ClickEvent, Holder, NbtComponentSource, ObjectiveRef, SelectorValue, TextComponent,
    TextComponentKind,
};
use crate::diagnostic::Diagnostic;

use super::rules::valid_resource_location;
use super::statements::ValidationContext;

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
        validate_component(hover, ctx, diagnostics);
    }
    match &component.kind {
        TextComponentKind::Text(_) => {}
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
            if !valid_nbt_component_path(path) {
                diagnostics.push(Diagnostic::new(
                    format!("`{path}` 不是有效的 NBT 路径"),
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
        }
        ClickEvent::CopyToClipboard(value) => {
            if value.is_empty() {
                diagnostics.push(Diagnostic::new("copy_to_clipboard 的文本不能为空", span));
            }
        }
        ClickEvent::ChangePage(page) => {
            if *page == 0 {
                diagnostics.push(Diagnostic::new("change_page 的页号从 1 开始", span));
            }
        }
    }
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

/// NBT 组件路径：点分、允许列表下标与引号键，括号必须配对。
pub(super) fn valid_nbt_component_path(path: &str) -> bool {
    if path.is_empty() || path.len() > 1024 || path.starts_with('.') || path.ends_with('.') {
        return false;
    }
    let mut depth = 0i32;
    let mut quote: Option<char> = None;
    for character in path.chars() {
        if let Some(opening) = quote {
            if character == opening {
                quote = None;
            }
            continue;
        }
        match character {
            '"' | '\'' => quote = Some(character),
            '[' | '{' => depth += 1,
            ']' | '}' => {
                depth -= 1;
                if depth < 0 {
                    return false;
                }
            }
            _ => {}
        }
    }
    depth == 0 && quote.is_none()
}
