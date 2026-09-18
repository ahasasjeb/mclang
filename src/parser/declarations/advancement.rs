use crate::ast::*;
use crate::diagnostic::Diagnostic;
use crate::lexer::TokenKind;

use crate::parser::Parser;
use crate::parser::keywords::{
    advancement_frame, advancement_property, advancement_requirements, boolean_word,
    criterion_property, display_property, reward_property,
};

impl Parser {
    /// `advancement 名称 { parent = …; criterion …; reward {…}; display {…}; }`
    ///
    /// 进度是数据包的事件入口：`criterion` 监听原版触发器，命中后由
    /// `reward.function` 指向的函数接管。条件本身是触发器的原始 JSON，
    /// 触发器名、奖励引用与展示字段在编译期检查。
    pub(in crate::parser) fn advancement(&mut self) -> Result<AdvancementDecl, Diagnostic> {
        let start = self.expect_word("advancement")?.span;
        let (name, name_span) = self.ident("进度名称")?;
        self.expect(TokenKind::LeftBrace, "进度声明需要 `{`")?;
        let mut parent = None;
        let mut criteria = Vec::new();
        let mut requirements = AdvancementRequirements::All;
        let mut requirements_span = None;
        let mut reward = None;
        let mut display = None;
        while !self.check(&TokenKind::RightBrace) {
            if self.check(&TokenKind::Eof) {
                return Err(Diagnostic::new("进度声明缺少 `}`", self.current().span));
            }
            let (property, property_span) = self.ident("进度属性")?;
            let Some(property) = advancement_property(&property) else {
                return Err(Diagnostic::new(
                    format!("未知进度属性 `{property}`"),
                    property_span,
                ));
            };
            match property {
                "parent" => {
                    if parent.is_some() {
                        return Err(Diagnostic::new("进度只能声明一次 parent", property_span));
                    }
                    self.expect(TokenKind::Equal, "parent 后需要 `=`")?;
                    parent = Some(self.advancement_reference("父进度")?);
                    self.expect(TokenKind::Semicolon, "parent 后需要 `;`")?;
                }
                "criterion" => criteria.push(self.advancement_criterion(property_span)?),
                "requirements" => {
                    if requirements_span.is_some() {
                        return Err(Diagnostic::new(
                            "进度只能声明一次 requirements",
                            property_span,
                        ));
                    }
                    self.expect(TokenKind::Equal, "requirements 后需要 `=`")?;
                    let (value, value_span) = self.ident("all 或 any")?;
                    let Some(value) = advancement_requirements(&value) else {
                        return Err(Diagnostic::new(
                            "requirements 只能是 all/全部 或 any/任意",
                            value_span,
                        ));
                    };
                    self.expect(TokenKind::Semicolon, "requirements 后需要 `;`")?;
                    requirements = value;
                    requirements_span = Some(property_span);
                }
                "reward" => {
                    if reward.is_some() {
                        return Err(Diagnostic::new("进度只能声明一次 reward", property_span));
                    }
                    reward = Some(self.advancement_reward()?);
                }
                "display" => {
                    if display.is_some() {
                        return Err(Diagnostic::new("进度只能声明一次 display", property_span));
                    }
                    display = Some(self.advancement_display()?);
                }
                _ => unreachable!(),
            }
        }
        let end = self.advance().span;
        self.take(&TokenKind::Semicolon);
        if criteria.is_empty() {
            return Err(Diagnostic::new(
                "进度必须声明至少一条 criterion",
                start.merge(end),
            ));
        }
        Ok(AdvancementDecl {
            exported: false,
            name,
            name_span,
            parent,
            criteria,
            requirements,
            reward,
            display,
            span: start.merge(end),
        })
    }

    /// 资源引用：标识符是本命名空间声明名，字符串是外部资源位置。
    pub(in crate::parser) fn advancement_reference(
        &mut self,
        label: &str,
    ) -> Result<AdvancementReference, Diagnostic> {
        let token = self.advance().clone();
        match token.kind {
            TokenKind::Ident(value) => Ok(AdvancementReference {
                name: value,
                span: token.span,
                external: false,
            }),
            TokenKind::String(value) => Ok(AdvancementReference {
                name: value,
                span: token.span,
                external: true,
            }),
            _ => Err(Diagnostic::new(
                format!("这里需要{label}名称或资源位置字符串"),
                token.span,
            )),
        }
    }

