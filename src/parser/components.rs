//! 文本组件（1.3）的解析。
//!
//! 组件语法是 `<构造器>(...)` 加可选样式块，例如：
//!
//! ```text
//! text("你好") { color = "red"; bold = 真; click = run_command("/say hi"); }
//! translate("chat.type.text", [text("甲"), text("乙")])
//! nbt(entity, self, "CustomName") { interpret = 假; }
//! ```

use crate::ast::{
    ClickEvent, ItemConditionSource, NbtComponentSource, ObjectiveRef, SelectorValue,
    TextComponent, TextComponentKind, TextStyle,
};
use crate::diagnostic::Diagnostic;
use crate::lexer::TokenKind;

use super::Parser;
use super::keywords::{boolean_word, click_action, nbt_source, text_color, text_style_property};

/// 样式块解析结果：通用样式 + `nbt` 组件专用开关。
#[derive(Default)]
struct StyleBlock {
    style: TextStyle,
    interpret: Option<bool>,
    plain: Option<bool>,
    separator: Option<Box<TextComponent>>,
}

impl Parser {
    /// 解析一个文本组件；当前位置不是组件时报错。
    pub(super) fn text_component(&mut self, label: &str) -> Result<TextComponent, Diagnostic> {
        if let Some(component) = self.try_text_component()? {
            return Ok(component);
        }
        Err(Diagnostic::new(
            format!(
                "{label}需要文本组件，例如 text(\"你好\")、translate(\"键\")、selector(\"@a\")"
            ),
            self.current().span,
        ))
    }

    /// 消息等内容参数：字符串字面量等价于 `text("...")`，也可以是结构化组件。
    pub(super) fn text_component_or_string(
        &mut self,
        label: &str,
    ) -> Result<TextComponent, Diagnostic> {
        if let Some(component) = self.try_text_component()? {
            return Ok(component);
        }
        let (text, span) = self.string(&format!("{label}需要字符串或文本组件"))?;
        Ok(TextComponent {
            kind: TextComponentKind::Text(text),
            style: TextStyle::default(),
            span,
        })
    }

    /// 尝试解析文本组件；返回 `None` 表示当前位置不是组件构造器。
    pub(super) fn try_text_component(&mut self) -> Result<Option<TextComponent>, Diagnostic> {
        if let Some(start) = self.take_word("text") {
            self.expect(TokenKind::LeftParen, "text 后需要 `(`")?;
            let (text, _) = self.string("text 需要字符串")?;
            let end = self.expect(TokenKind::RightParen, "text 缺少 `)`")?.span;
            let block = self.text_style_block()?;
            return Ok(Some(self.build_component(
                TextComponentKind::Text(text),
                block,
                start.span.merge(end),
                false,
            )?));
        }
        if let Some(start) = self.take_word("translate") {
            self.expect(TokenKind::LeftParen, "translate 后需要 `(`")?;
            let (key, _) = self.string("translate 需要本地化键字符串")?;
            let mut args = Vec::new();
            if self.take(&TokenKind::Comma).is_some() {
                self.expect(TokenKind::LeftBracket, "translate 参数需要 `[`")?;
                if self.take(&TokenKind::RightBracket).is_none() {
                    loop {
                        args.push(self.text_component("translate 参数")?);
                        if self.take(&TokenKind::Comma).is_none() {
                            break;
                        }
                    }
                    self.expect(TokenKind::RightBracket, "translate 参数列表缺少 `]`")?;
                }
            }
            let end = self
                .expect(TokenKind::RightParen, "translate 缺少 `)`")?
                .span;
            let block = self.text_style_block()?;
            return Ok(Some(self.build_component(
                TextComponentKind::Translate { key, args },
                block,
                start.span.merge(end),
                false,
            )?));
        }
        if let Some(start) = self.take_word("keybind") {
            self.expect(TokenKind::LeftParen, "keybind 后需要 `(`")?;
            let (key, _) = self.string("keybind 需要按键名字符串")?;
            let end = self.expect(TokenKind::RightParen, "keybind 缺少 `)`")?.span;
            let block = self.text_style_block()?;
            return Ok(Some(self.build_component(
                TextComponentKind::Keybind(key),
                block,
                start.span.merge(end),
                false,
            )?));
        }
        if let Some(start) = self.take_word("score") {
            self.expect(TokenKind::LeftParen, "score 后需要 `(`")?;
            let holder = self.score_holder()?;
            self.expect(TokenKind::Comma, "score 持有者后需要 `,`")?;
            let (objective, objective_span) = if matches!(self.current().kind, TokenKind::String(_))
            {
                let (name, span) = self.string("score 需要目标名称")?;
                (ObjectiveRef::Raw(name), span)
            } else {
                let (name, span) = self.ident("score 需要已声明的目标名称或字符串")?;
                (ObjectiveRef::Declared(name), span)
            };
            let end = self.expect(TokenKind::RightParen, "score 缺少 `)`")?.span;
            let block = self.text_style_block()?;
            return Ok(Some(self.build_component(
                TextComponentKind::Score {
                    holder,
                    objective,
                    objective_span,
                },
                block,
                start.span.merge(end),
                false,
            )?));
        }
        if let Some(start) = self.take_word("selector") {
            self.expect(TokenKind::LeftParen, "selector 后需要 `(`")?;
            let value = if matches!(self.current().kind, TokenKind::String(_)) {
                let (selector, span) = self.string("selector 需要选择器字符串")?;
                SelectorValue::Raw(selector, span)
            } else {
                let (name, span) = self.ident("selector 需要选择器字符串或实体查询名称")?;
                SelectorValue::Query(name, span)
            };
            let end = self
                .expect(TokenKind::RightParen, "selector 缺少 `)`")?
                .span;
            let block = self.text_style_block()?;
            return Ok(Some(self.build_component(
                TextComponentKind::Selector(value),
                block,
                start.span.merge(end),
                false,
            )?));
        }
        if let Some(start) = self.take_word("nbt") {
            self.expect(TokenKind::LeftParen, "nbt 组件后需要 `(`")?;
            let source = self.nbt_source_value("nbt 组件来源")?;
            self.expect(TokenKind::Comma, "nbt 组件来源后需要 `,`")?;
            let (path, path_span) = self.string("nbt 组件需要路径字符串")?;
            let end = self.expect(TokenKind::RightParen, "nbt 组件缺少 `)`")?.span;
            let mut block = self.text_style_block()?;
            let interpret = block.interpret.unwrap_or(false);
            let plain = block.plain.unwrap_or(false);
            let separator = block.separator.take();
            let kind = TextComponentKind::Nbt {
                source,
                path,
                path_span,
                interpret,
                plain,
                separator,
            };
            return Ok(Some(self.build_component(
                kind,
                block,
                start.span.merge(end),
                true,
            )?));
        }
        Ok(None)
    }

