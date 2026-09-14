use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::PathBuf;

use crate::ast::*;
use crate::diagnostic::Diagnostic;

#[derive(Debug)]
pub struct CompiledPack {
    pub files: BTreeMap<PathBuf, String>,
}

#[derive(Clone, Copy)]
struct Signature {
    parameters: usize,
    returns_score: bool,
    required_context: ExecutionContext,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum ExecutionContext {
    None,
    Entity,
    Player,
}

#[derive(Clone, Copy)]
struct ReturnRules {
    returns_score: bool,
    allowed_here: bool,
}

struct StatementSymbols<'a> {
    scores: &'a HashSet<&'a str>,
    parameters: &'a HashSet<&'a str>,
    functions: &'a HashMap<&'a str, Signature>,
    queries: &'a HashMap<&'a str, &'a EntityQueryDecl>,
    item_stacks: &'a HashSet<&'a str>,
    storages: &'a HashSet<&'a str>,
    predicates: &'a HashSet<&'a str>,
}

impl ReturnRules {
    fn nested(self) -> Self {
        Self {
            allowed_here: false,
            ..self
        }
    }
}

pub fn compile(program: &Program, description: &str) -> Result<CompiledPack, Vec<Diagnostic>> {
    let diagnostics = validate(program);
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }

    let mut compiler = Compiler::new(program);
    compiler.compile_functions();
    Ok(compiler.finish(description))
}

fn validate(program: &Program) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    if !valid_name(&program.namespace) || windows_reserved_name(&program.namespace) {
        diagnostics.push(Diagnostic::new(
            "命名空间只能包含小写 ASCII 字母、数字和下划线，不能以数字开头或使用 Windows 保留设备名",
            program.namespace_span,
        ));
    }

    let mut scores = HashSet::new();
    for score in &program.scores {
        validate_identifier("计分变量", &score.name, score.span, &mut diagnostics);
        if !scores.insert(score.name.as_str()) {
            diagnostics.push(Diagnostic::new(
                format!("重复声明计分变量 `{}`", score.name),
                score.span,
            ));
        }
    }

    let mut query_names = HashSet::new();
    let mut queries = HashMap::new();
    for query in &program.queries {
        validate_identifier("实体查询", &query.name, query.span, &mut diagnostics);
        if !query_names.insert(query.name.as_str()) {
            diagnostics.push(Diagnostic::new(
                format!("重复声明实体查询 `{}`", query.name),
                query.span,
            ));
        }
        queries.insert(query.name.as_str(), query);
        validate_entity_query(query, &mut diagnostics);
    }

    let mut item_stacks = HashSet::new();
    for item in &program.item_stacks {
        validate_identifier("物品定义", &item.name, item.span, &mut diagnostics);
        if !item_stacks.insert(item.name.as_str()) {
            diagnostics.push(Diagnostic::new(
                format!("重复声明物品定义 `{}`", item.name),
                item.span,
            ));
        }
        validate_item_stack(item, &mut diagnostics);
    }

    let mut storages = HashSet::new();
    for storage in &program.storages {
        validate_identifier("物品存储", &storage.name, storage.span, &mut diagnostics);
        if !storages.insert(storage.name.as_str()) {
            diagnostics.push(Diagnostic::new(
                format!("重复声明物品存储 `{}`", storage.name),
                storage.span,
            ));
        }
        if !valid_resource_location(&storage.storage_id) {
            diagnostics.push(Diagnostic::new(
                format!("`{}` 不是有效的存储资源位置", storage.storage_id),
                storage.span,
            ));
        }
        if !valid_nbt_path(&storage.path) {
            diagnostics.push(Diagnostic::new(
                format!("`{}` 不是受支持的存储路径", storage.path),
                storage.span,
            ));
        }
    }

    let mut resources = HashSet::new();
    for resource in &program.resources {
        if !supported_resource_kind(&resource.kind) {
            diagnostics.push(Diagnostic::new(
                format!("Minecraft 26.3 不支持 JSON 资源类型 `{}`", resource.kind),
                resource.span,
            ));
        }
        if !valid_resource_path(&resource.name) {
            diagnostics.push(Diagnostic::new(
                format!("资源名称 `{}` 不是有效的资源路径", resource.name),
                resource.span,
            ));
        }
        if !resources.insert((resource.kind.as_str(), resource.name.as_str())) {
            diagnostics.push(Diagnostic::new(
                format!("重复声明资源 `{}/{}`", resource.kind, resource.name),
                resource.span,
            ));
        }
        if let Err(error) = serde_json::from_str::<serde_json::Value>(&resource.json) {
            diagnostics.push(Diagnostic::new(
                format!(
                    "资源 `{}/{}` 的 JSON 无效（JSON 第 {} 行第 {} 列）：{}",
                    resource.kind,
                    resource.name,
                    error.line(),
                    error.column(),
                    error
                ),
                resource.span,
            ));
        }
    }
    let predicates = program
        .resources
        .iter()
        .filter(|resource| resource.kind == "predicate")
        .map(|resource| resource.name.as_str())
        .collect::<HashSet<_>>();

    let mut functions = HashSet::new();
    let mut signatures = HashMap::new();
    for function in &program.functions {
        validate_identifier("函数", &function.name, function.span, &mut diagnostics);
        if !functions.insert(function.name.as_str()) {
            diagnostics.push(Diagnostic::new(
                format!("重复声明函数 `{}`", function.name),
                function.span,
            ));
        }
        signatures.insert(
            function.name.as_str(),
            Signature {
                parameters: function.parameters.len(),
                returns_score: function.returns_score,
                required_context: function_context(function),
            },
        );
        let mut parameters = HashSet::new();
        for parameter in &function.parameters {
            validate_identifier("参数", &parameter.name, parameter.span, &mut diagnostics);
            if !parameters.insert(parameter.name.as_str()) {
                diagnostics.push(Diagnostic::new(
                    format!("重复声明参数 `{}`", parameter.name),
                    parameter.span,
                ));
            }
            if scores.contains(parameter.name.as_str()) {
                diagnostics.push(Diagnostic::new(
                    format!("参数 `{}` 与全局计分变量重名", parameter.name),
                    parameter.span,
                ));
            }
        }
        let mut attributes = HashSet::new();
        for attribute in &function.attributes {
            if !attributes.insert(*attribute) {
                diagnostics.push(Diagnostic::new(
                    "同一个函数不能重复使用相同属性",
                    function.span,
                ));
            }
        }
        let is_entry = function.attributes.contains(&Attribute::Load)
            || function.attributes.contains(&Attribute::Tick);
        if is_entry && !function.parameters.is_empty() {
            diagnostics.push(Diagnostic::new(
                "@load 和 @tick 入口函数不能声明参数",
                function.span,
            ));
        }
        if is_entry && function.returns_score {
            diagnostics.push(Diagnostic::new(
                "@load 和 @tick 入口函数不能返回值",
                function.span,
            ));
        }
        if is_entry
            && function
                .attributes
                .iter()
                .any(|attribute| matches!(attribute, Attribute::Entity | Attribute::Player))
        {
            diagnostics.push(Diagnostic::new(
                "@entity 和 @player 不能与 @load 或 @tick 用在同一个函数上",
                function.span,
            ));
        }
        if function.attributes.contains(&Attribute::Entity)
            && function.attributes.contains(&Attribute::Player)
        {
            diagnostics.push(Diagnostic::new(
                "同一个函数不能同时使用 @entity 和 @player",
                function.span,
            ));
        }
        if function.returns_score
            && !matches!(
                function.body.last().map(|statement| &statement.kind),
                Some(StatementKind::Return(Some(_)))
            )
        {
            diagnostics.push(Diagnostic::new(
                format!(
                    "返回 score 的函数 `{}` 必须以 return 表达式结束",
                    function.name
                ),
                function.span,
            ));
        }
    }

    for function in &program.functions {
        let parameters = function
            .parameters
            .iter()
            .map(|parameter| parameter.name.as_str())
            .collect::<HashSet<_>>();
        let mut declared_locals = HashSet::new();
        collect_local_declarations(
            &function.body,
            &scores,
            &parameters,
            &mut declared_locals,
            &mut diagnostics,
        );
        let mut visible_locals = HashSet::new();
        let symbols = StatementSymbols {
            scores: &scores,
            parameters: &parameters,
            functions: &signatures,
            queries: &queries,
            item_stacks: &item_stacks,
            storages: &storages,
            predicates: &predicates,
        };
        validate_statements(
            &function.body,
            &mut visible_locals,
            &symbols,
            function_context(function),
            ReturnRules {
                returns_score: function.returns_score,
                allowed_here: true,
            },
            &mut diagnostics,
        );
    }
    validate_synchronous_recursion(program, &functions, &mut diagnostics);
    diagnostics
}

fn validate_synchronous_recursion(
    program: &Program,
    known_functions: &HashSet<&str>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut graph = HashMap::<&str, HashSet<&str>>::new();
    for function in &program.functions {
        let mut calls = HashSet::new();
        collect_synchronous_calls(&function.body, &mut calls);
        calls.retain(|callee| known_functions.contains(callee));
        graph.insert(&function.name, calls);
    }

    for function in &program.functions {
        let mut visited = HashSet::new();
        if reaches_function(&function.name, &function.name, &graph, &mut visited) {
            diagnostics.push(Diagnostic::new(
                format!(
                    "函数 `{}` 位于同步递归调用环中；请改用 schedule 推迟下一次调用",
                    function.name
                ),
                function.span,
            ));
        }
    }
}

