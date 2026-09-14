use crate::ast::*;
use crate::diagnostic::Diagnostic;
use crate::lexer::{Token, TokenKind};

pub fn parse(tokens: Vec<Token>) -> Result<Program, Vec<Diagnostic>> {
    Parser { tokens, cursor: 0 }
        .program()
        .map_err(|error| vec![error])
}

struct Parser {
    tokens: Vec<Token>,
    cursor: usize,
}

impl Parser {
    fn program(&mut self) -> Result<Program, Diagnostic> {
        self.expect_word("namespace")?;
        let (namespace, namespace_span) = self.ident("命名空间名称")?;
        self.expect(TokenKind::Semicolon, "命名空间声明后需要 `;`")?;

        let mut scores = Vec::new();
        let mut queries = Vec::new();
        let mut item_stacks = Vec::new();
        let mut storages = Vec::new();
        let mut resources = Vec::new();
        let mut functions = Vec::new();
        while !self.check(&TokenKind::Eof) {
            if self.check_word("score") {
                scores.push(self.score()?);
            } else if self.check_word("query") {
                queries.push(self.query()?);
            } else if self.check_word("item") {
                item_stacks.push(self.item_stack()?);
            } else if self.check_word("storage") {
                storages.push(self.storage()?);
            } else if self.check_word("resource") {
                resources.push(self.resource()?);
            } else {
                functions.push(self.function()?);
            }
        }
        Ok(Program {
            namespace,
            namespace_span,
            scores,
            queries,
            item_stacks,
            storages,
            resources,
            functions,
        })
    }

    fn query(&mut self) -> Result<EntityQueryDecl, Diagnostic> {
        let start = self.expect_word("query")?.span;
        let (name, _) = self.ident("查询名称")?;
        self.expect(TokenKind::Equal, "查询名称后需要 `=`")?;
        self.expect_word("entity")?;
        self.expect(TokenKind::LeftParen, "entity 后需要 `(`")?;
        let (entity_type, _) = self.string("entity 需要实体类型字符串")?;
        self.expect(TokenKind::RightParen, "实体类型后需要 `)`")?;
        self.expect(TokenKind::LeftBrace, "实体查询需要 `{`")?;

        let mut tags = Vec::new();
        let mut excluded_tags = Vec::new();
        let mut limit = None;
        let mut sort = None;
        let mut within = None;
        let mut item = None;
        while !self.check(&TokenKind::RightBrace) {
            if self.check(&TokenKind::Eof) {
                return Err(Diagnostic::new("实体查询缺少 `}`", self.current().span));
            }
            let (property, property_span) = self.ident("查询属性")?;
            let Some(property_kind) = query_property(&property) else {
                return Err(Diagnostic::new(
                    format!("未知实体查询属性 `{property}`"),
                    property_span,
                ));
            };
            match property_kind {
                "tag" | "without_tag" => {
                    self.expect(TokenKind::LeftParen, "查询属性后需要 `(`")?;
                    let (value, _) = self.string("标签需要字符串")?;
                    self.expect(TokenKind::RightParen, "标签后需要 `)`")?;
                    self.expect(TokenKind::Semicolon, "查询属性后需要 `;`")?;
                    if property_kind == "tag" {
                        tags.push(value);
                    } else {
                        excluded_tags.push(value);
                    }
                }
                "limit" => {
                    if limit.is_some() {
                        return Err(Diagnostic::new("查询只能声明一次 limit", property_span));
                    }
                    limit = Some(self.unsigned_call("limit")?);
                }
                "within" => {
                    if within.is_some() {
                        return Err(Diagnostic::new("查询只能声明一次 within", property_span));
                    }
                    within = Some(self.unsigned_call("within")?);
                }
                "sort" => {
                    if sort.is_some() {
                        return Err(Diagnostic::new("查询只能声明一次 sort", property_span));
                    }
                    self.expect(TokenKind::LeftParen, "sort 后需要 `(`")?;
                    let (value, span) = self.ident("排序方式")?;
                    sort = Some(match entity_sort(&value) {
                        Some("nearest") => EntitySort::Nearest,
                        Some("furthest") => EntitySort::Furthest,
                        Some("random") => EntitySort::Random,
                        Some("arbitrary") => EntitySort::Arbitrary,
                        _ => {
                            return Err(Diagnostic::new(
                                "排序方式只能是 nearest/最近、furthest/最远、random/随机或 arbitrary/任意",
                                span,
                            ));
                        }
                    });
                    self.expect(TokenKind::RightParen, "排序方式后需要 `)`")?;
                    self.expect(TokenKind::Semicolon, "查询属性后需要 `;`")?;
                }
                "item" => {
                    if item.is_some() {
                        return Err(Diagnostic::new(
                            "查询只能声明一个 item 过滤器",
                            property_span,
                        ));
                    }
                    item = Some(self.item_filter(property_span)?);
                }
                _ => unreachable!(),
            }
        }
        let end = self.advance().span;
        self.take(&TokenKind::Semicolon);
        Ok(EntityQueryDecl {
            name,
            entity_type,
            tags,
            excluded_tags,
            limit,
            sort,
            within,
            item,
            span: start.merge(end),
        })
    }