    /// 解析 NBT 数据来源：`entity, <持有者>`、`block, <坐标>` 或
    /// `storage, "<资源位置>"`；逗号之后的部分留给调用方。
    pub(super) fn nbt_source_value(
        &mut self,
        label: &str,
    ) -> Result<NbtComponentSource, Diagnostic> {
        let (source_name, source_span) =
            self.ident(&format!("{label}需要 entity、block 或 storage"))?;
        let Some(source_kind) = nbt_source(&source_name) else {
            return Err(Diagnostic::new(
                format!(
                    "未知 NBT 来源 `{source_name}`，可用 entity（实体）、block（方块）或 storage（存储）"
                ),
                source_span,
            ));
        };
        self.expect(TokenKind::Comma, "NBT 来源后需要 `,`")?;
        Ok(match source_kind {
            "entity" => NbtComponentSource::Entity(self.score_holder()?),
            "block" => NbtComponentSource::Block(self.block_position("NBT 方块坐标")?),
            _ => {
                let (storage, span) = self.string("storage 来源需要存储资源位置字符串")?;
                NbtComponentSource::Storage(storage, span)
            }
        })
    }

    /// `if items`/`if slots` 的来源：`entity, <持有者>` 或 `block, <坐标>`；
    /// 逗号之后的部分留给调用方。
    pub(super) fn item_condition_source(
        &mut self,
        label: &str,
    ) -> Result<ItemConditionSource, Diagnostic> {
        let (source_name, source_span) = self.ident(&format!("{label}需要 entity 或 block"))?;
        self.expect(TokenKind::Comma, "物品条件来源后需要 `,`")?;
        match source_name.as_str() {
            "entity" | "实体" => Ok(ItemConditionSource::Entity(self.score_holder()?)),
            "block" | "方块" => Ok(ItemConditionSource::Block(
                self.block_position("物品条件方块坐标")?,
            )),
            _ => Err(Diagnostic::new(
                format!("未知物品条件来源 `{source_name}`，可用 entity（实体）或 block（方块）"),
                source_span,
            )),
        }
    }

    /// 组装组件；`nbt_only` 表示该组件是否允许 `interpret`/`plain`/`separator`。
    fn build_component(
        &self,
        kind: TextComponentKind,
        block: StyleBlock,
        span: crate::ast::Span,
        nbt_only: bool,
    ) -> Result<TextComponent, Diagnostic> {
        if !nbt_only {
            for (name, used) in [
                ("interpret", block.interpret.is_some()),
                ("plain", block.plain.is_some()),
                ("separator", block.separator.is_some()),
            ] {
                if used {
                    return Err(Diagnostic::new(format!("{name} 只适用于 nbt 组件"), span));
                }
            }
        }
        Ok(TextComponent {
            kind,
            style: block.style,
            span,
        })
    }