fn collect_synchronous_calls<'a>(statements: &'a [Statement], calls: &mut HashSet<&'a str>) {
    for statement in statements {
        match &statement.kind {
            StatementKind::Call {
                function,
                arguments,
            } => {
                calls.insert(function);
                for argument in arguments {
                    collect_expr_calls(argument, calls);
                }
            }
            StatementKind::If {
                condition,
                then_body,
                else_body,
                ..
            } => {
                collect_condition_calls(condition, calls);
                collect_synchronous_calls(then_body, calls);
                collect_synchronous_calls(else_body, calls);
            }
            StatementKind::While { condition, body } => {
                collect_condition_calls(condition, calls);
                collect_synchronous_calls(body, calls);
            }
            StatementKind::Execute { body, .. }
            | StatementKind::Each { body, .. }
            | StatementKind::InDimension { body, .. }
            | StatementKind::Spawn { body, .. } => collect_synchronous_calls(body, calls),
            StatementKind::Assign { value, .. } | StatementKind::Let { value, .. } => {
                collect_expr_calls(value, calls)
            }
            StatementKind::Return(Some(value)) => collect_expr_calls(value, calls),
            StatementKind::Run(_)
            | StatementKind::SelfAction(_)
            | StatementKind::Message { .. }
            | StatementKind::PlaySound { .. }
            | StatementKind::Schedule { .. }
            | StatementKind::Return(None) => {}
        }
    }
}

fn collect_condition_calls<'a>(condition: &'a Condition, calls: &mut HashSet<&'a str>) {
    match condition {
        Condition::Predicate { .. } => {}
        Condition::Compare { left, right, .. } => {
            collect_expr_calls(left, calls);
            collect_expr_calls(right, calls);
        }
        Condition::Not(condition) => collect_condition_calls(condition, calls),
        Condition::And(left, right) | Condition::Or(left, right) => {
            collect_condition_calls(left, calls);
            collect_condition_calls(right, calls);
        }
    }
}

fn collect_expr_calls<'a>(expression: &'a Expr, calls: &mut HashSet<&'a str>) {
    match &expression.kind {
        ExprKind::Call {
            function,
            arguments,
        } => {
            calls.insert(function);
            for argument in arguments {
                collect_expr_calls(argument, calls);
            }
        }
        ExprKind::Negate(value) => collect_expr_calls(value, calls),
        ExprKind::Binary { left, right, .. } => {
            collect_expr_calls(left, calls);
            collect_expr_calls(right, calls);
        }
        ExprKind::Integer(_) | ExprKind::Score(_) => {}
    }
}

fn reaches_function<'a>(
    current: &'a str,
    target: &str,
    graph: &HashMap<&'a str, HashSet<&'a str>>,
    visited: &mut HashSet<&'a str>,
) -> bool {
    let Some(callees) = graph.get(current) else {
        return false;
    };
    for callee in callees {
        if *callee == target {
            return true;
        }
        if visited.insert(callee) && reaches_function(callee, target, graph, visited) {
            return true;
        }
    }
    false
}

fn valid_name(name: &str) -> bool {
    let mut characters = name.chars();
    matches!(characters.next(), Some('a'..='z' | '_'))
        && characters.all(|character| matches!(character, 'a'..='z' | '0'..='9' | '_'))
}

fn valid_resource_path(path: &str) -> bool {
    !path.is_empty()
        && path.split('/').all(|segment| {
            !segment.is_empty()
                && segment != "."
                && segment != ".."
                && !windows_reserved_name(segment)
                && segment
                    .chars()
                    .all(|character| matches!(character, 'a'..='z' | '0'..='9' | '_' | '-' | '.'))
        })
}

fn valid_resource_location(value: &str) -> bool {
    let Some((namespace, path)) = value.split_once(':') else {
        return false;
    };
    !namespace.is_empty()
        && namespace
            .chars()
            .all(|character| matches!(character, 'a'..='z' | '0'..='9' | '_' | '-' | '.'))
        && valid_resource_path(path)
}

fn valid_nbt_path(path: &str) -> bool {
    !path.is_empty()
        && path.split('.').all(|segment| {
            !segment.is_empty()
                && segment
                    .chars()
                    .all(|character| character.is_ascii_alphanumeric() || character == '_')
        })
}

fn valid_entity_tag(tag: &str) -> bool {
    !tag.is_empty()
        && tag.len() <= 1024
        && tag.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '_' | '-' | '.')
        })
}

fn valid_text_color(color: &str) -> bool {
    matches!(
        color,
        "black"
            | "dark_blue"
            | "dark_green"
            | "dark_aqua"
            | "dark_red"
            | "dark_purple"
            | "gold"
            | "gray"
            | "dark_gray"
            | "blue"
            | "green"
            | "aqua"
            | "red"
            | "light_purple"
            | "yellow"
            | "white"
    )
}

fn valid_sound_source(source: &str) -> bool {
    matches!(
        source,
        "master"
            | "music"
            | "record"
            | "weather"
            | "block"
            | "hostile"
            | "neutral"
            | "player"
            | "ambient"
            | "voice"
            | "ui"
    )
}

fn function_context(function: &Function) -> ExecutionContext {
    if function.attributes.contains(&Attribute::Player) {
        ExecutionContext::Player
    } else if function.attributes.contains(&Attribute::Entity) {
        ExecutionContext::Entity
    } else {
        ExecutionContext::None
    }
}

fn validate_item_stack(item: &ItemStackDecl, diagnostics: &mut Vec<Diagnostic>) {
    if !valid_resource_location(&item.item_id) {
        diagnostics.push(Diagnostic::new(
            format!("`{}` 不是有效的物品资源位置", item.item_id),
            item.span,
        ));
    }
    if item.count == 0 || item.count > 100 {
        diagnostics.push(Diagnostic::new(
            "物品定义的 count 必须是 1 到 100",
            item.span,
        ));
    }
    if item.lore.len() > 256 {
        diagnostics.push(Diagnostic::new("物品定义最多包含 256 行 lore", item.span));
    }
    if item
        .custom_name
        .iter()
        .chain(&item.lore)
        .any(|text| text.chars().any(char::is_control))
    {
        diagnostics.push(Diagnostic::new(
            "物品名称和 lore 不能包含控制字符",
            item.span,
        ));
    }
    validate_item_enchantments("enchantment", &item.enchantments, diagnostics);
    validate_item_enchantments("stored_enchantment", &item.stored_enchantments, diagnostics);
    if item.damage.is_some_and(|damage| damage > i32::MAX as u32) {
        diagnostics.push(Diagnostic::new(
            "物品定义的 damage 不能超过 2147483647",
            item.span,
        ));
    }
}

fn validate_item_enchantments(
    property: &str,
    enchantments: &[ItemEnchantment],
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut ids = HashSet::new();
    for enchantment in enchantments {
        if !valid_resource_location(&enchantment.enchantment_id) {
            diagnostics.push(Diagnostic::new(
                format!("`{}` 不是有效的附魔资源位置", enchantment.enchantment_id),
                enchantment.span,
            ));
        }
        if enchantment.level == 0 || enchantment.level > 255 {
            diagnostics.push(Diagnostic::new("附魔等级必须是 1 到 255", enchantment.span));
        }
        if !ids.insert(enchantment.enchantment_id.as_str()) {
            diagnostics.push(Diagnostic::new(
                format!(
                    "物品定义重复声明 {property} `{}`",
                    enchantment.enchantment_id
                ),
                enchantment.span,
            ));
        }
    }
}

fn validate_entity_query(query: &EntityQueryDecl, diagnostics: &mut Vec<Diagnostic>) {
    if !valid_resource_location(&query.entity_type) {
        diagnostics.push(Diagnostic::new(
            format!("`{}` 不是有效的实体类型资源位置", query.entity_type),
            query.span,
        ));
    }
    let mut tags = HashSet::new();
    for tag in query.tags.iter().chain(&query.excluded_tags) {
        if !valid_entity_tag(tag) {
            diagnostics.push(Diagnostic::new(
                format!("`{tag}` 不是有效的实体标签"),
                query.span,
            ));
        }
        if !tags.insert(tag) {
            diagnostics.push(Diagnostic::new(
                format!("实体查询 `{}` 重复使用标签 `{tag}`", query.name),
                query.span,
            ));
        }
    }
    if query
        .limit
        .is_some_and(|limit| limit == 0 || limit > i32::MAX as u32)
    {
        diagnostics.push(Diagnostic::new(
            "查询 limit 必须是 1 到 2147483647",
            query.span,
        ));
    }
    if query
        .within
        .is_some_and(|within| within == 0 || within > 30_000_000)
    {
        diagnostics.push(Diagnostic::new(
            "查询 within 必须是 1 到 30000000",
            query.span,
        ));
    }
    if let Some(item) = &query.item {
        if item.slot != "contents" {
            diagnostics.push(Diagnostic::new(
                format!(
                    "Minecraft 26.3 的首版类型化物品查询只支持 contents 槽，实际为 `{}`",
                    item.slot
                ),
                item.span,
            ));
        }
        if !valid_resource_location(&item.item_id) {
            diagnostics.push(Diagnostic::new(
                format!("`{}` 不是有效的物品资源位置", item.item_id),
                item.span,
            ));
        }
        if item.count.is_some_and(|count| count == 0 || count > 99) {
            diagnostics.push(Diagnostic::new("item.count 必须是 1 到 99", item.span));
        }
        if item
            .custom_name
            .as_ref()
            .is_some_and(|name| name.chars().any(char::is_control))
        {
            diagnostics.push(Diagnostic::new(
                "item.custom_name 不能包含控制字符",
                item.span,
            ));
        }
    }
}

fn windows_reserved_name(name: &str) -> bool {
    let stem = name.split('.').next().unwrap_or(name).to_ascii_uppercase();
    matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || stem
            .strip_prefix("COM")
            .or_else(|| stem.strip_prefix("LPT"))
            .is_some_and(|number| {
                matches!(number, "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9")
            })
}

fn supported_resource_kind(kind: &str) -> bool {
    const SIMPLE_KINDS: &[&str] = &[
        "advancement",
        "banner_pattern",
        "block_transformer",
        "cat_sound_variant",
        "cat_variant",
        "chat_type",
        "chicken_sound_variant",
        "chicken_variant",
        "context_float_provider",
        "context_int_provider",
        "cow_sound_variant",
        "cow_variant",
        "damage_type",
        "decorated_pot_pattern",
        "dialog",
        "dimension_type",
        "enchantment",
        "enchantment_provider",
        "frog_variant",
        "instrument",
        "item_modifier",
        "jukebox_song",
        "loot_table",
        "painting_variant",
        "pig_sound_variant",
        "pig_variant",
        "predicate",
        "recipe",
        "sulfur_cube_archetype",
        "test_environment",
        "test_instance",
        "timeline",
        "trade_set",
        "trial_spawner",
        "trim_material",
        "trim_pattern",
        "villager_trade",
        "wolf_sound_variant",
        "wolf_variant",
        "world_clock",
        "zombie_nautilus_variant",
    ];
    SIMPLE_KINDS.contains(&kind) || kind.starts_with("worldgen/") && valid_resource_path(kind)
}