    fn item_filter(&mut self, start: Span) -> Result<ItemFilter, Diagnostic> {
        self.expect(TokenKind::LeftParen, "item 后需要 `(`")?;
        let (slot, _) = self.ident("物品槽名称")?;
        let slot = if slot == "内容" {
            "contents".to_owned()
        } else {
            slot
        };
        self.expect(TokenKind::RightParen, "物品槽后需要 `)`")?;
        self.expect(TokenKind::LeftBrace, "item 过滤器需要 `{`")?;
        let mut item_id = None;
        let mut count = None;
        let mut custom_name = None;
        while !self.check(&TokenKind::RightBrace) {
            if self.check(&TokenKind::Eof) {
                return Err(Diagnostic::new("item 过滤器缺少 `}`", self.current().span));
            }
            let (property, span) = self.ident("item 属性")?;
            self.expect(TokenKind::Equal, "item 属性后需要 `=`")?;
            let Some(property_kind) = item_property(&property) else {
                return Err(Diagnostic::new(
                    format!("未知 item 属性 `{property}`"),
                    span,
                ));
            };
            match property_kind {
                "id" => {
                    if item_id.is_some() {
                        return Err(Diagnostic::new("item.id 只能声明一次", span));
                    }
                    item_id = Some(self.string("item.id 需要物品类型字符串")?.0);
                }
                "count" => {
                    if count.is_some() {
                        return Err(Diagnostic::new("item.count 只能声明一次", span));
                    }
                    count = Some(self.unsigned("item.count")?);
                }
                "custom_name" => {
                    if custom_name.is_some() {
                        return Err(Diagnostic::new("item.custom_name 只能声明一次", span));
                    }
                    custom_name = Some(self.string("item.custom_name 需要文本字符串")?.0);
                }
                _ => unreachable!(),
            }
            self.expect(TokenKind::Semicolon, "item 属性后需要 `;`")?;
        }
        let end = self.advance().span;
        let Some(item_id) = item_id else {
            return Err(Diagnostic::new("item 过滤器必须声明 id", start.merge(end)));
        };
        Ok(ItemFilter {
            slot,
            item_id,
            count,
            custom_name,
            span: start.merge(end),
        })
    }

    fn item_stack(&mut self) -> Result<ItemStackDecl, Diagnostic> {
        let start = self.expect_word("item")?.span;
        let (name, _) = self.ident("物品定义名称")?;
        self.expect(TokenKind::Equal, "物品定义名称后需要 `=`")?;
        self.expect_word("item_stack")?;
        self.expect(TokenKind::LeftParen, "item_stack 后需要 `(`")?;
        let (item_id, _) = self.string("item_stack 需要物品资源位置")?;
        self.expect(TokenKind::RightParen, "物品资源位置后需要 `)`")?;
        self.expect(TokenKind::LeftBrace, "物品定义需要 `{`")?;

        let mut count = 1;
        let mut has_count = false;
        let mut custom_name = None;
        let mut lore = Vec::new();
        let mut enchantments = Vec::new();
        let mut stored_enchantments = Vec::new();
        let mut damage = None;
        let mut unbreakable = false;
        let mut has_unbreakable = false;
        while !self.check(&TokenKind::RightBrace) {
            if self.check(&TokenKind::Eof) {
                return Err(Diagnostic::new("物品定义缺少 `}`", self.current().span));
            }
            let (property, span) = self.ident("物品定义属性")?;
            let Some(property_kind) = item_stack_property(&property) else {
                return Err(Diagnostic::new(
                    format!("未知物品定义属性 `{property}`"),
                    span,
                ));
            };
            match property_kind {
                "count" => {
                    if has_count {
                        return Err(Diagnostic::new("物品数量只能声明一次", span));
                    }
                    has_count = true;
                    self.expect(TokenKind::Equal, "count 后需要 `=`")?;
                    count = self.unsigned("物品数量")?;
                    self.expect(TokenKind::Semicolon, "物品数量后需要 `;`")?;
                }
                "custom_name" => {
                    if custom_name.is_some() {
                        return Err(Diagnostic::new("物品自定义名称只能声明一次", span));
                    }
                    self.expect(TokenKind::Equal, "custom_name 后需要 `=`")?;
                    custom_name = Some(self.string("custom_name 需要文本字符串")?.0);
                    self.expect(TokenKind::Semicolon, "物品自定义名称后需要 `;`")?;
                }
                "lore" => {
                    self.expect(TokenKind::LeftParen, "lore 后需要 `(`")?;
                    lore.push(self.string("lore 需要文本字符串")?.0);
                    self.expect(TokenKind::RightParen, "lore 文本后需要 `)`")?;
                    self.expect(TokenKind::Semicolon, "lore 后需要 `;`")?;
                }
                "enchantment" | "stored_enchantment" => {
                    self.expect(TokenKind::LeftParen, "enchantment 后需要 `(`")?;
                    let (enchantment_id, enchantment_span) =
                        self.string("enchantment 需要附魔资源位置")?;
                    self.expect(TokenKind::Comma, "附魔资源位置后需要 `,`")?;
                    let level = self.unsigned("附魔等级")?;
                    self.expect(TokenKind::RightParen, "附魔等级后需要 `)`")?;
                    self.expect(TokenKind::Semicolon, "enchantment 后需要 `;`")?;
                    let enchantment = ItemEnchantment {
                        enchantment_id,
                        level,
                        span: enchantment_span,
                    };
                    if property_kind == "enchantment" {
                        enchantments.push(enchantment);
                    } else {
                        stored_enchantments.push(enchantment);
                    }
                }
                "damage" => {
                    if damage.is_some() {
                        return Err(Diagnostic::new("物品损伤值只能声明一次", span));
                    }
                    self.expect(TokenKind::Equal, "damage 后需要 `=`")?;
                    damage = Some(self.unsigned("物品损伤值")?);
                    self.expect(TokenKind::Semicolon, "物品损伤值后需要 `;`")?;
                }
                "unbreakable" => {
                    if has_unbreakable {
                        return Err(Diagnostic::new("unbreakable 只能声明一次", span));
                    }
                    has_unbreakable = true;
                    self.expect(TokenKind::Equal, "unbreakable 后需要 `=`")?;
                    let (value, value_span) = self.ident("true 或 false")?;
                    unbreakable = match boolean_word(&value) {
                        Some("true") => true,
                        Some("false") => false,
                        _ => return Err(Diagnostic::new("这里需要 true 或 false", value_span)),
                    };
                    self.expect(TokenKind::Semicolon, "unbreakable 后需要 `;`")?;
                }
                _ => unreachable!(),
            }
        }
        let end = self.advance().span;
        self.take(&TokenKind::Semicolon);
        Ok(ItemStackDecl {
            name,
            item_id,
            count,
            custom_name,
            lore,
            enchantments,
            stored_enchantments,
            damage,
            unbreakable,
            span: start.merge(end),
        })
    }