    /// `criterion 名称 { trigger = …; conditions = """…"""; }`
    fn advancement_criterion(&mut self, start: Span) -> Result<AdvancementCriterion, Diagnostic> {
        let (name, name_span) = self.ident("准则名称")?;
        self.expect(TokenKind::LeftBrace, "准则需要 `{`")?;
        let mut trigger = None;
        let mut conditions = None;
        while !self.check(&TokenKind::RightBrace) {
            if self.check(&TokenKind::Eof) {
                return Err(Diagnostic::new("准则缺少 `}`", self.current().span));
            }
            let (property, property_span) = self.ident("准则属性")?;
            let Some(property) = criterion_property(&property) else {
                return Err(Diagnostic::new(
                    format!("未知准则属性 `{property}`"),
                    property_span,
                ));
            };
            match property {
                "trigger" => {
                    if trigger.is_some() {
                        return Err(Diagnostic::new("准则只能声明一次 trigger", property_span));
                    }
                    self.expect(TokenKind::Equal, "trigger 后需要 `=`")?;
                    let token = self.advance().clone();
                    let value = match token.kind {
                        TokenKind::Ident(value) | TokenKind::String(value) => value,
                        _ => {
                            return Err(Diagnostic::new("trigger 需要触发器名称", token.span));
                        }
                    };
                    self.expect(TokenKind::Semicolon, "trigger 后需要 `;`")?;
                    trigger = Some((value, token.span));
                }
                "conditions" => {
                    if conditions.is_some() {
                        return Err(Diagnostic::new(
                            "准则只能声明一次 conditions",
                            property_span,
                        ));
                    }
                    self.expect(TokenKind::Equal, "conditions 后需要 `=`")?;
                    let (json, span) = self.string("conditions 需要 JSON 字符串")?;
                    self.expect(TokenKind::Semicolon, "conditions 后需要 `;`")?;
                    conditions = Some((json, span));
                }
                _ => unreachable!(),
            }
        }
        let end = self.advance().span;
        let span = start.merge(end);
        let Some((trigger, trigger_span)) = trigger else {
            return Err(Diagnostic::new("准则必须声明 trigger", span));
        };
        Ok(AdvancementCriterion {
            name,
            name_span,
            trigger,
            trigger_span,
            conditions: conditions.as_ref().map(|(json, _)| json.clone()),
            conditions_span: conditions.map(|(_, span)| span),
            span,
        })
    }

    /// `reward { function = …; experience = …; loot = …; recipe = …; }`
    fn advancement_reward(&mut self) -> Result<AdvancementReward, Diagnostic> {
        self.expect(TokenKind::LeftBrace, "reward 后需要 `{`")?;
        let mut function = None;
        let mut experience = None;
        let mut loot = Vec::new();
        let mut recipes = Vec::new();
        while !self.check(&TokenKind::RightBrace) {
            if self.check(&TokenKind::Eof) {
                return Err(Diagnostic::new("reward 缺少 `}`", self.current().span));
            }
            let (property, property_span) = self.ident("奖励属性")?;
            let Some(property) = reward_property(&property) else {
                return Err(Diagnostic::new(
                    format!("未知奖励属性 `{property}`"),
                    property_span,
                ));
            };
            match property {
                "function" => {
                    if function.is_some() {
                        return Err(Diagnostic::new(
                            "reward 只能声明一次 function",
                            property_span,
                        ));
                    }
                    self.expect(TokenKind::Equal, "function 后需要 `=`")?;
                    function = Some(self.advancement_reference("奖励函数")?);
                    self.expect(TokenKind::Semicolon, "function 后需要 `;`")?;
                }
                "experience" => {
                    if experience.is_some() {
                        return Err(Diagnostic::new(
                            "reward 只能声明一次 experience",
                            property_span,
                        ));
                    }
                    self.expect(TokenKind::Equal, "experience 后需要 `=`")?;
                    experience = Some(self.signed("experience 经验值")?);
                    self.expect(TokenKind::Semicolon, "experience 后需要 `;`")?;
                }
                "loot" => {
                    self.expect(TokenKind::Equal, "loot 后需要 `=`")?;
                    loot.push(self.advancement_reference("战利品表")?);
                    self.expect(TokenKind::Semicolon, "loot 后需要 `;`")?;
                }
                "recipe" => {
                    self.expect(TokenKind::Equal, "recipe 后需要 `=`")?;
                    recipes.push(self.advancement_reference("配方")?);
                    self.expect(TokenKind::Semicolon, "recipe 后需要 `;`")?;
                }
                _ => unreachable!(),
            }
        }
        self.advance();
        Ok(AdvancementReward {
            function,
            experience,
            loot,
            recipes,
        })
    }