fn validate_identifier(kind: &str, name: &str, span: Span, diagnostics: &mut Vec<Diagnostic>) {
    const RESERVED: &[&str] = &[
        "namespace",
        "score",
        "resource",
        "query",
        "item",
        "item_stack",
        "storage",
        "entity",
        "items",
        "fn",
        "let",
        "run",
        "call",
        "schedule",
        "after",
        "append",
        "replace",
        "if",
        "else",
        "while",
        "execute",
        "each",
        "in_dimension",
        "spawn",
        "self",
        "message",
        "sound",
        "predicate",
        "player",
        "true",
        "false",
        "return",
    ];
    if !valid_name(name) {
        diagnostics.push(Diagnostic::new(
            format!("{kind}名 `{name}` 只能包含小写 ASCII 字母、数字和下划线，且不能以数字开头"),
            span,
        ));
    } else if name.len() > 32 {
        diagnostics.push(Diagnostic::new(
            format!("{kind}名 `{name}` 不能超过 32 个字节"),
            span,
        ));
    } else if kind == "函数" && windows_reserved_name(name) {
        diagnostics.push(Diagnostic::new(
            format!("函数名 `{name}` 会与 Windows 设备名冲突"),
            span,
        ));
    } else if RESERVED.contains(&name) {
        diagnostics.push(Diagnostic::new(
            format!("`{name}` 是保留字，不能用作{kind}名"),
            span,
        ));
    }
}

fn collect_local_declarations<'a>(
    statements: &'a [Statement],
    scores: &HashSet<&str>,
    parameters: &HashSet<&str>,
    locals: &mut HashSet<&'a str>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for statement in statements {
        match &statement.kind {
            StatementKind::Let { name, .. } => {
                validate_identifier("局部变量", name, statement.span, diagnostics);
                if scores.contains(name.as_str()) {
                    diagnostics.push(Diagnostic::new(
                        format!("局部变量 `{name}` 与全局计分变量重名"),
                        statement.span,
                    ));
                }
                if parameters.contains(name.as_str()) {
                    diagnostics.push(Diagnostic::new(
                        format!("局部变量 `{name}` 与函数参数重名"),
                        statement.span,
                    ));
                }
                if !locals.insert(name) {
                    diagnostics.push(Diagnostic::new(
                        format!("函数内重复声明局部变量 `{name}`"),
                        statement.span,
                    ));
                }
            }
            StatementKind::If {
                then_body,
                else_body,
                ..
            } => {
                collect_local_declarations(then_body, scores, parameters, locals, diagnostics);
                collect_local_declarations(else_body, scores, parameters, locals, diagnostics);
            }
            StatementKind::Execute { body, .. }
            | StatementKind::Each { body, .. }
            | StatementKind::InDimension { body, .. }
            | StatementKind::Spawn { body, .. }
            | StatementKind::While { body, .. } => {
                collect_local_declarations(body, scores, parameters, locals, diagnostics);
            }
            StatementKind::Run(_)
            | StatementKind::SelfAction(_)
            | StatementKind::Message { .. }
            | StatementKind::PlaySound { .. }
            | StatementKind::Call { .. }
            | StatementKind::Schedule { .. }
            | StatementKind::Assign { .. }
            | StatementKind::Return(_) => {}
        }
    }
}

fn validate_statements<'a>(
    statements: &'a [Statement],
    locals: &mut HashSet<&'a str>,
    symbols: &StatementSymbols<'_>,
    context: ExecutionContext,
    return_rules: ReturnRules,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let scores = symbols.scores;
    let parameters = symbols.parameters;
    let functions = symbols.functions;
    let queries = symbols.queries;
    let item_stacks = symbols.item_stacks;
    let storages = symbols.storages;
    for statement in statements {
        match &statement.kind {
            StatementKind::Run(_) => {}
            StatementKind::SelfAction(action) => {
                let required_context = if matches!(action, SelfAction::GiveItem(_)) {
                    ExecutionContext::Player
                } else {
                    ExecutionContext::Entity
                };
                if context < required_context {
                    let message = if required_context == ExecutionContext::Player {
                        "self.give_item 需要玩家执行上下文；请放入玩家 query 的 each 块，或给函数添加 @player"
                    } else {
                        "self 方法需要实体执行上下文；请放入 each/spawn 块，或给函数添加 @entity"
                    };
                    diagnostics.push(Diagnostic::new(message, statement.span));
                }
                match action {
                    SelfAction::AddTag(tag) | SelfAction::RemoveTag(tag) => {
                        if !valid_entity_tag(tag) {
                            diagnostics.push(Diagnostic::new(
                                format!("`{tag}` 不是有效的实体标签"),
                                statement.span,
                            ));
                        }
                    }
                    SelfAction::SaveItems(storage)
                    | SelfAction::RestoreItems(storage)
                    | SelfAction::RemovePreservingItems(storage) => {
                        if !storages.contains(storage.as_str()) {
                            diagnostics.push(Diagnostic::new(
                                format!("找不到物品存储 `{storage}`"),
                                statement.span,
                            ));
                        }
                    }
                    SelfAction::GiveItem(item) => {
                        if !item_stacks.contains(item.as_str()) {
                            diagnostics.push(Diagnostic::new(
                                format!("找不到物品定义 `{item}`"),
                                statement.span,
                            ));
                        }
                    }
                    SelfAction::SetInvulnerable(_)
                    | SelfAction::ClearItems
                    | SelfAction::Remove
                    | SelfAction::Consume => {}
                }
            }
            StatementKind::Message { target, color, .. } => {
                if matches!(target, MessageTarget::SelfEntity) && context < ExecutionContext::Player
                {
                    diagnostics.push(Diagnostic::new(
                        "message.self 需要玩家执行上下文",
                        statement.span,
                    ));
                }
                if let MessageTarget::Nearest { within } = target
                    && (*within == 0 || *within > 30_000_000)
                {
                    diagnostics.push(Diagnostic::new(
                        "最近玩家消息范围必须是 1 到 30000000",
                        statement.span,
                    ));
                }
                if let Some(color) = color
                    && !valid_text_color(color)
                {
                    diagnostics.push(Diagnostic::new(
                        format!("`{color}` 不是有效的文本颜色"),
                        statement.span,
                    ));
                }
            }
            StatementKind::PlaySound { sound, source } => {
                if context < ExecutionContext::Player {
                    diagnostics.push(Diagnostic::new(
                        "sound.self 需要玩家执行上下文",
                        statement.span,
                    ));
                }
                if !valid_resource_location(sound) {
                    diagnostics.push(Diagnostic::new(
                        format!("`{sound}` 不是有效的声音资源位置"),
                        statement.span,
                    ));
                }
                if !valid_sound_source(source) {
                    diagnostics.push(Diagnostic::new(
                        format!("`{source}` 不是有效的声音分类"),
                        statement.span,
                    ));
                }
            }
            StatementKind::Each { query, body } => {
                let query_decl = queries.get(query.as_str());
                if query_decl.is_none() {
                    diagnostics.push(Diagnostic::new(
                        format!("找不到实体查询 `{query}`"),
                        statement.span,
                    ));
                }
                let body_context = query_decl.map_or(ExecutionContext::Entity, |query| {
                    if query.entity_type == "minecraft:player" {
                        ExecutionContext::Player
                    } else {
                        ExecutionContext::Entity
                    }
                });
                let mut body_locals = locals.clone();
                validate_statements(
                    body,
                    &mut body_locals,
                    symbols,
                    body_context,
                    return_rules.nested(),
                    diagnostics,
                );
            }
            StatementKind::InDimension { dimension, body } => {
                if !valid_resource_location(dimension) {
                    diagnostics.push(Diagnostic::new(
                        format!("`{dimension}` 不是有效的维度资源位置"),
                        statement.span,
                    ));
                }
                let mut body_locals = locals.clone();
                validate_statements(
                    body,
                    &mut body_locals,
                    symbols,
                    context,
                    return_rules.nested(),
                    diagnostics,
                );
            }
            StatementKind::Spawn { entity_type, body } => {
                if !valid_resource_location(entity_type) {
                    diagnostics.push(Diagnostic::new(
                        format!("`{entity_type}` 不是有效的实体类型资源位置"),
                        statement.span,
                    ));
                }
                let mut body_locals = locals.clone();
                validate_statements(
                    body,
                    &mut body_locals,
                    symbols,
                    ExecutionContext::Entity,
                    return_rules.nested(),
                    diagnostics,
                );
            }
            StatementKind::Return(value) => {
                if !return_rules.allowed_here {
                    diagnostics.push(Diagnostic::new(
                        "return 只能直接出现在函数代码块中，不能放在 if、while、each、spawn、in_dimension 或 execute 块内",
                        statement.span,
                    ));
                }
                match (return_rules.returns_score, value) {
                    (true, Some(value)) => validate_expr(
                        value,
                        scores,
                        parameters,
                        locals,
                        functions,
                        context,
                        diagnostics,
                    ),
                    (true, None) => diagnostics.push(Diagnostic::new(
                        "返回 score 的函数需要 `return <表达式>;`",
                        statement.span,
                    )),
                    (false, Some(_)) => diagnostics.push(Diagnostic::new(
                        "无返回值函数只能使用 `return;`",
                        statement.span,
                    )),
                    (false, None) => {}
                }
            }
            StatementKind::Call {
                function,
                arguments,
            } => {
                if let Some(signature) = functions.get(function.as_str()) {
                    if arguments.len() != signature.parameters {
                        diagnostics.push(Diagnostic::new(
                            format!(
                                "函数 `{function}` 需要 {} 个参数，实际提供 {} 个",
                                signature.parameters,
                                arguments.len()
                            ),
                            statement.span,
                        ));
                    }
                    validate_call_context(
                        function,
                        *signature,
                        context,
                        statement.span,
                        diagnostics,
                    );
                } else {
                    diagnostics.push(Diagnostic::new(
                        format!("找不到函数 `{function}`"),
                        statement.span,
                    ));
                }
                for argument in arguments {
                    validate_expr(
                        argument,
                        scores,
                        parameters,
                        locals,
                        functions,
                        context,
                        diagnostics,
                    );
                }
            }
            StatementKind::Schedule { function, .. } => match functions.get(function.as_str()) {
                None => diagnostics.push(Diagnostic::new(
                    format!("找不到函数 `{function}`"),
                    statement.span,
                )),
                Some(Signature {
                    required_context, ..
                }) if *required_context != ExecutionContext::None => {
                    diagnostics.push(Diagnostic::new(
                        format!(
                            "不能调度需要执行上下文的函数 `{function}`，调度不会保留实体或玩家"
                        ),
                        statement.span,
                    ))
                }
                Some(Signature { parameters: 0, .. }) => {}
                Some(signature) => diagnostics.push(Diagnostic::new(
                    format!(
                        "不能调度需要 {} 个参数的函数 `{function}`",
                        signature.parameters
                    ),
                    statement.span,
                )),
            },
            StatementKind::Assign {
                target,
                operation,
                value,
            } => {
                if !scores.contains(target.as_str())
                    && !parameters.contains(target.as_str())
                    && !locals.contains(target.as_str())
                {
                    diagnostics.push(Diagnostic::new(
                        format!("找不到计分变量 `{target}`"),
                        statement.span,
                    ));
                }
                validate_expr(
                    value,
                    scores,
                    parameters,
                    locals,
                    functions,
                    context,
                    diagnostics,
                );
                if matches!(operation, AssignOp::Divide | AssignOp::Modulo)
                    && constant_value(value) == Some(0)
                {
                    diagnostics.push(Diagnostic::new("不能除以零", value.span));
                }
            }
            StatementKind::Let { name, value } => {
                validate_expr(
                    value,
                    scores,
                    parameters,
                    locals,
                    functions,
                    context,
                    diagnostics,
                );
                locals.insert(name);
            }
            StatementKind::If {
                condition,
                then_body,
                else_body,
            } => {
                validate_condition(condition, locals, symbols, context, diagnostics);
                let mut then_locals = locals.clone();
                validate_statements(
                    then_body,
                    &mut then_locals,
                    symbols,
                    context,
                    return_rules.nested(),
                    diagnostics,
                );
                let mut else_locals = locals.clone();
                validate_statements(
                    else_body,
                    &mut else_locals,
                    symbols,
                    context,
                    return_rules.nested(),
                    diagnostics,
                );
            }
            StatementKind::While { condition, body } => {
                validate_condition(condition, locals, symbols, context, diagnostics);
                let mut body_locals = locals.clone();
                validate_statements(
                    body,
                    &mut body_locals,
                    symbols,
                    context,
                    return_rules.nested(),
                    diagnostics,
                );
            }
            StatementKind::Execute { body, .. } => {
                let mut body_locals = locals.clone();
                validate_statements(
                    body,
                    &mut body_locals,
                    symbols,
                    context,
                    return_rules.nested(),
                    diagnostics,
                )
            }
        }
    }
}