    fn storage(&mut self) -> Result<StorageDecl, Diagnostic> {
        let start = self.expect_word("storage")?.span;
        let (name, _) = self.ident("存储名称")?;
        self.expect(TokenKind::Equal, "存储名称后需要 `=`")?;
        self.expect_word("items")?;
        self.expect(TokenKind::LeftParen, "items 后需要 `(`")?;
        let (storage_id, _) = self.string("items 需要存储资源位置")?;
        self.expect(TokenKind::Comma, "存储资源位置后需要 `,`")?;
        let (path, _) = self.string("items 需要 NBT 路径")?;
        self.expect(TokenKind::RightParen, "存储声明缺少 `)`")?;
        let end = self
            .expect(TokenKind::Semicolon, "存储声明后需要 `;`")?
            .span;
        Ok(StorageDecl {
            name,
            storage_id,
            path,
            span: start.merge(end),
        })
    }

    fn unsigned_call(&mut self, name: &str) -> Result<u32, Diagnostic> {
        self.expect(TokenKind::LeftParen, &format!("{name} 后需要 `(`"))?;
        let value = self.unsigned(name)?;
        self.expect(TokenKind::RightParen, &format!("{name} 后需要 `)`"))?;
        self.expect(TokenKind::Semicolon, "查询属性后需要 `;`")?;
        Ok(value)
    }

    fn unsigned(&mut self, name: &str) -> Result<u32, Diagnostic> {
        let token = self.advance().clone();
        let TokenKind::Number(value) = token.kind else {
            return Err(Diagnostic::new(format!("{name} 需要非负整数"), token.span));
        };
        u32::try_from(value).map_err(|_| Diagnostic::new(format!("{name} 超出范围"), token.span))
    }

    fn resource(&mut self) -> Result<ResourceDecl, Diagnostic> {
        let start = self.expect_word("resource")?.span;
        let (kind, _) = self.resource_path("资源类型")?;
        let kind = if kind == "谓词" {
            "predicate".to_owned()
        } else {
            kind
        };
        let (name, _) = self.resource_path("资源名称")?;
        self.expect(TokenKind::Equal, "资源名称后需要 `=`")?;
        let (json, _) = self.string("资源内容需要 JSON 字符串")?;
        let end = self
            .expect(TokenKind::Semicolon, "资源声明后需要 `;`")?
            .span;
        Ok(ResourceDecl {
            kind,
            name,
            json,
            span: start.merge(end),
        })
    }

    fn resource_path(&mut self, expected: &str) -> Result<(String, Span), Diagnostic> {
        let token = self.advance().clone();
        match token.kind {
            TokenKind::Ident(value) | TokenKind::String(value) => Ok((value, token.span)),
            _ => Err(Diagnostic::new(
                format!("这里需要{expected}标识符或字符串"),
                token.span,
            )),
        }
    }

    fn score(&mut self) -> Result<ScoreDecl, Diagnostic> {
        let start = self.expect_word("score")?.span;
        let (name, _) = self.ident("计分变量名称")?;
        self.expect(
            TokenKind::Equal,
            "计分变量需要初始值，例如 `score count = 0;`",
        )?;
        let sign = if self.take(&TokenKind::Minus).is_some() {
            -1_i64
        } else {
            1
        };
        let token = self.advance().clone();
        let TokenKind::Number(number) = token.kind else {
            return Err(Diagnostic::new("初始值必须是整数常量", token.span));
        };
        let signed = number
            .checked_mul(sign)
            .ok_or_else(|| Diagnostic::new("整数超出 32 位范围", token.span))?;
        let initial =
            i32::try_from(signed).map_err(|_| Diagnostic::new("整数超出 32 位范围", token.span))?;
        let end = self
            .expect(TokenKind::Semicolon, "计分变量声明后需要 `;`")?
            .span;
        Ok(ScoreDecl {
            name,
            initial,
            span: start.merge(end),
        })
    }

    fn function(&mut self) -> Result<Function, Diagnostic> {
        let mut attributes = Vec::new();
        let start = self.current().span;
        while self.take(&TokenKind::At).is_some() {
            let (attribute, span) = self.ident("属性名称")?;
            attributes.push(match attribute.as_str() {
                "load" | "加载" => Attribute::Load,
                "tick" | "每刻" => Attribute::Tick,
                "entity" | "实体" => Attribute::Entity,
                "player" | "玩家" => Attribute::Player,
                _ => {
                    return Err(Diagnostic::new(
                        format!("未知函数属性 `@{attribute}`"),
                        span,
                    ));
                }
            });
        }
        self.expect_word("fn")?;
        let (name, _) = self.ident("函数名称")?;
        self.expect(TokenKind::LeftParen, "函数名称后需要 `(`")?;
        let mut parameters = Vec::new();
        if !self.check(&TokenKind::RightParen) {
            loop {
                let (name, span) = self.ident("参数名称")?;
                parameters.push(Parameter { name, span });
                if self.take(&TokenKind::Comma).is_none() {
                    break;
                }
            }
        }
        self.expect(TokenKind::RightParen, "参数列表缺少 `)`")?;
        let returns_score = if self.take(&TokenKind::Arrow).is_some() {
            self.expect_word("score")?;
            true
        } else {
            false
        };
        let (body, end) = self.block()?;
        Ok(Function {
            name,
            parameters,
            returns_score,
            attributes,
            body,
            span: start.merge(end),
        })
    }