    /// 解析可选的 `{ 属性 = 值; ... }` 样式块。
    fn text_style_block(&mut self) -> Result<StyleBlock, Diagnostic> {
        let mut block = StyleBlock::default();
        if self.take(&TokenKind::LeftBrace).is_none() {
            return Ok(block);
        }
        loop {
            while self.take(&TokenKind::Semicolon).is_some()
                || self.take(&TokenKind::Comma).is_some()
            {}
            if self.take(&TokenKind::RightBrace).is_some() {
                break;
            }
            if self.check(&TokenKind::Eof) {
                return Err(Diagnostic::new(
                    "文本组件样式块缺少 `}`",
                    self.current().span,
                ));
            }
            let (name, span) = self.ident("样式属性名")?;
            let Some(property) = text_style_property(&name) else {
                return Err(Diagnostic::new(
                    format!(
                        "未知文本样式属性 `{name}`，可用 color、bold、italic、underlined、strikethrough、obfuscated、click、hover，nbt 组件还支持 interpret、plain、separator"
                    ),
                    span,
                ));
            };
            self.expect(TokenKind::Equal, "样式属性后需要 `=`")?;
            match property {
                "color" => {
                    if block.style.color.is_some() {
                        return Err(Diagnostic::new("重复设置样式属性 color", span));
                    }
                    let (value, _) = self.string("color 需要颜色字符串")?;
                    // 颜色字符串允许中英文颜色名，统一归一化为英文。
                    let value = text_color(&value).unwrap_or(&value).to_owned();
                    block.style.color = Some(value);
                }
                "bold" | "italic" | "underlined" | "strikethrough" | "obfuscated" => {
                    let value = self.boolean_value("样式开关")?;
                    let slot = match property {
                        "bold" => &mut block.style.bold,
                        "italic" => &mut block.style.italic,
                        "underlined" => &mut block.style.underlined,
                        "strikethrough" => &mut block.style.strikethrough,
                        _ => &mut block.style.obfuscated,
                    };
                    if slot.replace(value).is_some() {
                        return Err(Diagnostic::new(
                            format!("重复设置样式属性 {property}"),
                            span,
                        ));
                    }
                }
                "click" => {
                    if block.style.click.is_some() {
                        return Err(Diagnostic::new("重复设置样式属性 click", span));
                    }
                    block.style.click = Some(self.click_event()?);
                }
                "hover" => {
                    if block.style.hover.is_some() {
                        return Err(Diagnostic::new("重复设置样式属性 hover", span));
                    }
                    block.style.hover = Some(Box::new(self.text_component("悬停内容")?));
                }
                "interpret" | "plain" => {
                    let value = self.boolean_value("nbt 组件开关")?;
                    let slot = if property == "interpret" {
                        &mut block.interpret
                    } else {
                        &mut block.plain
                    };
                    if slot.replace(value).is_some() {
                        return Err(Diagnostic::new(
                            format!("重复设置样式属性 {property}"),
                            span,
                        ));
                    }
                }
                "separator" => {
                    if block.separator.is_some() {
                        return Err(Diagnostic::new("重复设置样式属性 separator", span));
                    }
                    block.separator = Some(Box::new(self.text_component("分隔符")?));
                }
                _ => unreachable!("text_style_property 只返回已知属性"),
            }
        }
        Ok(block)
    }

    /// `click = run_command("...")` 等点击事件。
    fn click_event(&mut self) -> Result<ClickEvent, Diagnostic> {
        let (action, span) = self.ident("点击事件动作")?;
        let Some(action) = click_action(&action) else {
            return Err(Diagnostic::new(
                format!(
                    "未知点击事件 `{action}`，可用 open_url、run_command、suggest_command、copy_to_clipboard、change_page"
                ),
                span,
            ));
        };
        self.expect(TokenKind::LeftParen, "点击事件后需要 `(`")?;
        let event = match action {
            "open_url" => {
                let (url, _) = self.string("open_url 需要链接字符串")?;
                ClickEvent::OpenUrl(url)
            }
            "run_command" => {
                let (command, _) = self.string("run_command 需要命令字符串")?;
                ClickEvent::RunCommand(command)
            }
            "suggest_command" => {
                let (command, _) = self.string("suggest_command 需要命令字符串")?;
                ClickEvent::SuggestCommand(command)
            }
            "copy_to_clipboard" => {
                let (value, _) = self.string("copy_to_clipboard 需要文本字符串")?;
                ClickEvent::CopyToClipboard(value)
            }
            _ => {
                let page = self.unsigned("change_page 需要页号")?;
                ClickEvent::ChangePage(page)
            }
        };
        self.expect(TokenKind::RightParen, "点击事件缺少 `)`")?;
        Ok(event)
    }

    /// 布尔字面量：`true`/`false` 与中文 `真`/`假`。
    fn boolean_value(&mut self, label: &str) -> Result<bool, Diagnostic> {
        let (value, span) = self.ident(label)?;
        match boolean_word(&value) {
            Some("true") => Ok(true),
            Some("false") => Ok(false),
            _ => Err(Diagnostic::new(
                format!("{label}需要 true/false（中文 `真`/`假`），实际为 `{value}`"),
                span,
            )),
        }
    }
}