fn validate_call_context(
    function: &str,
    signature: Signature,
    context: ExecutionContext,
    span: Span,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if context >= signature.required_context {
        return;
    }
    let (attribute, kind) = match signature.required_context {
        ExecutionContext::Player => ("@player", "玩家"),
        ExecutionContext::Entity => ("@entity", "实体"),
        ExecutionContext::None => return,
    };
    diagnostics.push(Diagnostic::new(
        format!("{attribute} 函数 `{function}` 需要{kind}执行上下文"),
        span,
    ));
}

fn validate_condition(
    condition: &Condition,
    locals: &HashSet<&str>,
    symbols: &StatementSymbols<'_>,
    context: ExecutionContext,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let scores = symbols.scores;
    let parameters = symbols.parameters;
    let functions = symbols.functions;
    let predicates = symbols.predicates;
    match condition {
        Condition::Predicate { name, span } => {
            if !predicates.contains(name.as_str()) {
                diagnostics.push(Diagnostic::new(
                    format!("找不到 predicate 资源 `{name}`"),
                    *span,
                ));
            }
        }
        Condition::Compare { left, right, .. } => {
            validate_expr(
                left,
                scores,
                parameters,
                locals,
                functions,
                context,
                diagnostics,
            );
            validate_expr(
                right,
                scores,
                parameters,
                locals,
                functions,
                context,
                diagnostics,
            );
        }
        Condition::Not(condition) => {
            validate_condition(condition, locals, symbols, context, diagnostics)
        }
        Condition::And(left, right) | Condition::Or(left, right) => {
            validate_condition(left, locals, symbols, context, diagnostics);
            validate_condition(right, locals, symbols, context, diagnostics);
        }
    }
}

fn validate_expr(
    expression: &Expr,
    scores: &HashSet<&str>,
    parameters: &HashSet<&str>,
    locals: &HashSet<&str>,
    functions: &HashMap<&str, Signature>,
    context: ExecutionContext,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match &expression.kind {
        ExprKind::Integer(_) => {}
        ExprKind::Score(name) => {
            if !scores.contains(name.as_str())
                && !parameters.contains(name.as_str())
                && !locals.contains(name.as_str())
            {
                diagnostics.push(Diagnostic::new(
                    format!("找不到计分变量 `{name}`"),
                    expression.span,
                ));
            }
        }
        ExprKind::Call {
            function,
            arguments,
        } => {
            match functions.get(function.as_str()) {
                None => diagnostics.push(Diagnostic::new(
                    format!("找不到函数 `{function}`"),
                    expression.span,
                )),
                Some(signature) => {
                    if !signature.returns_score {
                        diagnostics.push(Diagnostic::new(
                            format!("无返回值函数 `{function}` 不能用于表达式"),
                            expression.span,
                        ));
                    }
                    validate_call_context(
                        function,
                        *signature,
                        context,
                        expression.span,
                        diagnostics,
                    );
                    if arguments.len() != signature.parameters {
                        diagnostics.push(Diagnostic::new(
                            format!(
                                "函数 `{function}` 需要 {} 个参数，实际提供 {} 个",
                                signature.parameters,
                                arguments.len()
                            ),
                            expression.span,
                        ));
                    }
                }
            }
            for argument in arguments {
                validate_expr(
                    argument,
                    scores,
                    parameters,
                    locals,
                    functions,
                    context,
                    diagnostics,
                );
            }
        }
        ExprKind::Negate(value) => validate_expr(
            value,
            scores,
            parameters,
            locals,
            functions,
            context,
            diagnostics,
        ),
        ExprKind::Binary {
            left,
            operation,
            right,
        } => {
            validate_expr(
                left,
                scores,
                parameters,
                locals,
                functions,
                context,
                diagnostics,
            );
            validate_expr(
                right,
                scores,
                parameters,
                locals,
                functions,
                context,
                diagnostics,
            );
            if matches!(operation, BinaryOp::Divide | BinaryOp::Modulo)
                && constant_value(right) == Some(0)
            {
                diagnostics.push(Diagnostic::new("不能除以零", right.span));
            }
        }
    }
}

fn constant_value(expression: &Expr) -> Option<i32> {
    match &expression.kind {
        ExprKind::Integer(value) => Some(*value),
        ExprKind::Negate(value) => constant_value(value)?.checked_neg(),
        ExprKind::Binary {
            left,
            operation,
            right,
        } => {
            let left = constant_value(left)?;
            let right = constant_value(right)?;
            match operation {
                BinaryOp::Add => left.checked_add(right),
                BinaryOp::Subtract => left.checked_sub(right),
                BinaryOp::Multiply => left.checked_mul(right),
                BinaryOp::Divide => left.checked_div(right),
                BinaryOp::Modulo => left.checked_rem(right),
            }
        }
        ExprKind::Score(_) | ExprKind::Call { .. } => None,
    }
}

struct Compiler<'a> {
    program: &'a Program,
    objective: String,
    functions: BTreeMap<String, Vec<String>>,
    helper_counters: HashMap<String, usize>,
    temporary_counter: usize,
}

#[derive(Clone)]
enum Value {
    Integer(i32),
    Score(String),
}

impl<'a> Compiler<'a> {
    fn new(program: &'a Program) -> Self {
        Self {
            program,
            objective: objective_name(&program.namespace),
            functions: BTreeMap::new(),
            helper_counters: HashMap::new(),
            temporary_counter: 0,
        }
    }

    fn compile_functions(&mut self) {
        for function in &self.program.functions {
            let commands = self.compile_block(&function.body, &function.name);
            self.functions.insert(function.name.clone(), commands);
        }

        let load_functions = self
            .program
            .functions
            .iter()
            .filter(|function| function.attributes.contains(&Attribute::Load))
            .collect::<Vec<_>>();
        let mut commands = vec![format!(
            "scoreboard objectives add {} dummy",
            self.objective
        )];
        for score in &self.program.scores {
            commands.push(format!(
                "execute unless score {} {} = {} {} run scoreboard players set {} {} {}",
                score_holder(&score.name),
                self.objective,
                score_holder(&score.name),
                self.objective,
                score_holder(&score.name),
                self.objective,
                score.initial
            ));
        }
        for storage in &self.program.storages {
            commands.push(format!(
                "execute unless data storage {} {} run data modify storage {} {} set value []",
                storage.storage_id, storage.path, storage.storage_id, storage.path
            ));
        }
        for function in load_functions {
            commands.push(format!(
                "function {}:{}",
                self.program.namespace, function.name
            ));
        }
        self.functions.insert("__mcl/load".to_owned(), commands);
    }