    fn block(&mut self) -> Result<(Vec<Statement>, Span), Diagnostic> {
        self.expect(TokenKind::LeftBrace, "这里需要 `{`")?;
        let mut statements = Vec::new();
        while !self.check(&TokenKind::RightBrace) {
            if self.check(&TokenKind::Eof) {
                return Err(Diagnostic::new("代码块缺少 `}`", self.current().span));
            }
            statements.push(self.statement()?);
        }
        let end = self.advance().span;
        Ok((statements, end))
    }

    fn statement(&mut self) -> Result<Statement, Diagnostic> {
        let start = self.current().span;
        let kind = if self.take_word("each").is_some() {
            self.expect(TokenKind::LeftParen, "each 后需要 `(`")?;
            let (query, _) = self.ident("实体查询名称")?;
            self.expect(TokenKind::RightParen, "查询名称后需要 `)`")?;
            let (body, _) = self.block()?;
            StatementKind::Each { query, body }
        } else if self.take_word("in_dimension").is_some() {
            self.expect(TokenKind::LeftParen, "in_dimension 后需要 `(`")?;
            let (dimension, _) = self.string("in_dimension 需要维度资源位置")?;
            self.expect(TokenKind::RightParen, "维度资源位置后需要 `)`")?;
            let (body, _) = self.block()?;
            StatementKind::InDimension { dimension, body }
        } else if self.take_word("spawn").is_some() {
            self.expect(TokenKind::LeftParen, "spawn 后需要 `(`")?;
            let (entity_type, _) = self.string("spawn 需要实体类型资源位置")?;
            self.expect(TokenKind::RightParen, "实体类型后需要 `)`")?;
            let (body, _) = self.block()?;
            StatementKind::Spawn { entity_type, body }
        } else if self.take_word("self").is_some() {
            StatementKind::SelfAction(self.self_action()?)
        } else if self.take_word("message").is_some() {
            self.message_statement()?
        } else if self.take_word("sound").is_some() {
            self.sound_statement()?
        } else if self.take_word("run").is_some() {
            let (command, span) = self.string("run 后需要命令字符串")?;
            if command.trim().is_empty() {
                return Err(Diagnostic::new("run 命令不能为空", span));
            }
            if command.starts_with('/') {
                return Err(Diagnostic::new(
                    "Minecraft 函数中的命令不能以 `/` 开头",
                    span,
                ));
            }
            if command.contains(['\n', '\r']) {
                return Err(Diagnostic::new("一条 run 语句只能包含一行命令", span));
            }
            self.expect(TokenKind::Semicolon, "命令后需要 `;`")?;
            StatementKind::Run(command)
        } else if self.take_word("call").is_some() {
            let (name, _) = self.ident("被调用函数名称")?;
            let arguments = self.call_arguments()?;
            self.expect(TokenKind::Semicolon, "函数调用后需要 `;`")?;
            StatementKind::Call {
                function: name,
                arguments,
            }
        } else if self.take_word("schedule").is_some() {
            self.schedule_statement()?
        } else if self.take_word("if").is_some() {
            let condition = self.condition()?;
            let (then_body, _) = self.block()?;
            let else_body = if self.take_word("else").is_some() {
                self.block()?.0
            } else {
                Vec::new()
            };
            StatementKind::If {
                condition,
                then_body,
                else_body,
            }
        } else if self.take_word("while").is_some() {
            let condition = self.condition()?;
            let (body, _) = self.block()?;
            StatementKind::While { condition, body }
        } else if self.take_word("execute").is_some() {
            let (clauses, span) = self.string("execute 后需要子句字符串")?;
            if clauses.trim().is_empty()
                || clauses.contains(['\n', '\r'])
                || clauses.starts_with("execute ")
                || clauses.ends_with(" run")
            {
                return Err(Diagnostic::new(
                    "execute 字符串应只包含子句，例如 `as @a at @s`",
                    span,
                ));
            }
            let (body, _) = self.block()?;
            StatementKind::Execute { clauses, body }
        } else if self.take_word("return").is_some() {
            let value = if self.take(&TokenKind::Semicolon).is_some() {
                None
            } else {
                let value = self.expression()?;
                self.expect(TokenKind::Semicolon, "return 表达式后需要 `;`")?;
                Some(value)
            };
            StatementKind::Return(value)
        } else if self.take_word("let").is_some() {
            let (name, _) = self.ident("局部变量名称")?;
            self.expect(TokenKind::Equal, "局部变量需要初始值")?;
            let value = self.expression()?;
            self.expect(TokenKind::Semicolon, "局部变量声明后需要 `;`")?;
            StatementKind::Let { name, value }
        } else {
            let (name, _) = self.ident("语句")?;
            if self.check(&TokenKind::LeftParen) {
                let arguments = self.call_arguments()?;
                self.expect(TokenKind::Semicolon, "函数调用后需要 `;`")?;
                StatementKind::Call {
                    function: name,
                    arguments,
                }
            } else {
                let operation = self.assignment_operator()?;
                let value = self.expression()?;
                self.expect(TokenKind::Semicolon, "赋值后需要 `;`")?;
                StatementKind::Assign {
                    target: name,
                    operation,
                    value,
                }
            }
        };
        let end = self.previous().span;
        Ok(Statement {
            kind,
            span: start.merge(end),
        })
    }