    /// `display { icon = …; title = "…"; description = "…"; … }`
    fn advancement_display(&mut self) -> Result<AdvancementDisplay, Diagnostic> {
        let start = self
            .expect(TokenKind::LeftBrace, "display 后需要 `{`")?
            .span;
        let mut icon = None;
        let mut title = None;
        let mut description = None;
        let mut frame = AdvancementFrame::Task;
        let mut frame_span = None;
        let mut background = None;
        let mut background_span = None;
        let mut show_toast = None;
        let mut announce_to_chat = None;
        let mut hidden = None;
        while !self.check(&TokenKind::RightBrace) {
            if self.check(&TokenKind::Eof) {
                return Err(Diagnostic::new("display 缺少 `}`", self.current().span));
            }
            let (property, property_span) = self.ident("展示属性")?;
            let Some(property) = display_property(&property) else {
                return Err(Diagnostic::new(
                    format!("未知展示属性 `{property}`"),
                    property_span,
                ));
            };
            match property {
                "icon" => {
                    if icon.is_some() {
                        return Err(Diagnostic::new("display 只能声明一次 icon", property_span));
                    }
                    self.expect(TokenKind::Equal, "icon 后需要 `=`")?;
                    icon = Some(self.ident("图标物品定义")?);
                    self.expect(TokenKind::Semicolon, "icon 后需要 `;`")?;
                }
                "title" => {
                    if title.is_some() {
                        return Err(Diagnostic::new("display 只能声明一次 title", property_span));
                    }
                    self.expect(TokenKind::Equal, "title 后需要 `=`")?;
                    title = Some(self.string("title 需要文本字符串")?);
                    self.expect(TokenKind::Semicolon, "title 后需要 `;`")?;
                }
                "description" => {
                    if description.is_some() {
                        return Err(Diagnostic::new(
                            "display 只能声明一次 description",
                            property_span,
                        ));
                    }
                    self.expect(TokenKind::Equal, "description 后需要 `=`")?;
                    description = Some(self.string("description 需要文本字符串")?);
                    self.expect(TokenKind::Semicolon, "description 后需要 `;`")?;
                }
                "frame" => {
                    if frame_span.is_some() {
                        return Err(Diagnostic::new("display 只能声明一次 frame", property_span));
                    }
                    self.expect(TokenKind::Equal, "frame 后需要 `=`")?;
                    let (value, value_span) = self.ident("task、goal 或 challenge")?;
                    let Some(value) = advancement_frame(&value) else {
                        return Err(Diagnostic::new(
                            "frame 只能是 task/任务、goal/目标 或 challenge/挑战",
                            value_span,
                        ));
                    };
                    self.expect(TokenKind::Semicolon, "frame 后需要 `;`")?;
                    frame = value;
                    frame_span = Some(property_span);
                }
                "background" => {
                    if background_span.is_some() {
                        return Err(Diagnostic::new(
                            "display 只能声明一次 background",
                            property_span,
                        ));
                    }
                    self.expect(TokenKind::Equal, "background 后需要 `=`")?;
                    let (value, value_span) = self.string("background 需要纹理资源位置")?;
                    self.expect(TokenKind::Semicolon, "background 后需要 `;`")?;
                    background = Some(value);
                    background_span = Some(value_span);
                }
                "show_toast" | "announce_to_chat" | "hidden" => {
                    let slot = match property {
                        "show_toast" => &mut show_toast,
                        "announce_to_chat" => &mut announce_to_chat,
                        _ => &mut hidden,
                    };
                    if slot.is_some() {
                        return Err(Diagnostic::new(
                            format!("display 只能声明一次 {property}"),
                            property_span,
                        ));
                    }
                    self.expect(TokenKind::Equal, &format!("{property} 后需要 `=`"))?;
                    let (value, value_span) = self.ident("true 或 false")?;
                    *slot = Some(match boolean_word(&value) {
                        Some("true") => true,
                        Some("false") => false,
                        _ => return Err(Diagnostic::new("这里需要 true 或 false", value_span)),
                    });
                    self.expect(TokenKind::Semicolon, &format!("{property} 后需要 `;`"))?;
                }
                _ => unreachable!(),
            }
        }
        let end = self.advance().span;
        let span = start.merge(end);
        let Some((icon, icon_span)) = icon else {
            return Err(Diagnostic::new("display 必须声明 icon", span));
        };
        let Some((title, _)) = title else {
            return Err(Diagnostic::new("display 必须声明 title", span));
        };
        let Some((description, _)) = description else {
            return Err(Diagnostic::new("display 必须声明 description", span));
        };
        Ok(AdvancementDisplay {
            icon,
            icon_span,
            title,
            description,
            frame,
            background,
            background_span,
            show_toast,
            announce_to_chat,
            hidden,
            span,
        })
    }
}