    fn compile_block(&mut self, statements: &[Statement], owner: &str) -> Vec<String> {
        let mut commands = Vec::new();
        for statement in statements {
            match &statement.kind {
                StatementKind::Run(command) => commands.push(command.clone()),
                StatementKind::Each { query, body } => {
                    let helper = self.compile_helper(body, owner);
                    let query = self
                        .program
                        .queries
                        .iter()
                        .find(|candidate| candidate.name == *query)
                        .expect("semantic validation guarantees the entity query exists");
                    commands.push(format!(
                        "execute {} run function {}:{helper}",
                        entity_query_clause(query),
                        self.program.namespace
                    ));
                }
                StatementKind::InDimension { dimension, body } => {
                    let helper = self.compile_helper(body, owner);
                    commands.push(format!(
                        "execute in {dimension} run function {}:{helper}",
                        self.program.namespace
                    ));
                }
                StatementKind::Spawn { entity_type, body } => {
                    let helper = self.compile_helper(body, owner);
                    commands.push(format!(
                        "execute summon {entity_type} run function {}:{helper}",
                        self.program.namespace
                    ));
                }
                StatementKind::SelfAction(action) => {
                    commands.extend(self.compile_self_action(action));
                }
                StatementKind::Message {
                    target,
                    text,
                    color,
                } => commands.push(compile_message(target, text, color.as_deref())),
                StatementKind::PlaySound { sound, source } => {
                    commands.push(format!("playsound {sound} {source} @s ~ ~ ~ 1 1"))
                }
                StatementKind::Call {
                    function,
                    arguments,
                } => self.compile_call(function, arguments, owner, &mut commands),
                StatementKind::Schedule {
                    function,
                    delay,
                    mode,
                } => commands.push(format!(
                    "schedule function {}:{} {} {}",
                    self.program.namespace,
                    function,
                    delay,
                    match mode {
                        ScheduleMode::Replace => "replace",
                        ScheduleMode::Append => "append",
                    }
                )),
                StatementKind::Assign {
                    target,
                    operation,
                    value,
                } => self.compile_assignment(target, *operation, value, owner, &mut commands),
                StatementKind::Let { name, value } => {
                    self.compile_assignment(name, AssignOp::Set, value, owner, &mut commands)
                }
                StatementKind::If {
                    condition,
                    then_body,
                    else_body,
                } => {
                    self.compile_if(condition, then_body, else_body, owner, &mut commands);
                }
                StatementKind::While { condition, body } => {
                    self.compile_while(condition, body, owner, &mut commands);
                }
                StatementKind::Execute { clauses, body } => {
                    let helper = self.compile_helper(body, owner);
                    commands.push(format!(
                        "execute {clauses} run function {}:{helper}",
                        self.program.namespace
                    ));
                }
                StatementKind::Return(None) => commands.push("return 0".to_owned()),
                StatementKind::Return(Some(expression)) => {
                    let value = self.compile_expr(expression, owner, &mut commands);
                    match value {
                        Value::Integer(value) => commands.push(format!("return {value}")),
                        Value::Score(score) => commands.push(format!(
                            "return run scoreboard players get {score} {}",
                            self.objective
                        )),
                    }
                }
            }
        }
        commands
    }

    fn compile_self_action(&self, action: &SelfAction) -> Vec<String> {
        match action {
            SelfAction::AddTag(tag) => vec![format!("tag @s add {tag}")],
            SelfAction::RemoveTag(tag) => vec![format!("tag @s remove {tag}")],
            SelfAction::SetInvulnerable(value) => vec![format!(
                "data merge entity @s {{Invulnerable:{}b}}",
                if *value { 1 } else { 0 }
            )],
            SelfAction::SaveItems(name) => {
                let storage = self
                    .program
                    .storages
                    .iter()
                    .find(|candidate| candidate.name == *name)
                    .expect("semantic validation guarantees the item storage exists");
                vec![format!(
                    "data modify storage {} {} set from entity @s Items",
                    storage.storage_id, storage.path
                )]
            }
            SelfAction::RestoreItems(name) => {
                let storage = self
                    .program
                    .storages
                    .iter()
                    .find(|candidate| candidate.name == *name)
                    .expect("semantic validation guarantees the item storage exists");
                vec![format!(
                    "data modify entity @s Items set from storage {} {}",
                    storage.storage_id, storage.path
                )]
            }
            SelfAction::RemovePreservingItems(name) => {
                let storage = self
                    .program
                    .storages
                    .iter()
                    .find(|candidate| candidate.name == *name)
                    .expect("semantic validation guarantees the item storage exists");
                vec![
                    format!(
                        "data modify storage {} {} set from entity @s Items",
                        storage.storage_id, storage.path
                    ),
                    "data modify entity @s Items set value []".to_owned(),
                    "kill @s".to_owned(),
                ]
            }
            SelfAction::GiveItem(name) => {
                let item = self
                    .program
                    .item_stacks
                    .iter()
                    .find(|candidate| candidate.name == *name)
                    .expect("semantic validation guarantees the item definition exists");
                vec![format!(
                    "give @s {} {}",
                    item_stack_argument(item),
                    item.count
                )]
            }
            SelfAction::ClearItems => {
                vec!["data modify entity @s Items set value []".to_owned()]
            }
            SelfAction::Remove | SelfAction::Consume => vec!["kill @s".to_owned()],
        }
    }

    fn compile_call(
        &mut self,
        function: &str,
        arguments: &[Expr],
        owner: &str,
        commands: &mut Vec<String>,
    ) {
        self.bind_arguments(function, arguments, owner, commands);
        commands.push(format!("function {}:{function}", self.program.namespace));
    }

    fn bind_arguments(
        &mut self,
        function: &str,
        arguments: &[Expr],
        owner: &str,
        commands: &mut Vec<String>,
    ) {
        let mut values = Vec::new();
        for argument in arguments {
            values.push(self.compile_expr(argument, owner, commands));
        }
        let parameters = self
            .program
            .functions
            .iter()
            .find(|candidate| candidate.name == function)
            .expect("semantic validation guarantees the called function exists")
            .parameters
            .iter()
            .map(|parameter| parameter.name.clone())
            .collect::<Vec<_>>();
        for (parameter, value) in parameters.iter().zip(values) {
            let target = parameter_holder(function, parameter);
            match value {
                Value::Integer(value) => commands.push(format!(
                    "scoreboard players set {target} {} {value}",
                    self.objective
                )),
                Value::Score(source) => commands.push(format!(
                    "scoreboard players operation {target} {} = {source} {}",
                    self.objective, self.objective
                )),
            }
        }
    }

    fn compile_assignment(
        &mut self,
        target: &str,
        operation: AssignOp,
        expression: &Expr,
        owner: &str,
        commands: &mut Vec<String>,
    ) {
        let target = self.variable_holder(owner, target);
        let value = self.compile_expr(expression, owner, commands);
        if operation == AssignOp::Set {
            match value {
                Value::Integer(value) => commands.push(format!(
                    "scoreboard players set {target} {} {value}",
                    self.objective
                )),
                Value::Score(source) => commands.push(format!(
                    "scoreboard players operation {target} {} = {source} {}",
                    self.objective, self.objective
                )),
            }
            return;
        }

        if matches!(operation, AssignOp::Add | AssignOp::Subtract)
            && let Value::Integer(value) = &value
        {
            let signed = if operation == AssignOp::Add {
                i64::from(*value)
            } else {
                -i64::from(*value)
            };
            if (0..=i64::from(i32::MAX)).contains(&signed) {
                commands.push(format!(
                    "scoreboard players add {target} {} {signed}",
                    self.objective
                ));
                return;
            } else if (-i64::from(i32::MAX)..0).contains(&signed) {
                commands.push(format!(
                    "scoreboard players remove {target} {} {}",
                    self.objective, -signed
                ));
                return;
            }
        }

        let source = self.materialize(value, commands);
        let symbol = match operation {
            AssignOp::Set => unreachable!(),
            AssignOp::Add => "+=",
            AssignOp::Subtract => "-=",
            AssignOp::Multiply => "*=",
            AssignOp::Divide => "/=",
            AssignOp::Modulo => "%=",
        };
        commands.push(format!(
            "scoreboard players operation {target} {} {symbol} {source} {}",
            self.objective, self.objective
        ));
    }

    fn compile_if(
        &mut self,
        condition: &Condition,
        then_body: &[Statement],
        else_body: &[Statement],
        owner: &str,
        commands: &mut Vec<String>,
    ) {
        let flag = self.compile_condition(condition, owner, commands);
        let then_helper = self.compile_helper(then_body, owner);
        commands.push(format!(
            "execute if score {flag} {} matches 1 run function {}:{then_helper}",
            self.objective, self.program.namespace
        ));
        if !else_body.is_empty() {
            let else_helper = self.compile_helper(else_body, owner);
            commands.push(format!(
                "execute if score {flag} {} matches 0 run function {}:{else_helper}",
                self.objective, self.program.namespace
            ));
        }
    }

    fn compile_while(
        &mut self,
        condition: &Condition,
        body: &[Statement],
        owner: &str,
        commands: &mut Vec<String>,
    ) {
        let loop_helper = self.next_helper_path(owner);
        let body_helper = self.next_helper_path(owner);

        let mut body_commands = self.compile_block(body, owner);
        body_commands.push(format!("function {}:{loop_helper}", self.program.namespace));
        self.functions.insert(body_helper.clone(), body_commands);

        let mut loop_commands = Vec::new();
        let flag = self.compile_condition(condition, owner, &mut loop_commands);
        loop_commands.push(format!(
            "execute if score {flag} {} matches 1 run function {}:{body_helper}",
            self.objective, self.program.namespace
        ));
        self.functions.insert(loop_helper.clone(), loop_commands);
        commands.push(format!("function {}:{loop_helper}", self.program.namespace));
    }