    fn self_action(&mut self) -> Result<SelfAction, Diagnostic> {
        self.expect(TokenKind::Dot, "self 后需要 `.`")?;
        let (method, span) = self.ident("self 方法名称")?;
        let Some(method_kind) = self_method(&method) else {
            return Err(Diagnostic::new(format!("未知 self 方法 `{method}`"), span));
        };
        self.expect(TokenKind::LeftParen, "self 方法后需要 `(`")?;
        let action = match method_kind {
            "add_tag" | "remove_tag" => {
                let (tag, _) = self.string("标签需要字符串")?;
                if method_kind == "add_tag" {
                    SelfAction::AddTag(tag)
                } else {
                    SelfAction::RemoveTag(tag)
                }
            }
            "set_invulnerable" => {
                let (value, value_span) = self.ident("true 或 false")?;
                let value = match boolean_word(&value) {
                    Some("true") => true,
                    Some("false") => false,
                    _ => return Err(Diagnostic::new("这里需要 true 或 false", value_span)),
                };
                SelfAction::SetInvulnerable(value)
            }
            "save_items" | "restore_items" | "remove_preserving_items" | "give_item" => {
                let expected = if method_kind == "give_item" {
                    "物品定义名称"
                } else {
                    "物品存储名称"
                };
                let (reference, _) = self.ident(expected)?;
                match method_kind {
                    "save_items" => SelfAction::SaveItems(reference),
                    "restore_items" => SelfAction::RestoreItems(reference),
                    "remove_preserving_items" => SelfAction::RemovePreservingItems(reference),
                    "give_item" => SelfAction::GiveItem(reference),
                    _ => unreachable!(),
                }
            }
            "clear_items" => SelfAction::ClearItems,
            "remove" => SelfAction::Remove,
            "consume" => SelfAction::Consume,
            _ => unreachable!(),
        };
        self.expect(TokenKind::RightParen, "self 方法缺少 `)`")?;
        self.expect(TokenKind::Semicolon, "self 方法调用后需要 `;`")?;
        Ok(action)
    }

    fn message_statement(&mut self) -> Result<StatementKind, Diagnostic> {
        self.expect(TokenKind::Dot, "message 后需要 `.`")?;
        let (method, span) = self.ident("消息目标")?;
        let Some(method_kind) = message_target(&method) else {
            return Err(Diagnostic::new("消息目标只能是 all、self 或 nearest", span));
        };
        self.expect(TokenKind::LeftParen, "消息目标后需要 `(`")?;
        let (target, text) = match method_kind {
            "all" | "self" => {
                let (text, _) = self.string("消息需要文本字符串")?;
                let target = if method_kind == "all" {
                    MessageTarget::All
                } else {
                    MessageTarget::SelfEntity
                };
                (target, text)
            }
            "nearest" => {
                let within = self.unsigned("message.nearest 范围")?;
                self.expect(TokenKind::Comma, "范围后需要 `,`")?;
                let (text, _) = self.string("message.nearest 需要文本字符串")?;
                (MessageTarget::Nearest { within }, text)
            }
            _ => unreachable!(),
        };
        let color = if self.take(&TokenKind::Comma).is_some() {
            let (color, _) = self.ident("消息颜色")?;
            Some(text_color(&color).unwrap_or(&color).to_owned())
        } else {
            None
        };
        self.expect(TokenKind::RightParen, "消息调用缺少 `)`")?;
        self.expect(TokenKind::Semicolon, "消息调用后需要 `;`")?;
        Ok(StatementKind::Message {
            target,
            text,
            color,
        })
    }

    fn sound_statement(&mut self) -> Result<StatementKind, Diagnostic> {
        self.expect(TokenKind::Dot, "sound 后需要 `.`")?;
        let (target, span) = self.ident("声音目标")?;
        if !word_matches(&target, "self") {
            return Err(Diagnostic::new("声音目标目前只能是 self", span));
        }
        self.expect(TokenKind::LeftParen, "sound.self 后需要 `(`")?;
        let (sound, _) = self.string("sound.self 需要声音资源位置")?;
        self.expect(TokenKind::Comma, "声音资源位置后需要 `,`")?;
        let (source, _) = self.ident("声音分类")?;
        let source = sound_source(&source).unwrap_or(&source).to_owned();
        self.expect(TokenKind::RightParen, "声音调用缺少 `)`")?;
        self.expect(TokenKind::Semicolon, "声音调用后需要 `;`")?;
        Ok(StatementKind::PlaySound { sound, source })
    }

    fn schedule_statement(&mut self) -> Result<StatementKind, Diagnostic> {
        let (function, _) = self.ident("被调度函数名称")?;
        self.empty_arguments()?;
        self.expect_word("after")?;
        let number_token = self.advance().clone();
        let TokenKind::Number(number) = number_token.kind else {
            return Err(Diagnostic::new("调度延迟需要正整数", number_token.span));
        };
        if !(1..=i64::from(i32::MAX)).contains(&number) {
            return Err(Diagnostic::new(
                "调度延迟必须是 1 到 2147483647 之间的整数",
                number_token.span,
            ));
        }
        let (unit, unit_span) = self.ident("时间单位 t、s 或 d")?;
        let Some(unit) = time_unit(&unit) else {
            return Err(Diagnostic::new("时间单位只能是 t、s 或 d", unit_span));
        };
        let mode = if self.take_word("append").is_some() {
            ScheduleMode::Append
        } else {
            self.take_word("replace");
            ScheduleMode::Replace
        };
        self.expect(TokenKind::Semicolon, "调度语句后需要 `;`")?;
        Ok(StatementKind::Schedule {
            function,
            delay: format!("{number}{unit}"),
            mode,
        })
    }

    fn condition(&mut self) -> Result<Condition, Diagnostic> {
        self.condition_or()
    }

    fn condition_or(&mut self) -> Result<Condition, Diagnostic> {
        let mut condition = self.condition_and()?;
        while self.take(&TokenKind::OrOr).is_some() {
            let right = self.condition_and()?;
            condition = Condition::Or(Box::new(condition), Box::new(right));
        }
        Ok(condition)
    }

    fn condition_and(&mut self) -> Result<Condition, Diagnostic> {
        let mut condition = self.condition_not()?;
        while self.take(&TokenKind::AndAnd).is_some() {
            let right = self.condition_not()?;
            condition = Condition::And(Box::new(condition), Box::new(right));
        }
        Ok(condition)
    }

    fn condition_not(&mut self) -> Result<Condition, Diagnostic> {
        if self.take(&TokenKind::Bang).is_some() {
            return Ok(Condition::Not(Box::new(self.condition_not()?)));
        }
        if let Some(start) = self.take_word("predicate") {
            self.expect(TokenKind::LeftParen, "predicate 后需要 `(`")?;
            let (name, _) = self.resource_path("predicate 资源名称")?;
            let end = self
                .expect(TokenKind::RightParen, "predicate 资源名称后需要 `)`")?
                .span;
            return Ok(Condition::Predicate {
                name,
                span: start.span.merge(end),
            });
        }
        if self.condition_group_starts() {
            self.expect(TokenKind::LeftParen, "这里需要 `(`")?;
            let condition = self.condition()?;
            self.expect(TokenKind::RightParen, "条件分组缺少 `)`")?;
            return Ok(condition);
        }
        self.comparison_condition()
    }

    fn condition_group_starts(&self) -> bool {
        if !self.check(&TokenKind::LeftParen) {
            return false;
        }
        let mut depth = 0_usize;
        for index in self.cursor..self.tokens.len() {
            match self.tokens[index].kind {
                TokenKind::LeftParen => depth += 1,
                TokenKind::RightParen => {
                    depth -= 1;
                    if depth == 0 {
                        let Some(next) = self.tokens.get(index + 1) else {
                            return true;
                        };
                        return !matches!(
                            &next.kind,
                            TokenKind::Plus
                                | TokenKind::Minus
                                | TokenKind::Star
                                | TokenKind::Slash
                                | TokenKind::Percent
                                | TokenKind::EqualEqual
                                | TokenKind::BangEqual
                                | TokenKind::Less
                                | TokenKind::LessEqual
                                | TokenKind::Greater
                                | TokenKind::GreaterEqual
                        );
                    }
                }
                TokenKind::Eof => return true,
                _ => {}
            }
        }
        true
    }

    fn comparison_condition(&mut self) -> Result<Condition, Diagnostic> {
        let left = self.expression()?;
        let token = self.advance().clone();
        let comparison = match token.kind {
            TokenKind::EqualEqual => Comparison::Equal,
            TokenKind::BangEqual => Comparison::NotEqual,
            TokenKind::Less => Comparison::Less,
            TokenKind::LessEqual => Comparison::LessEqual,
            TokenKind::Greater => Comparison::Greater,
            TokenKind::GreaterEqual => Comparison::GreaterEqual,
            _ => {
                return Err(Diagnostic::new(
                    "条件需要 ==、!=、<、<=、> 或 >=",
                    token.span,
                ));
            }
        };
        let right = self.expression()?;
        Ok(Condition::Compare {
            left,
            comparison,
            right,
        })
    }

    fn expression(&mut self) -> Result<Expr, Diagnostic> {
        self.additive()
    }

    fn additive(&mut self) -> Result<Expr, Diagnostic> {
        let mut expression = self.multiplicative()?;
        loop {
            let operation = if self.take(&TokenKind::Plus).is_some() {
                Some(BinaryOp::Add)
            } else if self.take(&TokenKind::Minus).is_some() {
                Some(BinaryOp::Subtract)
            } else {
                None
            };
            let Some(operation) = operation else { break };
            let right = self.multiplicative()?;
            let span = expression.span.merge(right.span);
            expression = Expr {
                kind: ExprKind::Binary {
                    left: Box::new(expression),
                    operation,
                    right: Box::new(right),
                },
                span,
            };
        }
        Ok(expression)
    }

    fn multiplicative(&mut self) -> Result<Expr, Diagnostic> {
        let mut expression = self.unary()?;
        loop {
            let operation = if self.take(&TokenKind::Star).is_some() {
                Some(BinaryOp::Multiply)
            } else if self.take(&TokenKind::Slash).is_some() {
                Some(BinaryOp::Divide)
            } else if self.take(&TokenKind::Percent).is_some() {
                Some(BinaryOp::Modulo)
            } else {
                None
            };
            let Some(operation) = operation else { break };
            let right = self.unary()?;
            let span = expression.span.merge(right.span);
            expression = Expr {
                kind: ExprKind::Binary {
                    left: Box::new(expression),
                    operation,
                    right: Box::new(right),
                },
                span,
            };
        }
        Ok(expression)
    }

    fn unary(&mut self) -> Result<Expr, Diagnostic> {
        if let Some(minus) = self.take(&TokenKind::Minus) {
            if let TokenKind::Number(number) = &self.current().kind {
                let number = *number;
                let number_span = self.advance().span;
                let signed = number
                    .checked_neg()
                    .and_then(|value| i32::try_from(value).ok())
                    .ok_or_else(|| Diagnostic::new("整数超出 32 位范围", number_span))?;
                return Ok(Expr {
                    kind: ExprKind::Integer(signed),
                    span: minus.span.merge(number_span),
                });
            }
            let value = self.unary()?;
            let span = minus.span.merge(value.span);
            return Ok(Expr {
                kind: ExprKind::Negate(Box::new(value)),
                span,
            });
        }
        self.primary()
    }