    fn compile_condition(
        &mut self,
        condition: &Condition,
        owner: &str,
        commands: &mut Vec<String>,
    ) -> String {
        match condition {
            Condition::Predicate { name, .. } => {
                let flag = self.temporary();
                commands.push(format!(
                    "scoreboard players set {flag} {} 0",
                    self.objective
                ));
                commands.push(format!(
                    "execute if predicate {}:{name} run scoreboard players set {flag} {} 1",
                    self.program.namespace, self.objective
                ));
                flag
            }
            Condition::Compare {
                left,
                comparison,
                right,
            } => {
                let left_value = self.compile_expr(left, owner, commands);
                let right_value = self.compile_expr(right, owner, commands);
                let left = self.materialize(left_value, commands);
                let right = self.materialize(right_value, commands);
                let flag = self.temporary();
                commands.push(format!(
                    "scoreboard players set {flag} {} 0",
                    self.objective
                ));
                let (prefix, symbol) = match comparison {
                    Comparison::Equal => ("if", "="),
                    Comparison::NotEqual => ("unless", "="),
                    Comparison::Less => ("if", "<"),
                    Comparison::LessEqual => ("if", "<="),
                    Comparison::Greater => ("if", ">"),
                    Comparison::GreaterEqual => ("if", ">="),
                };
                commands.push(format!(
                    "execute {prefix} score {left} {} {symbol} {right} {} run scoreboard players set {flag} {} 1",
                    self.objective, self.objective, self.objective
                ));
                flag
            }
            Condition::Not(condition) => {
                let inner = self.compile_condition(condition, owner, commands);
                let flag = self.temporary();
                commands.push(format!(
                    "scoreboard players set {flag} {} 1",
                    self.objective
                ));
                commands.push(format!(
                    "execute if score {inner} {} matches 1 run scoreboard players set {flag} {} 0",
                    self.objective, self.objective
                ));
                flag
            }
            Condition::And(left, right) => {
                let left = self.compile_condition(left, owner, commands);
                let right = self.compile_condition(right, owner, commands);
                let flag = self.temporary();
                commands.push(format!(
                    "scoreboard players set {flag} {} 0",
                    self.objective
                ));
                commands.push(format!(
                    "execute if score {left} {} matches 1 if score {right} {} matches 1 run scoreboard players set {flag} {} 1",
                    self.objective, self.objective, self.objective
                ));
                flag
            }
            Condition::Or(left, right) => {
                let left = self.compile_condition(left, owner, commands);
                let right = self.compile_condition(right, owner, commands);
                let flag = self.temporary();
                commands.push(format!(
                    "scoreboard players set {flag} {} 0",
                    self.objective
                ));
                commands.push(format!(
                    "execute if score {left} {} matches 1 run scoreboard players set {flag} {} 1",
                    self.objective, self.objective
                ));
                commands.push(format!(
                    "execute if score {right} {} matches 1 run scoreboard players set {flag} {} 1",
                    self.objective, self.objective
                ));
                flag
            }
        }
    }

    fn compile_helper(&mut self, body: &[Statement], owner: &str) -> String {
        let path = self.next_helper_path(owner);
        let commands = self.compile_block(body, owner);
        self.functions.insert(path.clone(), commands);
        path
    }

    fn next_helper_path(&mut self, owner: &str) -> String {
        let counter = self.helper_counters.entry(owner.to_owned()).or_default();
        let path = format!("__mcl/{owner}/{}", *counter);
        *counter += 1;
        path
    }

    fn compile_expr(
        &mut self,
        expression: &Expr,
        owner: &str,
        commands: &mut Vec<String>,
    ) -> Value {
        if let Some(value) = constant_value(expression) {
            return Value::Integer(value);
        }
        match &expression.kind {
            ExprKind::Integer(value) => Value::Integer(*value),
            ExprKind::Score(name) => Value::Score(self.variable_holder(owner, name)),
            ExprKind::Call {
                function,
                arguments,
            } => {
                self.bind_arguments(function, arguments, owner, commands);
                let target = self.temporary();
                commands.push(format!(
                    "execute store result score {target} {} run function {}:{function}",
                    self.objective, self.program.namespace
                ));
                Value::Score(target)
            }
            ExprKind::Negate(value) => {
                let source_value = self.compile_expr(value, owner, commands);
                let source = self.materialize(source_value, commands);
                let target = self.temporary();
                commands.push(format!(
                    "scoreboard players set {target} {} 0",
                    self.objective
                ));
                commands.push(format!(
                    "scoreboard players operation {target} {} -= {source} {}",
                    self.objective, self.objective
                ));
                Value::Score(target)
            }
            ExprKind::Binary {
                left,
                operation,
                right,
            } => {
                let left_value = self.compile_expr(left, owner, commands);
                let right_value = self.compile_expr(right, owner, commands);
                let target = self.temporary();
                match left_value {
                    Value::Integer(value) => commands.push(format!(
                        "scoreboard players set {target} {} {value}",
                        self.objective
                    )),
                    Value::Score(source) => commands.push(format!(
                        "scoreboard players operation {target} {} = {source} {}",
                        self.objective, self.objective
                    )),
                }
                if matches!(operation, BinaryOp::Add | BinaryOp::Subtract)
                    && let Value::Integer(value) = &right_value
                {
                    let signed = if *operation == BinaryOp::Add {
                        i64::from(*value)
                    } else {
                        -i64::from(*value)
                    };
                    if (0..=i64::from(i32::MAX)).contains(&signed) {
                        commands.push(format!(
                            "scoreboard players add {target} {} {signed}",
                            self.objective
                        ));
                        return Value::Score(target);
                    } else if (-i64::from(i32::MAX)..0).contains(&signed) {
                        commands.push(format!(
                            "scoreboard players remove {target} {} {}",
                            self.objective, -signed
                        ));
                        return Value::Score(target);
                    }
                }
                let source = self.materialize(right_value, commands);
                let symbol = match operation {
                    BinaryOp::Add => "+=",
                    BinaryOp::Subtract => "-=",
                    BinaryOp::Multiply => "*=",
                    BinaryOp::Divide => "/=",
                    BinaryOp::Modulo => "%=",
                };
                commands.push(format!(
                    "scoreboard players operation {target} {} {symbol} {source} {}",
                    self.objective, self.objective
                ));
                Value::Score(target)
            }
        }
    }

    fn materialize(&mut self, value: Value, commands: &mut Vec<String>) -> String {
        match value {
            Value::Score(score) => score,
            Value::Integer(value) => {
                let score = self.temporary();
                commands.push(format!(
                    "scoreboard players set {score} {} {value}",
                    self.objective
                ));
                score
            }
        }
    }

    fn variable_holder(&self, owner: &str, name: &str) -> String {
        let is_parameter = self
            .program
            .functions
            .iter()
            .find(|function| function.name == owner)
            .is_some_and(|function| {
                function
                    .parameters
                    .iter()
                    .any(|parameter| parameter.name == name)
            });
        if is_parameter {
            parameter_holder(owner, name)
        } else if self
            .program
            .functions
            .iter()
            .find(|function| function.name == owner)
            .is_some_and(|function| function_has_local(&function.body, name))
        {
            local_holder(owner, name)
        } else {
            score_holder(name)
        }
    }

    fn temporary(&mut self) -> String {
        let temporary = format!("#t{}", self.temporary_counter);
        self.temporary_counter += 1;
        temporary
    }

    fn finish(self, description: &str) -> CompiledPack {
        let mut files = BTreeMap::new();
        files.insert(PathBuf::from("pack.mcmeta"), pack_metadata(description));
        for (name, commands) in self.functions {
            let path = PathBuf::from("data")
                .join(&self.program.namespace)
                .join("function")
                .join(format!("{name}.mcfunction"));
            let mut contents = "# Generated by mclang.\n".to_owned();
            if !commands.is_empty() {
                contents.push_str(&commands.join("\n"));
                contents.push('\n');
            }
            files.insert(path, contents);
        }
        for resource in &self.program.resources {
            let value = serde_json::from_str::<serde_json::Value>(&resource.json)
                .expect("semantic validation guarantees valid JSON");
            let contents = serde_json::to_string_pretty(&value)
                .expect("a parsed JSON value can always be serialized")
                + "\n";
            files.insert(
                PathBuf::from("data")
                    .join(&self.program.namespace)
                    .join(&resource.kind)
                    .join(format!("{}.json", resource.name)),
                contents,
            );
        }

        files.insert(
            PathBuf::from("data/minecraft/tags/function/load.json"),
            tag_json(&[format!("{}:__mcl/load", self.program.namespace)]),
        );
        let ticks = self
            .program
            .functions
            .iter()
            .filter(|function| function.attributes.contains(&Attribute::Tick))
            .map(|function| format!("{}:{}", self.program.namespace, function.name))
            .collect::<Vec<_>>();
        if !ticks.is_empty() {
            files.insert(
                PathBuf::from("data/minecraft/tags/function/tick.json"),
                tag_json(&ticks),
            );
        }
        CompiledPack { files }
    }
}

fn score_holder(name: &str) -> String {
    format!("#v_{name}")
}

fn parameter_holder(function: &str, parameter: &str) -> String {
    format!(
        "#p_{:012x}",
        stable_hash(&format!("{function}:{parameter}")) & 0xffffffffffff
    )
}

fn local_holder(function: &str, local: &str) -> String {
    format!(
        "#l_{:012x}",
        stable_hash(&format!("{function}:{local}")) & 0xffffffffffff
    )
}

fn function_has_local(statements: &[Statement], name: &str) -> bool {
    statements.iter().any(|statement| match &statement.kind {
        StatementKind::Let { name: local, .. } => local == name,
        StatementKind::If {
            then_body,
            else_body,
            ..
        } => function_has_local(then_body, name) || function_has_local(else_body, name),
        StatementKind::Execute { body, .. }
        | StatementKind::Each { body, .. }
        | StatementKind::InDimension { body, .. }
        | StatementKind::Spawn { body, .. }
        | StatementKind::While { body, .. } => function_has_local(body, name),
        StatementKind::Run(_)
        | StatementKind::SelfAction(_)
        | StatementKind::Message { .. }
        | StatementKind::PlaySound { .. }
        | StatementKind::Call { .. }
        | StatementKind::Schedule { .. }
        | StatementKind::Assign { .. }
        | StatementKind::Return(_) => false,
    })
}

fn objective_name(namespace: &str) -> String {
    format!("mcl_{:012x}", stable_hash(namespace) & 0xffffffffffff)
}