    fn primary(&mut self) -> Result<Expr, Diagnostic> {
        let token = self.advance().clone();
        match token.kind {
            TokenKind::Number(number) => {
                let value = i32::try_from(number)
                    .map_err(|_| Diagnostic::new("整数超出 32 位范围", token.span))?;
                Ok(Expr {
                    kind: ExprKind::Integer(value),
                    span: token.span,
                })
            }
            TokenKind::Ident(name) => {
                if self.check(&TokenKind::LeftParen) {
                    let arguments = self.call_arguments()?;
                    let span = token.span.merge(self.previous().span);
                    Ok(Expr {
                        kind: ExprKind::Call {
                            function: name,
                            arguments,
                        },
                        span,
                    })
                } else {
                    Ok(Expr {
                        kind: ExprKind::Score(name),
                        span: token.span,
                    })
                }
            }
            TokenKind::LeftParen => {
                let expression = self.expression()?;
                let end = self.expect(TokenKind::RightParen, "表达式缺少 `)`")?.span;
                Ok(Expr {
                    span: token.span.merge(end),
                    ..expression
                })
            }
            _ => Err(Diagnostic::new(
                "这里需要整数、计分变量或括号表达式",
                token.span,
            )),
        }
    }

    fn assignment_operator(&mut self) -> Result<AssignOp, Diagnostic> {
        let token = self.advance().clone();
        match token.kind {
            TokenKind::Equal => Ok(AssignOp::Set),
            TokenKind::PlusEqual => Ok(AssignOp::Add),
            TokenKind::MinusEqual => Ok(AssignOp::Subtract),
            TokenKind::StarEqual => Ok(AssignOp::Multiply),
            TokenKind::SlashEqual => Ok(AssignOp::Divide),
            TokenKind::PercentEqual => Ok(AssignOp::Modulo),
            _ => Err(Diagnostic::new(
                "这里需要赋值运算符 =、+=、-=、*=、/= 或 %=",
                token.span,
            )),
        }
    }

    fn empty_arguments(&mut self) -> Result<(), Diagnostic> {
        self.expect(TokenKind::LeftParen, "函数名称后需要 `(`")?;
        self.expect(TokenKind::RightParen, "调度函数暂不支持参数，需要 `)`")?;
        Ok(())
    }

    fn call_arguments(&mut self) -> Result<Vec<Expr>, Diagnostic> {
        self.expect(TokenKind::LeftParen, "函数名称后需要 `(`")?;
        let mut arguments = Vec::new();
        if !self.check(&TokenKind::RightParen) {
            loop {
                arguments.push(self.expression()?);
                if self.take(&TokenKind::Comma).is_none() {
                    break;
                }
            }
        }
        self.expect(TokenKind::RightParen, "实参列表缺少 `)`")?;
        Ok(arguments)
    }

    fn ident(&mut self, expected: &str) -> Result<(String, Span), Diagnostic> {
        let token = self.advance().clone();
        match token.kind {
            TokenKind::Ident(value) => Ok((value, token.span)),
            _ => Err(Diagnostic::new(format!("这里需要{expected}"), token.span)),
        }
    }

    fn string(&mut self, message: &str) -> Result<(String, Span), Diagnostic> {
        let token = self.advance().clone();
        match token.kind {
            TokenKind::String(value) => Ok((value, token.span)),
            _ => Err(Diagnostic::new(message, token.span)),
        }
    }

    fn expect_word(&mut self, word: &str) -> Result<Token, Diagnostic> {
        if self.check_word(word) {
            Ok(self.advance().clone())
        } else {
            let expected = keyword_alias(word)
                .map(|alias| format!("`{word}` 或 `{alias}`"))
                .unwrap_or_else(|| format!("`{word}`"));
            Err(Diagnostic::new(
                format!("这里需要 {expected}"),
                self.current().span,
            ))
        }
    }

    fn take_word(&mut self, word: &str) -> Option<Token> {
        if self.check_word(word) {
            Some(self.advance().clone())
        } else {
            None
        }
    }

    fn check_word(&self, word: &str) -> bool {
        matches!(&self.current().kind, TokenKind::Ident(value) if word_matches(value, word))
    }

    fn expect(&mut self, kind: TokenKind, message: &str) -> Result<Token, Diagnostic> {
        if self.check(&kind) {
            Ok(self.advance().clone())
        } else {
            Err(Diagnostic::new(message, self.current().span))
        }
    }

    fn take(&mut self, kind: &TokenKind) -> Option<Token> {
        if self.check(kind) {
            Some(self.advance().clone())
        } else {
            None
        }
    }

    fn check(&self, kind: &TokenKind) -> bool {
        std::mem::discriminant(&self.current().kind) == std::mem::discriminant(kind)
    }

    fn current(&self) -> &Token {
        &self.tokens[self.cursor]
    }

    fn previous(&self) -> &Token {
        &self.tokens[self.cursor - 1]
    }

    fn advance(&mut self) -> &Token {
        let index = self.cursor;
        if !self.check(&TokenKind::Eof) {
            self.cursor += 1;
        }
        &self.tokens[index]
    }
}

fn word_matches(value: &str, english: &str) -> bool {
    value == english || keyword_alias(english) == Some(value)
}

fn keyword_alias(english: &str) -> Option<&'static str> {
    match english {
        "namespace" => Some("命名空间"),
        "score" => Some("计分"),
        "query" => Some("查询"),
        "entity" => Some("实体"),
        "storage" => Some("存储"),
        "items" => Some("物品"),
        "item" => Some("物品"),
        "item_stack" => Some("物品堆"),
        "resource" => Some("资源"),
        "fn" => Some("函数"),
        "each" => Some("遍历"),
        "in_dimension" => Some("在维度"),
        "spawn" => Some("召唤"),
        "self" => Some("自身"),
        "message" => Some("消息"),
        "sound" => Some("声音"),
        "run" => Some("原生命令"),
        "call" => Some("调用"),
        "schedule" => Some("调度"),
        "after" => Some("延后"),
        "append" => Some("追加"),
        "replace" => Some("替换"),
        "if" => Some("如果"),
        "else" => Some("否则"),
        "while" => Some("当"),
        "execute" => Some("原生执行"),
        "return" => Some("返回"),
        "let" => Some("令"),
        "predicate" => Some("谓词"),
        _ => None,
    }
}

fn query_property(value: &str) -> Option<&'static str> {
    match value {
        "tag" | "标签" => Some("tag"),
        "without_tag" | "排除标签" => Some("without_tag"),
        "limit" | "上限" => Some("limit"),
        "within" | "范围" => Some("within"),
        "sort" | "排序" => Some("sort"),
        "item" | "物品" => Some("item"),
        _ => None,
    }
}

fn item_property(value: &str) -> Option<&'static str> {
    match value {
        "id" | "类型" => Some("id"),
        "count" | "数量" => Some("count"),
        "custom_name" | "自定义名称" => Some("custom_name"),
        _ => None,
    }
}

fn item_stack_property(value: &str) -> Option<&'static str> {
    match value {
        "count" | "数量" => Some("count"),
        "custom_name" | "自定义名称" => Some("custom_name"),
        "lore" | "描述" => Some("lore"),
        "enchantment" | "附魔" => Some("enchantment"),
        "stored_enchantment" | "存储附魔" => Some("stored_enchantment"),
        "damage" | "损伤" => Some("damage"),
        "unbreakable" | "无法破坏" => Some("unbreakable"),
        _ => None,
    }
}

fn entity_sort(value: &str) -> Option<&'static str> {
    match value {
        "nearest" | "最近" => Some("nearest"),
        "furthest" | "最远" => Some("furthest"),
        "random" | "随机" => Some("random"),
        "arbitrary" | "任意" => Some("arbitrary"),
        _ => None,
    }
}

fn self_method(value: &str) -> Option<&'static str> {
    match value {
        "add_tag" | "添加标签" => Some("add_tag"),
        "remove_tag" | "移除标签" => Some("remove_tag"),
        "set_invulnerable" | "设置无敌" => Some("set_invulnerable"),
        "save_items" | "保存物品" => Some("save_items"),
        "restore_items" | "恢复物品" => Some("restore_items"),
        "remove_preserving_items" | "保存并移除" => Some("remove_preserving_items"),
        "give_item" | "给予物品" => Some("give_item"),
        "clear_items" | "清空物品" => Some("clear_items"),
        "remove" | "移除" => Some("remove"),
        "consume" | "消耗" => Some("consume"),
        _ => None,
    }
}

fn message_target(value: &str) -> Option<&'static str> {
    match value {
        "all" | "全部" => Some("all"),
        "self" | "自身" => Some("self"),
        "nearest" | "最近" => Some("nearest"),
        _ => None,
    }
}

fn boolean_word(value: &str) -> Option<&'static str> {
    match value {
        "true" | "真" => Some("true"),
        "false" | "假" => Some("false"),
        _ => None,
    }
}

fn time_unit(value: &str) -> Option<&'static str> {
    match value {
        "t" | "刻" => Some("t"),
        "s" | "秒" => Some("s"),
        "d" | "天" => Some("d"),
        _ => None,
    }
}

fn text_color(value: &str) -> Option<&'static str> {
    match value {
        "black" | "黑色" => Some("black"),
        "dark_blue" | "深蓝色" => Some("dark_blue"),
        "dark_green" | "深绿色" => Some("dark_green"),
        "dark_aqua" | "深青色" => Some("dark_aqua"),
        "dark_red" | "深红色" => Some("dark_red"),
        "dark_purple" | "深紫色" => Some("dark_purple"),
        "gold" | "金色" => Some("gold"),
        "gray" | "灰色" => Some("gray"),
        "dark_gray" | "深灰色" => Some("dark_gray"),
        "blue" | "蓝色" => Some("blue"),
        "green" | "绿色" => Some("green"),
        "aqua" | "青色" => Some("aqua"),
        "red" | "红色" => Some("red"),
        "light_purple" | "亮紫色" => Some("light_purple"),
        "yellow" | "黄色" => Some("yellow"),
        "white" | "白色" => Some("white"),
        _ => None,
    }
}

fn sound_source(value: &str) -> Option<&'static str> {
    match value {
        "master" | "主音量" => Some("master"),
        "music" | "音乐" => Some("music"),
        "record" | "唱片" => Some("record"),
        "weather" | "天气" => Some("weather"),
        "block" | "方块" => Some("block"),
        "hostile" | "敌对" => Some("hostile"),
        "neutral" | "中立" => Some("neutral"),
        "player" | "玩家" => Some("player"),
        "ambient" | "环境" => Some("ambient"),
        "voice" | "语音" => Some("voice"),
        "ui" | "界面" => Some("ui"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::lex;

    #[test]
    fn parses_a_complete_program() {
        let source = r#"
            namespace demo;
            score timer = 0;
            @tick fn tick() {
                timer += 1;
                if timer >= 20 { announce(); } else { run "say waiting"; }
            }
            fn announce() { schedule tick() after 1 s append; }
        "#;
        let program = parse(lex(source, 0).unwrap()).unwrap();
        assert_eq!(program.namespace, "demo");
        assert_eq!(program.scores.len(), 1);
        assert_eq!(program.functions.len(), 2);
    }
}