fn stable_hash(value: &str) -> u64 {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in value.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn pack_metadata(description: &str) -> String {
    format!(
        "{{\n  \"pack\": {{\n    \"description\": \"{}\",\n    \"min_format\": [121, 0],\n    \"max_format\": [121, 0]\n  }}\n}}\n",
        json_escape(description)
    )
}

fn entity_query_clause(query: &EntityQueryDecl) -> String {
    let mut selector = vec![format!("type={}", query.entity_type)];
    selector.extend(query.tags.iter().map(|tag| format!("tag={tag}")));
    selector.extend(query.excluded_tags.iter().map(|tag| format!("tag=!{tag}")));
    if let Some(sort) = query.sort {
        selector.push(format!("sort={}", sort.as_str()));
    }
    if let Some(limit) = query.limit {
        selector.push(format!("limit={limit}"));
    }
    if let Some(within) = query.within {
        selector.push(format!("distance=..{within}"));
    }

    let mut clause = format!("as @e[{}] at @s", selector.join(","));
    if let Some(item) = &query.item {
        let mut components = Vec::new();
        if let Some(custom_name) = &item.custom_name {
            components.push(format!(
                "minecraft:custom_name={{text:{}}}",
                snbt_string(custom_name)
            ));
        }
        if let Some(count) = item.count {
            components.push(format!("minecraft:count={count}"));
        }
        let predicate = if components.is_empty() {
            item.item_id.clone()
        } else {
            format!("{}[{}]", item.item_id, components.join(","))
        };
        clause.push_str(&format!(" if items entity @s {} {predicate}", item.slot));
    }
    clause
}

fn item_stack_argument(item: &ItemStackDecl) -> String {
    let mut components = Vec::new();
    if let Some(custom_name) = &item.custom_name {
        components.push(format!(
            "minecraft:custom_name={{text:{}}}",
            snbt_string(custom_name)
        ));
    }
    if !item.lore.is_empty() {
        let lines = item
            .lore
            .iter()
            .map(|line| format!("{{text:{}}}", snbt_string(line)))
            .collect::<Vec<_>>()
            .join(",");
        components.push(format!("minecraft:lore=[{lines}]"));
    }
    append_enchantments_component(
        &mut components,
        "minecraft:enchantments",
        &item.enchantments,
    );
    append_enchantments_component(
        &mut components,
        "minecraft:stored_enchantments",
        &item.stored_enchantments,
    );
    if let Some(damage) = item.damage {
        components.push(format!("minecraft:damage={damage}"));
    }
    if item.unbreakable {
        components.push("minecraft:unbreakable={}".to_owned());
    }
    if components.is_empty() {
        item.item_id.clone()
    } else {
        format!("{}[{}]", item.item_id, components.join(","))
    }
}

fn append_enchantments_component(
    components: &mut Vec<String>,
    component_id: &str,
    enchantments: &[ItemEnchantment],
) {
    if enchantments.is_empty() {
        return;
    }
    let entries = enchantments
        .iter()
        .map(|enchantment| {
            format!(
                "{}:{}",
                snbt_string(&enchantment.enchantment_id),
                enchantment.level
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    components.push(format!("{component_id}={{{entries}}}"));
}

fn snbt_string(value: &str) -> String {
    let mut escaped = String::from("\"");
    for character in value.chars() {
        match character {
            '\"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            character => escaped.push(character),
        }
    }
    escaped.push('\"');
    escaped
}

fn compile_message(target: &MessageTarget, text: &str, color: Option<&str>) -> String {
    let selector = match target {
        MessageTarget::All => "@a".to_owned(),
        MessageTarget::SelfEntity => "@s".to_owned(),
        MessageTarget::Nearest { within } => {
            format!("@a[sort=nearest,limit=1,distance=..{within}]")
        }
    };
    let mut component = serde_json::Map::new();
    component.insert(
        "text".to_owned(),
        serde_json::Value::String(text.to_owned()),
    );
    if let Some(color) = color {
        component.insert(
            "color".to_owned(),
            serde_json::Value::String(color.to_owned()),
        );
    }
    format!(
        "tellraw {selector} {}",
        serde_json::Value::Object(component)
    )
}

fn tag_json(values: &[String]) -> String {
    let values = values
        .iter()
        .map(|value| format!("    \"{}\"", json_escape(value)))
        .collect::<Vec<_>>()
        .join(",\n");
    format!("{{\n  \"values\": [\n{values}\n  ]\n}}\n")
}

fn json_escape(value: &str) -> String {
    let mut escaped = String::new();
    for character in value.chars() {
        match character {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            character if character < ' ' => {
                escaped.push_str(&format!("\\u{:04x}", character as u32))
            }
            character => escaped.push(character),
        }
    }
    escaped
}

#[cfg(test)]
mod tests {
    use crate::lexer::lex;
    use crate::parser::parse;

    use super::*;

    fn compile_text(source: &str) -> CompiledPack {
        let program = parse(lex(source, 0).unwrap()).unwrap();
        compile(&program, "test").unwrap()
    }

    #[test]
    fn emits_26_3_pack_and_entry_tags() {
        let pack = compile_text(
            "namespace demo; score timer = 0; @load fn start() {} @tick fn tick() { timer += 1; }",
        );
        assert!(pack.files[&PathBuf::from("pack.mcmeta")].contains("\"min_format\": [121, 0]"));
        assert!(
            pack.files
                .contains_key(&PathBuf::from("data/demo/function/__mcl/load.mcfunction"))
        );
        assert!(
            pack.files[&PathBuf::from("data/minecraft/tags/function/tick.json")]
                .contains("demo:tick")
        );
    }

    #[test]
    fn lowers_arithmetic_and_conditionals() {
        let pack = compile_text(
            "namespace demo; score timer = 0; fn tick() { timer = timer * 2 + 1; if timer >= 20 { run \"say done\"; } }",
        );
        let function = &pack.files[&PathBuf::from("data/demo/function/tick.mcfunction")];
        assert!(function.contains("scoreboard players operation"));
        assert!(function.contains("execute if score"));
        assert!(
            pack.files
                .contains_key(&PathBuf::from("data/demo/function/__mcl/tick/0.mcfunction"))
        );
    }

    #[test]
    fn rejects_unknown_references() {
        let program =
            parse(lex("namespace demo; fn main() { missing(); value = 1; }", 0).unwrap()).unwrap();
        let errors = compile(&program, "test").unwrap_err();
        assert_eq!(errors.len(), 2);
    }

    #[test]
    fn lowers_boolean_conditions_and_while_loops() {
        let pack = compile_text(
            "namespace demo; score a = 1; score b = 2; fn main() { if a == 1 && !b == 0 { while a < 4 { a += 1; } } }",
        );
        let main = &pack.files[&PathBuf::from("data/demo/function/main.mcfunction")];
        assert!(main.contains("matches 1 run function demo:__mcl/main/"));
        assert!(
            pack.files
                .keys()
                .filter(|path| path.to_string_lossy().contains("__mcl/main"))
                .count()
                >= 3
        );
    }

    #[test]
    fn rejects_synchronous_recursion_but_allows_scheduled_self_call() {
        let recursive =
            parse(lex("namespace demo; fn a() { b(); } fn b() { a(); }", 0).unwrap()).unwrap();
        assert_eq!(compile(&recursive, "test").unwrap_err().len(), 2);

        let scheduled =
            compile_text("namespace demo; fn heartbeat() { schedule heartbeat() after 1 t; }");
        assert!(
            scheduled
                .files
                .contains_key(&PathBuf::from("data/demo/function/heartbeat.mcfunction"))
        );
    }

    #[test]
    fn passes_expression_arguments_through_private_score_slots() {
        let pack = compile_text(
            "namespace demo; score total = 0; fn add(amount) { total += amount; } fn main() { add(total + 2); }",
        );
        let caller = &pack.files[&PathBuf::from("data/demo/function/main.mcfunction")];
        let callee = &pack.files[&PathBuf::from("data/demo/function/add.mcfunction")];
        assert!(caller.contains("scoreboard players operation #p_"));
        assert!(caller.contains("function demo:add"));
        assert!(callee.contains("+= #p_"));
    }

    #[test]
    fn validates_and_emits_json_resources() {
        let pack = compile_text(
            "namespace demo; resource predicate coin = \"\"\"{\"condition\":\"minecraft:random_chance\",\"chance\":0.5}\"\"\";",
        );
        let resource = &pack.files[&PathBuf::from("data/demo/predicate/coin.json")];
        assert!(resource.contains("\"chance\": 0.5"));

        let invalid = parse(
            lex(
                "namespace demo; resource predicate broken = \"\"\"{no}\"\"\";",
                0,
            )
            .unwrap(),
        )
        .unwrap();
        assert!(compile(&invalid, "test").is_err());
    }

    #[test]
    fn lowers_predicate_conditions_and_typed_sounds() {
        let pack = compile_text(
            r#"
            namespace demo;
            resource predicate coin = """{"condition":"minecraft:random_chance","chance":0.5}""";
            query players = entity("minecraft:player") {}
            @tick fn tick() {
                if predicate(coin) {
                    each(players) {
                        sound.self("minecraft:block.note_block.pling", master);
                    }
                }
            }
            "#,
        );
        let generated = pack.files.values().cloned().collect::<Vec<_>>().join("\n");
        assert!(generated.contains("execute if predicate demo:coin run scoreboard players set"));
        assert!(
            generated.contains("playsound minecraft:block.note_block.pling master @s ~ ~ ~ 1 1")
        );

        let invalid = parse(
            lex(
                r#"
                namespace demo;
                fn broken() {
                    if predicate(missing) {}
                    sound.self("Invalid Sound", invalid_category);
                }
                "#,
                0,
            )
            .unwrap(),
        )
        .unwrap();
        let errors = compile(&invalid, "test").unwrap_err();
        assert!(
            errors
                .iter()
                .any(|error| error.message.contains("找不到 predicate 资源"))
        );
        assert!(
            errors
                .iter()
                .any(|error| error.message.contains("需要玩家执行上下文"))
        );
        assert!(
            errors
                .iter()
                .any(|error| error.message.contains("不是有效的声音资源位置"))
        );
        assert!(
            errors
                .iter()
                .any(|error| error.message.contains("不是有效的声音分类"))
        );
    }

    #[test]
    fn chinese_and_english_keywords_compile_identically() {
        let english = compile_text(
            r#"
            namespace demo;
            score active = 0;
            query triggers = entity("minecraft:item") {
                without_tag("handled");
                limit(1);
                within(16);
                sort(nearest);
                item(contents) {
                    id = "minecraft:emerald";
                    count = 1;
                    custom_name = "A";
                }
            }
            query players = entity("minecraft:player") { tag("ready"); }
            item reward = item_stack("minecraft:diamond") {
                count = 3;
                custom_name = "Reward";
                lore("First line");
                enchantment("minecraft:fortune", 2);
                stored_enchantment("minecraft:mending", 1);
                damage = 4;
                unbreakable = true;
            }
            storage saved = items("demo:state", "saved_items");
            resource predicate coin = """{"condition":"minecraft:random_chance","chance":0.5}""";
            @load fn load() { message.all("loaded", green); }
            @entity fn mark() { self.add_tag("handled"); }
            @tick fn tick() {
                each(triggers) {
                    call mark();
                    if predicate(coin) {
                        spawn("minecraft:chest_minecart") {
                            self.set_invulnerable(true);
                            self.restore_items(saved);
                        }
                        message.nearest(16, "ready", gold);
                    } else {
                        in_dimension("minecraft:overworld") {
                            self.remove_preserving_items(saved);
                        }
                    }
                    self.consume();
                }
                each(players) {
                    self.give_item(reward);
                    sound.self("minecraft:block.note_block.pling", master);
                }
            }
            fn plus_one(value) -> score {
                let result = value + 1;
                return result;
            }
            fn later() { schedule later() after 1 s append; }
            fn drain() { while active > 0 { active -= 1; } }
            "#,
        );
        let chinese = compile_text(
            r#"
            命名空间 demo;
            计分 active = 0;
            查询 triggers = 实体("minecraft:item") {
                排除标签("handled");
                上限(1);
                范围(16);
                排序(最近);
                物品(内容) {
                    类型 = "minecraft:emerald";
                    数量 = 1;
                    自定义名称 = "A";
                }
            }
            查询 players = 实体("minecraft:player") { 标签("ready"); }
            物品 reward = 物品堆("minecraft:diamond") {
                数量 = 3;
                自定义名称 = "Reward";
                描述("First line");
                附魔("minecraft:fortune", 2);
                存储附魔("minecraft:mending", 1);
                损伤 = 4;
                无法破坏 = 真;
            }
            存储 saved = 物品("demo:state", "saved_items");
            资源 谓词 coin = """{"condition":"minecraft:random_chance","chance":0.5}""";
            @加载 函数 load() { 消息.全部("loaded", 绿色); }
            @实体 函数 mark() { 自身.添加标签("handled"); }
            @每刻 函数 tick() {
                遍历(triggers) {
                    调用 mark();
                    如果 谓词(coin) {
                        召唤("minecraft:chest_minecart") {
                            自身.设置无敌(真);
                            自身.恢复物品(saved);
                        }
                        消息.最近(16, "ready", 金色);
                    } 否则 {
                        在维度("minecraft:overworld") {
                            自身.保存并移除(saved);
                        }
                    }
                    自身.消耗();
                }
                遍历(players) {
                    自身.给予物品(reward);
                    声音.自身("minecraft:block.note_block.pling", 主音量);
                }
            }
            函数 plus_one(value) -> 计分 {
                令 result = value + 1;
                返回 result;
            }
            函数 later() { 调度 later() 延后 1 秒 追加; }
            函数 drain() { 当 active > 0 { active -= 1; } }
            "#,
        );

        assert_eq!(english.files, chinese.files);
    }

    #[test]
    fn enforces_local_scope_and_emits_private_local_slots() {
        let pack = compile_text(
            "namespace demo; score total = 0; fn add(value) { let doubled = value * 2; total += doubled; }",
        );
        let function = &pack.files[&PathBuf::from("data/demo/function/add.mcfunction")];
        assert!(function.contains("#l_"));

        let invalid = parse(
            lex(
                "namespace demo; fn broken() { value = 1; let value = 0; }",
                0,
            )
            .unwrap(),
        )
        .unwrap();
        assert!(compile(&invalid, "test").is_err());
    }

    #[test]
    fn captures_score_function_results_in_expressions() {
        let pack = compile_text(
            "namespace demo; score total = 0; fn double(value) -> score { return value * 2; } fn main() { total = double(20) + 2; }",
        );
        let returning = &pack.files[&PathBuf::from("data/demo/function/double.mcfunction")];
        let caller = &pack.files[&PathBuf::from("data/demo/function/main.mcfunction")];
        assert!(returning.contains("return run scoreboard players get"));
        assert!(caller.contains("execute store result score"));
        assert!(caller.contains("run function demo:double"));

        let invalid = parse(
            lex(
                "namespace demo; score flag = 1; fn wrong_void() { return 1; } fn wrong_value() -> score { return; } fn nested() { if flag == 1 { return; } }",
                0,
            )
            .unwrap(),
        )
        .unwrap();
        let errors = compile(&invalid, "test").unwrap_err();
        assert!(
            errors
                .iter()
                .any(|error| error.message.contains("无返回值函数"))
        );
        assert!(
            errors
                .iter()
                .any(|error| error.message.contains("需要 `return <表达式>;`"))
        );
        assert!(
            errors
                .iter()
                .any(|error| error.message.contains("只能直接出现在函数代码块"))
        );
    }

    #[test]
    fn lowers_typed_minecraft_queries_storage_and_actions() {
        let pack = compile_text(
            r#"
            namespace demo;
            score active = 0;
            query triggers = entity("minecraft:item") {
                without_tag("handled");
                item(contents) {
                    id = "minecraft:emerald";
                    count = 1;
                    custom_name = "A";
                }
            }
            storage saved = items("demo:state", "saved_items");
            @tick fn tick() {
                each(triggers) {
                    self.add_tag("handled");
                    spawn("minecraft:chest_minecart") {
                        self.set_invulnerable(true);
                        self.restore_items(saved);
                    }
                    self.remove_preserving_items(saved);
                    message.nearest(16, "Ready", green);
                }
            }
            "#,
        );
        let generated = pack.files.values().cloned().collect::<Vec<_>>().join("\n");
        assert!(generated.contains(
            "if items entity @s contents minecraft:emerald[minecraft:custom_name={text:\"A\"},minecraft:count=1]"
        ));
        assert!(generated.contains("execute summon minecraft:chest_minecart run function demo:"));
        assert!(generated.contains(
            "execute unless data storage demo:state saved_items run data modify storage demo:state saved_items set value []"
        ));
        assert!(
            generated
                .contains("data modify storage demo:state saved_items set from entity @s Items")
        );
        assert!(
            generated
                .contains("data modify entity @s Items set from storage demo:state saved_items")
        );
        assert!(generated.contains("data modify entity @s Items set value []"));
        assert!(generated.contains("tellraw @a[sort=nearest,limit=1,distance=..16]"));
    }

    #[test]
    fn lowers_typed_item_giving_and_checks_player_context() {
        let pack = compile_text(
            r#"
            namespace demo;
            item reward = item_stack("minecraft:diamond") {
                count = 3;
                custom_name = "Explorer's Gem";
                lore("First line");
                lore("Second line");
                enchantment("minecraft:fortune", 2);
                stored_enchantment("minecraft:mending", 1);
                damage = 4;
                unbreakable = true;
            }
            query players = entity("minecraft:player") {}
            @player fn reward_player() { self.give_item(reward); }
            @tick fn tick() {
                each(players) { reward_player(); }
            }
            "#,
        );
        let generated = pack.files.values().cloned().collect::<Vec<_>>().join("\n");
        assert!(generated.contains(
            "give @s minecraft:diamond[minecraft:custom_name={text:\"Explorer's Gem\"},minecraft:lore=[{text:\"First line\"},{text:\"Second line\"}],minecraft:enchantments={\"minecraft:fortune\":2},minecraft:stored_enchantments={\"minecraft:mending\":1},minecraft:damage=4,minecraft:unbreakable={}] 3"
        ));

        let invalid = parse(
            lex(
                r#"
                namespace demo;
                item broken = item_stack("Invalid Item") {
                    count = 101;
                    enchantment("Invalid Enchantment", 0);
                    enchantment("Invalid Enchantment", 256);
                    damage = 2147483648;
                }
                @player fn needs_player() {}
                @entity fn wrong_entity() {
                    needs_player();
                    self.give_item(missing);
                }
                fn wrong_call() { wrong_entity(); }
                fn wrong_schedule() { schedule needs_player() after 1 t; }
                "#,
                0,
            )
            .unwrap(),
        )
        .unwrap();
        let errors = compile(&invalid, "test").unwrap_err();
        assert!(
            errors
                .iter()
                .any(|error| error.message.contains("物品资源位置"))
        );
        assert!(
            errors
                .iter()
                .any(|error| error.message.contains("count 必须是 1 到 100"))
        );
        assert!(
            errors
                .iter()
                .any(|error| error.message.contains("附魔资源位置"))
        );
        assert!(
            errors
                .iter()
                .any(|error| error.message.contains("附魔等级"))
        );
        assert!(
            errors
                .iter()
                .any(|error| error.message.contains("重复声明"))
        );
        assert!(
            errors
                .iter()
                .any(|error| error.message.contains("2147483647"))
        );
        assert!(
            errors
                .iter()
                .any(|error| error.message.contains("需要玩家执行上下文"))
        );
        assert!(errors.iter().any(|error| {
            error
                .message
                .contains("@player 函数 `needs_player` 需要玩家执行上下文")
        }));
        assert!(
            errors
                .iter()
                .any(|error| error.message.contains("找不到物品定义"))
        );
        assert!(
            errors
                .iter()
                .any(|error| error.message.contains("不能调度需要执行上下文"))
        );
    }

    #[test]
    fn checks_entity_context_and_typed_references() {
        let source = r#"
            namespace demo;
            @entity fn entity_only() { self.remove(); }
            @tick fn tick() {
                entity_only();
                each(missing_query) { self.save_items(missing_storage); }
                message.all("bad color", orange);
            }
        "#;
        let program = parse(lex(source, 0).unwrap()).unwrap();
        let errors = compile(&program, "test").unwrap_err();
        assert!(
            errors
                .iter()
                .any(|error| error.message.contains("需要实体执行上下文"))
        );
        assert!(
            errors
                .iter()
                .any(|error| error.message.contains("找不到实体查询"))
        );
        assert!(
            errors
                .iter()
                .any(|error| error.message.contains("找不到物品存储"))
        );
        assert!(
            errors
                .iter()
                .any(|error| error.message.contains("不是有效的文本颜色"))
        );
    }
}
